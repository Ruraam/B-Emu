#!/usr/bin/env bash
# Guards the architecture rule: emulator launch commands must be built from
# argument arrays, never from shell strings.
set -euo pipefail

cd "$(dirname "$0")/.."

pattern='sh[[:space:]]+-c|bash[[:space:]]+-c|/bin/sh|/bin/bash|Command::new\(format!|Command::new\(&format!|\.args\(format!'

if grep -R -n -E "$pattern" src --include='*.rs'; then
    echo "Launch safety audit failed: shell invocation or formatted command detected." >&2
    exit 1
fi

echo "Launch safety audit passed: no shell command concatenation."
