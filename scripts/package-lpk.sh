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

# Keep ULPM architecture names explicit. Never label a native build as a
# different architecture: Cargo must compile the matching Rust target.
case "$ARCH" in
    amd64)
        RUST_TARGET="x86_64-unknown-linux-gnu"
        EXPECTED_CLASS="ELF64"
        EXPECTED_MACHINE="Advanced Micro Devices X86-64"
        ;;
    arm64)
        RUST_TARGET="aarch64-unknown-linux-gnu"
        EXPECTED_CLASS="ELF64"
        EXPECTED_MACHINE="AArch64"
        ;;
    armhf)
        RUST_TARGET="armv7-unknown-linux-gnueabihf"
        EXPECTED_CLASS="ELF32"
        EXPECTED_MACHINE="ARM"
        ;;
    *)
        echo "Unsupported ULPM architecture: $ARCH (supported: amd64, arm64, armhf)" >&2
        exit 1
        ;;
esac

[[ -n "$VERSION" ]] || {
    echo "Could not read package version from Cargo.toml." >&2
    exit 1
}

for tool in "$ULPM" cargo jq readelf; do
    command -v "$tool" >/dev/null || {
        echo "Required command not found: $tool" >&2
        exit 1
    }
done

[[ -f packaging/manifest.json ]] || {
    echo "Missing packaging/manifest.json." >&2
    exit 1
}

echo "Building release binary for $ARCH ($RUST_TARGET)..."
cargo build --release --bin b-emu --target "$RUST_TARGET"

BINARY="target/$RUST_TARGET/release/b-emu"
[[ -f "$BINARY" && -x "$BINARY" ]] || {
    echo "Expected executable was not produced: $BINARY" >&2
    exit 1
}

ELF_HEADER="$(readelf -h "$BINARY")"
ACTUAL_CLASS="$(awk -F: '/^[[:space:]]*Class:/ {gsub(/^[[:space:]]+|[[:space:]]+$/, "", $2); print $2}' <<< "$ELF_HEADER")"
ACTUAL_MACHINE="$(awk -F: '/^[[:space:]]*Machine:/ {sub(/^[[:space:]]+/, "", $2); print $2}' <<< "$ELF_HEADER")"

[[ "$ACTUAL_CLASS" == "$EXPECTED_CLASS" && "$ACTUAL_MACHINE" == "$EXPECTED_MACHINE" ]] || {
    echo "Binary architecture mismatch for $ARCH." >&2
    echo "Expected: $EXPECTED_CLASS / $EXPECTED_MACHINE" >&2
    echo "Actual:   $ACTUAL_CLASS / $ACTUAL_MACHINE" >&2
    exit 1
}

rm -rf "$STAGE"
mkdir -p "$STAGE/meta" "$STAGE/bin"

jq --arg version "$VERSION" --arg arch "$ARCH" \
    '.version = $version | .arch = $arch' \
    packaging/manifest.json > "$STAGE/meta/manifest.json.tmp"
mv "$STAGE/meta/manifest.json.tmp" "$STAGE/meta/manifest.json"

install -m 0755 "$BINARY" "$STAGE/bin/b-emu"

echo "Packaging $OUT (version $VERSION, $ARCH)..."
"$ULPM" pack "$STAGE" b-emu "$OUT" "$VERSION"
"$ULPM" inspect "$OUT"
