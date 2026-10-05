#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: bun --cwd=app run release:dmg <X.Y.Z>" >&2
}

fail() {
  echo "release-dmg: $*" >&2
  exit 1
}

if [[ $# -ne 1 || ! $1 =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  usage
  exit 1
fi

version=$1
tag="v$version"
repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

[[ $(uname -s) == Darwin ]] || fail "release assets must be built on macOS"
if ! git diff --quiet || ! git diff --cached --quiet; then
  fail "worktree must be clean"
fi
[[ $(git rev-parse "$tag^{commit}") == $(git rev-parse HEAD) ]] ||
  fail "$tag must point at HEAD"

for command in bun codesign security hdiutil shasum node rustup xcrun; do
  command -v "$command" >/dev/null || fail "missing required command: $command"
done

# Sign with the Developer ID identity installed in the runner keychain, as foc does;
# a certificate from the environment would make Tauri import it into a temporary keychain.
unset APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD
# Notarization uses the notarytool keychain profile below, so Tauri must not notarize itself.
unset APPLE_ID APPLE_PASSWORD APPLE_TEAM_ID APPLE_API_ISSUER APPLE_API_KEY APPLE_API_KEY_PATH

[[ -n ${APPLE_SIGNING_IDENTITY:-} ]] || fail "missing APPLE_SIGNING_IDENTITY; name of the Developer ID identity in the keychain"
[[ -n ${NOTARY_KEYCHAIN_PROFILE:-} ]] || fail "missing NOTARY_KEYCHAIN_PROFILE; see xcrun notarytool store-credentials"
[[ -n ${TAURI_SIGNING_PRIVATE_KEY:-} ]] || fail "missing TAURI_SIGNING_PRIVATE_KEY; the updater signing key (Forgejo Actions secret in CI)"

security find-identity -v -p codesigning | grep -Fq "\"$APPLE_SIGNING_IDENTITY\"" ||
  fail "signing identity $APPLE_SIGNING_IDENTITY is not installed in the keychain"
# Read the profile from an explicit keychain: notarytool otherwise searches the default keychain,
# which other release jobs on the shared runner may switch (v0.31.10 failed with "No Keychain
# password item found" although the profile was in the login keychain).
notary_keychain=${NOTARY_KEYCHAIN:-$HOME/Library/Keychains/login.keychain-db}
notary_auth=(--keychain-profile "$NOTARY_KEYCHAIN_PROFILE" --keychain "$notary_keychain")
if ! xcrun notarytool history "${notary_auth[@]}" >/dev/null; then
  echo "release-dmg: default keychain: $(security default-keychain 2>&1)" >&2
  fail "notarytool keychain profile $NOTARY_KEYCHAIN_PROFILE is not usable in $notary_keychain"
fi

manifest=app/package.json
output_dir="dist/MyMail-v$version"
[[ ! -e $output_dir ]] || fail "refusing to overwrite $output_dir"

manifest_version=$(node -p "require('./$manifest').version")
[[ $manifest_version == "$version" ]] ||
  fail "$manifest has version $manifest_version; build from the merged release tag $tag"

rustup target add aarch64-apple-darwin
# CI keeps CARGO_TARGET_DIR on the runner between releases; drop old bundles so only this build's DMG is found.
target_dir=${CARGO_TARGET_DIR:-target}
rm -rf "$target_dir/aarch64-apple-darwin/release/bundle"
CI=true bun --cwd=app run tauri build --target aarch64-apple-darwin --bundles app,dmg

bundle_dir=$target_dir/aarch64-apple-darwin/release/bundle/dmg
dmg=$(find "$bundle_dir" -maxdepth 1 -type f -name '*.dmg' -print -quit)
[[ -n $dmg ]] || fail "Tauri did not create a DMG in $bundle_dir"
updater_dir=$target_dir/aarch64-apple-darwin/release/bundle/macos
updater=$(find "$updater_dir" -maxdepth 1 -type f -name '*.app.tar.gz' -print -quit)
[[ -n $updater ]] || fail "Tauri did not create an updater archive in $updater_dir"
[[ -f "$updater.sig" ]] || fail "Tauri did not create an updater signature for $updater"

codesign --verify --deep --strict --verbose=2 "$updater_dir/MyMail.app"
hdiutil verify "$dmg"

# Notarizing the DMG also issues the ticket for the signed app inside it; the ticket is stapled to the DMG.
notary_log=$(mktemp)
notary_err=$(mktemp)
# notarytool sometimes aborts the upload to Apple's storage and writes no result; retry those attempts.
notary_attempts=3
notary_retry_delay=${NOTARY_RETRY_DELAY:-60}
notary_status=""
notary_id=""
for ((notary_attempt = 1; notary_attempt <= notary_attempts; notary_attempt++)); do
  # The JSON status decides, so a non-zero exit still reaches the checks below.
  notary_exit=0
  xcrun notarytool submit "$dmg" "${notary_auth[@]}" --wait --output-format json \
    > "$notary_log" 2> "$notary_err" || notary_exit=$?
  cat "$notary_log"
  # Prints "<status> <id>", or nothing when notarytool wrote no JSON (e.g. the upload to Apple was aborted).
  notary_result=$(node -e '
    try {
      const result = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"))
      console.log((result.status || "") + " " + (result.id || ""))
    } catch {}
  ' "$notary_log")
  read -r notary_status notary_id <<< "$notary_result" || true
  [[ -z ${notary_status:-} ]] || break
  cat "$notary_err" >&2
  if ((notary_attempt < notary_attempts)); then
    echo "release-dmg: notarytool submit returned no result (exit $notary_exit), attempt $notary_attempt of $notary_attempts; retrying in ${notary_retry_delay}s" >&2
    sleep "$notary_retry_delay"
  fi
done
if [[ -z ${notary_status:-} ]]; then
  fail "notarytool submit returned no result in $notary_attempts attempts (last exit $notary_exit); the upload of $dmg to Apple likely failed (see notarytool output above), rerun the release"
fi
if [[ $notary_status != Accepted ]]; then
  if [[ -n ${notary_id:-} ]]; then
    xcrun notarytool log "$notary_id" "${notary_auth[@]}" >&2 || true
  fi
  fail "notarization of $dmg finished with status $notary_status"
fi
xcrun stapler staple "$dmg"
xcrun stapler validate "$dmg"
spctl --assess --type open --context context:primary-signature --verbose=2 "$dmg"

mkdir -p "$output_dir"
dmg_artifact="MyMail_${version}_arm64.dmg"
updater_artifact="MyMail_${version}_arm64.app.tar.gz"
cp "$dmg" "$output_dir/$dmg_artifact"
cp "$updater" "$output_dir/$updater_artifact"
cp "$updater.sig" "$output_dir/$updater_artifact.sig"

(
  cd "$output_dir"
  shasum -a 256 "$dmg_artifact" "$updater_artifact" "$updater_artifact.sig" > SHA256SUMS
)

node - "$version" "$updater_artifact" "$output_dir/$updater_artifact.sig" "$output_dir/latest.json" <<'NODE'
const [version, asset, signaturePath, output] = process.argv.slice(2)
const fs = require('fs')
const signature = fs.readFileSync(signaturePath, 'utf8').trim()
const url = 'https://git.7n.ai/vitaliytv/mymail/releases/download/v' + version + '/' + asset
const platform = { signature, url }
fs.writeFileSync(
  output,
  JSON.stringify(
    {
      version,
      notes: 'MyMail v' + version,
      pub_date: new Date().toISOString(),
      platforms: {
        'darwin-aarch64': platform,
      },
    },
    null,
    2,
  ) + '\n',
)
NODE

echo "release assets: $output_dir"
