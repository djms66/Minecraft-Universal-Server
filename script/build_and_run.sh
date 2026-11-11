#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-dev}"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

case "$MODE" in
  dev) npm run tauri dev ;;
  build)
    if [[ "$(uname -s)" == "Darwin" ]]; then
      # The .app is the directly usable Mac application; DMG creation is a
      # separate packaging step and is not needed for local development.
      npm run tauri build -- --bundles app
    else
      npm run tauri build
    fi
    ;;
  frontend) npm run build ;;
  *) echo "usage: $0 [dev|build|frontend]" >&2; exit 2 ;;
esac
