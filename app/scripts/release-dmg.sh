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

for command in bun codesign hdiutil shasum node rustup; do
  command -v "$command" >/dev/null || fail "missing required command: $command"
done

for variable in \
  APPLE_CERTIFICATE \
  APPLE_CERTIFICATE_PASSWORD \
  APPLE_SIGNING_IDENTITY \
  APPLE_ID \
  APPLE_PASSWORD \
  APPLE_TEAM_ID \
  TAURI_SIGNING_PRIVATE_KEY; do
  [[ -n ${!variable:-} ]] || fail "missing $variable; run through Infisical"
done

config=app/src-tauri/tauri.conf.json
output_dir="dist/MyMail-v$version"
[[ ! -e $output_dir ]] || fail "refusing to overwrite $output_dir"

backup=$(mktemp "${TMPDIR:-/tmp}/mymail-tauri-conf.XXXXXX")

cleanup() {
  cp "$backup" "$config"
  rm -f "$backup"
}
trap cleanup EXIT

cp "$config" "$backup"
node - "$config" "$version" <<'NODE'
const [file, version] = process.argv.slice(2)
const fs = require('fs')
const config = JSON.parse(fs.readFileSync(file, 'utf8'))
config.version = version
fs.writeFileSync(file, JSON.stringify(config, null, 2) + '\n')
NODE

rustup target add aarch64-apple-darwin x86_64-apple-darwin
CI=true bun --cwd=app run tauri build --target universal-apple-darwin --bundles app,dmg

bundle_dir=target/universal-apple-darwin/release/bundle/dmg
dmg=$(find "$bundle_dir" -maxdepth 1 -type f -name '*.dmg' -print -quit)
[[ -n $dmg ]] || fail "Tauri did not create a DMG in $bundle_dir"
updater_dir=target/universal-apple-darwin/release/bundle/macos
updater=$(find "$updater_dir" -maxdepth 1 -type f -name '*.app.tar.gz' -print -quit)
[[ -n $updater ]] || fail "Tauri did not create an updater archive in $updater_dir"
[[ -f "$updater.sig" ]] || fail "Tauri did not create an updater signature for $updater"

mkdir -p "$output_dir"
dmg_artifact="MyMail_${version}_universal.dmg"
updater_artifact="MyMail_${version}_universal.app.tar.gz"
cp "$dmg" "$output_dir/$dmg_artifact"
cp "$updater" "$output_dir/$updater_artifact"
cp "$updater.sig" "$output_dir/$updater_artifact.sig"

codesign --verify --deep --strict --verbose=2 "$updater_dir/MyMail.app"
hdiutil verify "$dmg"

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
        'darwin-x86_64': platform,
      },
    },
    null,
    2,
  ) + '\n',
)
NODE

echo "local release assets: $output_dir"
echo "upload with:"
echo "  foc release upload $tag $output_dir/$dmg_artifact --repo vitaliytv/mymail"
echo "  foc release upload $tag $output_dir/$updater_artifact --repo vitaliytv/mymail"
echo "  foc release upload $tag $output_dir/$updater_artifact.sig --repo vitaliytv/mymail"
echo "  foc release upload $tag $output_dir/SHA256SUMS --repo vitaliytv/mymail"
echo "  foc release upload $tag $output_dir/latest.json --repo vitaliytv/mymail"
