#!/usr/bin/env bash
set -euo pipefail
mkdir -p artifacts
export XDG_RUNTIME_DIR="$(mktemp -d)"
chmod 700 "$XDG_RUNTIME_DIR"
export WAYLAND_DISPLAY=wayland-osb
export XDG_SESSION_TYPE=wayland
export WINIT_UNIX_BACKEND=wayland
export WGPU_BACKEND=vulkan
unset DISPLAY
weston --backend=headless-backend.so --socket="$WAYLAND_DISPLAY" --width=1440 --height=900 --idle-time=0 --use-pixman > artifacts/weston.log 2>&1 &
compositor=$!
trap 'kill "$compositor" 2>/dev/null || true; rm -rf "$XDG_RUNTIME_DIR"' EXIT
for _ in $(seq 1 50); do
  test -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" && break
  sleep 0.1
done
if ! test -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY"; then cat artifacts/weston.log; exit 1; fi
timeout 90s target/debug/osb-client --software-renderer --smoke-frames 100 --screenshot artifacts/client.png > artifacts/client.log 2>&1 || { cat artifacts/client.log; exit 1; }
cat artifacts/client.log
grep -q 'OSB_SMOKE_OK' artifacts/client.log
test -s artifacts/client.png
