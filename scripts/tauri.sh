#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
# Optional local pkg-config prefix used when system -dev packages aren't installed
if [[ -f "$HOME/.local/tauri-deps/env.sh" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.local/tauri-deps/env.sh"
fi
if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi
cd "$ROOT"
exec npm run tauri -- "$@"
