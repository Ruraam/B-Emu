#!/usr/bin/env bash
# Headless Wayland startup smoke test.
# Override COMPOSITOR_CMD to validate another compositor (BubbleWM, cage, ...).
set -euo pipefail

cd "$(dirname "$0")/.."

RUNTIME_DIR="${RUNTIME_DIR:-/tmp/opencode/b-emu-smoke-runtime}"
DISPLAY_NAME="${DISPLAY_NAME:-wayland-b-emu-smoke}"
TARGET="${TARGET:-binary}"          # binary | lpk
LPK="${LPK:-target/b-emu_amd64.lpk}"
SECONDS_ALIVE="${SECONDS_ALIVE:-8}"

COMPOSITOR_CMD="${COMPOSITOR_CMD:-weston --backend=headless --renderer=pixman --socket=$DISPLAY_NAME --idle-time=0 --no-config --log=$RUNTIME_DIR/compositor.log}"

rm -rf "$RUNTIME_DIR"
mkdir -p "$RUNTIME_DIR"
chmod 700 "$RUNTIME_DIR"
export XDG_RUNTIME_DIR="$RUNTIME_DIR"
export WAYLAND_DISPLAY="$DISPLAY_NAME"

# shellcheck disable=SC2086
$COMPOSITOR_CMD &
compositor_pid=$!
cleanup() {
    kill "$compositor_pid" 2>/dev/null || true
    wait "$compositor_pid" 2>/dev/null || true
}
trap cleanup EXIT

for _ in $(seq 1 50); do
    [[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]] && break
    sleep 0.1
done
if [[ ! -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]]; then
    echo "Compositor did not create its Wayland socket." >&2
    exit 1
fi

if [[ "$TARGET" == "lpk" ]]; then
    launch=(ulpm run "$LPK")
elif [[ "$TARGET" == "binary" ]]; then
    launch=(./target/debug/b-emu)
else
    echo "Unknown TARGET: $TARGET (use binary or lpk)" >&2
    exit 1
fi

set +e
env -u DISPLAY LIBGL_ALWAYS_SOFTWARE=1 \
    timeout --signal=TERM --kill-after=2s "${SECONDS_ALIVE}s" \
    "${launch[@]}" > "$RUNTIME_DIR/app.log" 2>&1
status=$?
set -e

echo "--- application log ---"
cat "$RUNTIME_DIR/app.log"
if [[ "$status" -ne 124 ]]; then
    echo "FAILED: application exited before the smoke window (status $status)." >&2
    exit 1
fi
echo "Wayland smoke OK (target=$TARGET, compositor=$COMPOSITOR_CMD)"
