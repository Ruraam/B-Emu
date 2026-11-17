#!/usr/bin/env bash
# Builds a signed package and a minimal signed ULPM repository.
# ULPM is a release tool only; nothing here is needed to build or run the app.
set -euo pipefail

cd "$(dirname "$0")/.."

ULPM="${ULPM:-ulpm}"
ARCH="${ARCH:-amd64}"
OUT="${OUT:-target/b-emu_${ARCH}.lpk}"
REPO_DIR="${REPO_DIR:-target/repo}"
KEY_DIR="${KEY_DIR:-target/release-keys}"
RELEASE_KEY="${RELEASE_KEY:-$KEY_DIR/b-emu-release.priv}"

command -v "$ULPM" >/dev/null || {
    echo "ULPM CLI not found: $ULPM (set ULPM=/path/to/ulpm)" >&2
    exit 1
}

ARCH="$ARCH" OUT="$OUT" ULPM="$ULPM" bash scripts/package-lpk.sh

mkdir -p "$KEY_DIR" "$REPO_DIR"
if [[ ! -f "$RELEASE_KEY" ]]; then
    echo "Generating a release key in $KEY_DIR..."
    # Isolated XDG environment: keygen must not touch the user's ULPM config.
    XDG_CONFIG_HOME="$KEY_DIR/xdg-config" \
    XDG_DATA_HOME="$KEY_DIR/xdg-data" \
    XDG_CACHE_HOME="$KEY_DIR/xdg-cache" \
    XDG_STATE_HOME="$KEY_DIR/xdg-state" \
        "$ULPM" keygen b-emu-release
    cp "$KEY_DIR/xdg-config/ulpm/keys/b-emu-release.priv" "$RELEASE_KEY"
    cp "$KEY_DIR/xdg-config/ulpm/keys/b-emu-release.pub" "${RELEASE_KEY%.priv}.pub"
    chmod 600 "$RELEASE_KEY"
fi

PUBLIC_KEY="${RELEASE_KEY%.priv}.pub"
[[ -f "$PUBLIC_KEY" ]] || {
    echo "Missing public key: $PUBLIC_KEY" >&2
    exit 1
}

cp "$OUT" "$REPO_DIR/"
cp "$PUBLIC_KEY" "$REPO_DIR/release_key.pub"
"$ULPM" sign "$REPO_DIR/$(basename "$OUT")" "$RELEASE_KEY"
"$ULPM" repo index "$REPO_DIR" "$RELEASE_KEY"

echo
echo "Repository ready: $REPO_DIR"
echo "Distribute $REPO_DIR/release_key.pub out of band, then:"
echo "  ulpm repo add b-emu <repo-url> <path-to-release_key.pub>"
echo "  ulpm install io.lpk.b-emu"
