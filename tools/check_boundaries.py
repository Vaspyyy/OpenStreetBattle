#!/usr/bin/env python3
"""Fail if the headless dependency tree pulls in graphical or HTTP runtimes."""
import subprocess

result = subprocess.run(
    ["cargo", "tree", "-p", "osb-headless", "-e", "normal", "--prefix", "none"],
    check=True, capture_output=True, text=True,
)
names = {line.split()[0] for line in result.stdout.splitlines() if line.strip()}
forbidden = {"bevy_render", "bevy_winit", "wgpu", "winit", "egui", "wayland-client", "x11-dl", "reqwest"}
found = sorted(names & forbidden)
if found:
    raise SystemExit(f"Headless boundary violation: {', '.join(found)}")
print("Headless dependency boundary: clean (no window, renderer or network client).")
