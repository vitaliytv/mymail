#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: bun --cwd=app run release:dmg <X.Y.Z> [arm64|x64]" >&2
}

fail() {
  echo "release-dmg: $*" >&2
  exit 1
}

if [[ $# -lt 1 || $# -gt 2 || ! $1 =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  usage
  exit 1
fi

version=$1
arch=${2:-arm64}
case $arch in
  arm64)
    target=aarch64-apple-darwin
    updater_platform=darwin-aarch64
    ;;
  x64)
    target=x86_64-apple-darwin
    updater_platform=darwin-x86_64
    ;;
  *)
    usage
    exit 1
    ;;
esac
tag="v$version"
repo_root=$(git rev-parse --show-toplevel)
cd "$repo_root"

[[ $(uname -s) == Darwin ]] || fail "release assets must be built on macOS"
if ! git diff --quiet || ! git diff --cached --quiet; then
  fail "worktree must be clean"
fi
[[ $(git rev-parse "$tag^{commit}") == $(git rev-parse HEAD) ]] ||
  fail "$tag must point at HEAD"

for command in bun codesign security hdiutil shasum node rustup; do
  command -v "$command" >/dev/null || fail "missing required command: $command"
done

# Sign with the Developer ID identity installed in the runner keychain, as foc does;
# a certificate from the environment would make Tauri import it into a temporary keychain.
unset APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD

for variable in \
  APPLE_SIGNING_IDENTITY \
  APPLE_ID \
  APPLE_PASSWORD \
  APPLE_TEAM_ID \
  TAURI_SIGNING_PRIVATE_KEY; do
  [[ -n ${!variable:-} ]] || fail "missing $variable; run through Infisical"
done

security find-identity -v -p codesigning | grep -Fq "\"$APPLE_SIGNING_IDENTITY\"" ||
  fail "signing identity $APPLE_SIGNING_IDENTITY is not installed in the keychain"

manifest=app/package.json
output_dir="dist/MyMail-v$version-$arch"
[[ ! -e $output_dir ]] || fail "refusing to overwrite $output_dir"

manifest_version=$(node -p "require('./$manifest').version")
[[ $manifest_version == "$version" ]] ||
  fail "$manifest has version $manifest_version; build from the merged release tag $tag"

rustup target add "$target"
CI=true bun --cwd=app run tauri build --target "$target" --bundles app,dmg

bundle_dir=target/$target/release/bundle/dmg
dmg=$(find "$bundle_dir" -maxdepth 1 -type f -name '*.dmg' -print -quit)
[[ -n $dmg ]] || fail "Tauri did not create a DMG in $bundle_dir"
updater_dir=target/$target/release/bundle/macos
updater=$(find "$updater_dir" -maxdepth 1 -type f -name '*.app.tar.gz' -print -quit)
[[ -n $updater ]] || fail "Tauri did not create an updater archive in $updater_dir"
[[ -f "$updater.sig" ]] || fail "Tauri did not create an updater signature for $updater"

mkdir -p "$output_dir"
dmg_artifact="MyMail_${version}_${arch}.dmg"
updater_artifact="MyMail_${version}_${arch}.app.tar.gz"
cp "$dmg" "$output_dir/$dmg_artifact"
cp "$updater" "$output_dir/$updater_artifact"
cp "$updater.sig" "$output_dir/$updater_artifact.sig"

codesign --verify --deep --strict --verbose=2 "$updater_dir/MyMail.app"
hdiutil verify "$dmg"

(
  cd "$output_dir"
  shasum -a 256 "$dmg_artifact" "$updater_artifact" "$updater_artifact.sig" > SHA256SUMS
)

# latest.json carries only this architecture; the release collector merges the per-arch files.
node - "$version" "$updater_platform" "$updater_artifact" "$output_dir/$updater_artifact.sig" "$output_dir/latest.json" <<'NODE'
const [version, updaterPlatform, asset, signaturePath, output] = process.argv.slice(2)
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
        [updaterPlatform]: platform,
      },
    },
    null,
    2,
  ) + '\n',
)
NODE

echo "release assets: $output_dir"
