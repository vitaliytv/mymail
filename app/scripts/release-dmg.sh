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
mount_dir=
mounted=0

cleanup() {
  cp "$backup" "$config"
  rm -f "$backup"
  if [[ $mounted == 1 ]]; then
    hdiutil detach "$mount_dir" -quiet || true
  fi
  [[ -z $mount_dir ]] || rmdir "$mount_dir" 2>/dev/null || true
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
bun --cwd=app run tauri build --target universal-apple-darwin --bundles dmg

bundle_dir=target/universal-apple-darwin/release/bundle/dmg
dmg=$(find "$bundle_dir" -maxdepth 1 -type f -name '*.dmg' -print -quit)
[[ -n $dmg ]] || fail "Tauri did not create a DMG in $bundle_dir"
[[ -f "$dmg.sig" ]] || fail "Tauri did not create an updater signature for $dmg"

mkdir -p "$output_dir"
artifact="MyMail_${version}_universal.dmg"
cp "$dmg" "$output_dir/$artifact"
cp "$dmg.sig" "$output_dir/$artifact.sig"

mount_dir=$(mktemp "${TMPDIR:-/tmp}/mymail-dmg.XXXXXX")
hdiutil attach "$dmg" -nobrowse -readonly -mountpoint "$mount_dir" >/dev/null
mounted=1
app_bundle=$(find "$mount_dir" -maxdepth 2 -type d -name '*.app' -print -quit)
[[ -n $app_bundle ]] || fail "DMG does not contain an application bundle"
codesign --verify --deep --strict --verbose=2 "$app_bundle"
hdiutil detach "$mount_dir" -quiet
mounted=0
rmdir "$mount_dir"
mount_dir=

(
  cd "$output_dir"
  shasum -a 256 "$artifact" "$artifact.sig" > SHA256SUMS
)

node - "$version" "$artifact" "$output_dir/$artifact.sig" "$output_dir/latest.json" <<'NODE'
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
echo "  foc release upload $tag $output_dir/$artifact --repo vitaliytv/mymail"
echo "  foc release upload $tag $output_dir/$artifact.sig --repo vitaliytv/mymail"
echo "  foc release upload $tag $output_dir/SHA256SUMS --repo vitaliytv/mymail"
echo "  foc release upload $tag $output_dir/latest.json --repo vitaliytv/mymail"
