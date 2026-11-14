#!/usr/bin/env bash
set -euo pipefail

if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo is required" >&2
    exit 127
fi

features="$(cargo tree -e features)"
if grep -Eiq 'backend-winit-x11|winit feature "x11"|x11-dl' <<<"$features"; then
    echo "error: an X11 backend feature was found in the dependency graph" >&2
    grep -Ein 'backend-winit-x11|winit feature "x11"|x11-dl' <<<"$features" >&2
    exit 1
fi

if ! grep -q 'backend-winit-wayland' Cargo.toml; then
    echo "error: the Wayland-only Winit backend is not enabled" >&2
    exit 1
fi

echo "Wayland feature audit passed: no X11 backend feature detected."
