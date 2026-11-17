#!/usr/bin/env bash
# Starts a nested Weston on the current X11 display and runs B-Emu from the
# project files inside it, with a small demo library. No .lpk needed.
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ -z "${DISPLAY:-}" ]]; then
    echo "DISPLAY is not set: this script needs an X11 session to host nested Weston." >&2
    echo "On a native Wayland session, run ./target/debug/b-emu directly instead." >&2
    exit 1
fi

bash scripts/prepare-demo.sh

export XDG_CONFIG_HOME=/tmp/b-emu-demo/config
export XDG_DATA_HOME=/tmp/b-emu-demo/data
export XDG_CACHE_HOME=/tmp/b-emu-demo/cache
export XDG_RUNTIME_DIR=/tmp/b-emu-weston-x11
export WAYLAND_DISPLAY=wayland-b-emu-interactive

rm -rf "$XDG_RUNTIME_DIR"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"

weston \
    --backend=x11 \
    --socket="$WAYLAND_DISPLAY" \
    --shell=desktop-shell.so \
    --idle-time=0 \
    --log="$XDG_RUNTIME_DIR/weston.log" &
weston_pid=$!
cleanup() {
    kill "$weston_pid" 2>/dev/null || true
    wait "$weston_pid" 2>/dev/null || true
}
trap cleanup EXIT

for _ in $(seq 1 50); do
    [[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]] && break
    sleep 0.1
done
if [[ ! -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]]; then
    echo "Nested Weston did not create its socket; see $XDG_RUNTIME_DIR/weston.log" >&2
    exit 1
fi

echo "B-Emu is running inside the Weston window on your desktop."
echo "Close the B-Emu window (or press Ctrl+C here) to stop everything."

if [[ -n "${SECONDS_ALIVE:-}" ]]; then
    set +e
    env -u DISPLAY LIBGL_ALWAYS_SOFTWARE=1 \
        timeout --signal=TERM "${SECONDS_ALIVE}s" ./target/debug/b-emu
    set -e
else
    env -u DISPLAY LIBGL_ALWAYS_SOFTWARE=1 ./target/debug/b-emu
fi
