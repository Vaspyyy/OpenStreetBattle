#!/usr/bin/env bash
# Install the exact native artifact matching the pinned Rust binding. No sudo.
set -euo pipefail
if [[ $(uname -s) != Linux || $(uname -m) != x86_64 ]]; then
  echo 'The first prebuilt MapLibre integration supports Linux x86_64. Use the offline client on other targets.' >&2
  exit 1
fi
revision=6f7998eec595560c0359ed033519cbab1f7c9aeb
digest=a2467bd331b8432614c5424160af47b34451d05fc7735d1be8eb4572dbc0fb19
root="${XDG_CACHE_HOME:-$HOME/.cache}/openstreetbattle/native"
prefix="$root/$revision"
mkdir -p "$root"
exec 9>"$root/.install.lock"
flock 9
if [[ ! -f "$prefix/.osb-verified-$digest" ]]; then
  if [[ -e "$prefix" ]]; then
    echo "Unverified installation already exists: $prefix. Move it aside before reinstalling." >&2
    exit 1
  fi
  tmp=$(mktemp -d "$root/.install-XXXXXX")
  trap 'rm -rf "$tmp"' EXIT
  echo 'Fetching the pinned MapLibre Native Vulkan artifact...' >&2
  curl --fail --location --connect-timeout 15 --max-time 180 --retry 1 \
    -H 'Accept: application/octet-stream' \
    'https://api.github.com/repos/maplibre/maplibre-native-ffi/releases/assets/574254252' -o "$tmp/native.tar.gz" >&2
  printf '%s  %s\n' "$digest" "$tmp/native.tar.gz" | sha256sum --check >&2
  mkdir "$tmp/prefix"
  tar --extract --gzip --file "$tmp/native.tar.gz" --directory "$tmp/prefix" --no-same-owner
  python3 - "$tmp/prefix" "$revision" <<'PY'
import json, pathlib, sys
prefix = pathlib.Path(sys.argv[1])
meta = json.loads((prefix / 'share/maplibre-native-c/artifact.json').read_text())
assert meta['gitSha'] == sys.argv[2], 'Native source revision mismatch'
assert meta['renderBackend'] == 'vulkan', 'Expected Vulkan artifact'
assert meta['targetPlatform'] == 'linux-gnu-x64', 'Expected Linux x86_64 artifact'
assert (prefix / 'lib/libmaplibre-native-c.a').is_file(), 'Missing native static archive'
PY
  touch "$tmp/prefix/.osb-verified-$digest"
  mv "$tmp/prefix" "$prefix"
fi
printf '%s\n' "$prefix"
