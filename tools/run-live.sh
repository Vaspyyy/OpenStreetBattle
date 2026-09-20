#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
export MAPLIBRE_NATIVE_C_INSTALL_DIR
MAPLIBRE_NATIVE_C_INSTALL_DIR=$(bash tools/setup-maplibre.sh)
exec cargo run --locked -p osb-client --features live-map -- "$@"
