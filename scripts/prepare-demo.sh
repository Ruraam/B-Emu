#!/usr/bin/env bash
# Creates an isolated XDG tree with a few dummy games so the shell is not empty
# when you try B-Emu. Prints the environment variables to export before launch.
set -euo pipefail

cd "$(dirname "$0")/.."

DEMO_DIR="${DEMO_DIR:-/tmp/b-emu-demo}"
ROMS="$DEMO_DIR/roms"
rm -rf "$DEMO_DIR"
mkdir -p "$ROMS" "$DEMO_DIR/config/b-emu" "$DEMO_DIR/data" "$DEMO_DIR/cache"

printf 'demo rom mario' > "$ROMS/Super Mario World (USA) (1990).sfc"
printf 'demo rom zelda' > "$ROMS/The Legend of Zelda (USA) (1998).z64"
printf 'demo rom sonic' > "$ROMS/Sonic the Hedgehog (Europe) (1993).md"
printf 'demo rom' > "$ROMS/notes.txt"

# 1x1 PNG, used as sidecar artwork for the first game.
printf 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGNQuvwOAAL+AeSGuPZQAAAAAElFTkSuQmCC' \
    | base64 -d > "$ROMS/Super Mario World (USA) (1990).png"

cat > "$DEMO_DIR/config/b-emu/config.json" <<EOF
{
  "game_dirs": ["$ROMS"],
  "auto_scan": true,
  "vibration": true,
  "reduced_motion": false
}
EOF

echo "Demo library ready in $ROMS"
echo
echo "Run it from an active Wayland session:"
echo "  export XDG_CONFIG_HOME='$DEMO_DIR/config'"
echo "  export XDG_DATA_HOME='$DEMO_DIR/data'"
echo "  export XDG_CACHE_HOME='$DEMO_DIR/cache'"
echo "  ./target/debug/b-emu"
echo
echo "Or smoke-test it headlessly:"
echo "  XDG_CONFIG_HOME='$DEMO_DIR/config' XDG_DATA_HOME='$DEMO_DIR/data' \\"
echo "  XDG_CACHE_HOME='$DEMO_DIR/cache' bash scripts/smoke-wayland.sh"

if [[ "${1:-}" == "--run" ]]; then
    export XDG_CONFIG_HOME="$DEMO_DIR/config"
    export XDG_DATA_HOME="$DEMO_DIR/data"
    export XDG_CACHE_HOME="$DEMO_DIR/cache"
    exec ./target/debug/b-emu
fi
