#!/usr/bin/env bash
# Builds a signed ULPM .lpk for B-Emu. This is a release tool: the development
# binary has no runtime dependency on ULPM or any compositor-specific service.
set -euo pipefail

cd "$(dirname "$0")/.."

ULPM="${ULPM:-ulpm}"
ARCH="${ARCH:-amd64}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
STAGE="target/lpk-staging"
OUT="${OUT:-target/b-emu_${ARCH}.lpk}"

command -v "$ULPM" >/dev/null || {
    echo "ULPM CLI not found: $ULPM (set ULPM=/path/to/ulpm)" >&2
    exit 1
}

rm -rf "$STAGE"
mkdir -p "$STAGE/meta" "$STAGE/bin"

cp packaging/manifest.json "$STAGE/meta/manifest.json"
if command -v jq >/dev/null; then
    jq --arg version "$VERSION" --arg arch "$ARCH" \
        '.version = $version | .arch = $arch' \
        "$STAGE/meta/manifest.json" > "$STAGE/meta/manifest.json.tmp"
    mv "$STAGE/meta/manifest.json.tmp" "$STAGE/meta/manifest.json"
fi

echo "Building release binary..."
cargo build --release --bin b-emu
install -m 0755 target/release/b-emu "$STAGE/bin/b-emu"

echo "Packaging $OUT (version $VERSION, $ARCH)..."
"$ULPM" pack "$STAGE" b-emu "$OUT" "$VERSION"
"$ULPM" inspect "$OUT"
