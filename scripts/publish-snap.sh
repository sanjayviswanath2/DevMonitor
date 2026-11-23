#!/usr/bin/env bash
# Build DevMonitor snap and publish it to the Snap Store.
#
# Prerequisites (one-time):
#   1. Create a free Snap Store / Ubuntu One account:
#        https://snapcraft.io/account
#   2. Install tooling (needs sudo):
#        sudo snap install snapcraft --classic
#        sudo snap install lxd   # or: sudo snap install multipass
#        sudo lxd init --auto    # if using LXD
#
# Usage:
#   bash scripts/publish-snap.sh              # build only
#   bash scripts/publish-snap.sh --login      # interactive Snap Store login
#   bash scripts/publish-snap.sh --register   # register name "devmonitor"
#   bash scripts/publish-snap.sh --upload     # upload to edge channel
#   bash scripts/publish-snap.sh --release    # upload + release to stable
#   bash scripts/publish-snap.sh --all        # login → register → build → edge
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NAME="devmonitor"
CHANNEL_EDGE="edge"
CHANNEL_STABLE="stable"

if [[ -f "$HOME/.local/tauri-deps/env.sh" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.local/tauri-deps/env.sh"
fi
if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi
export PATH="$HOME/.local/tauri-deps/root/usr/bin:$HOME/.cargo/bin:$PATH"
export PKG_CONFIG_PATH="$ROOT/src-tauri/packaging/pkgconfig:${PKG_CONFIG_PATH:-}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/src-tauri/target}"

DO_LOGIN=0
DO_REGISTER=0
DO_BUILD=1
DO_UPLOAD=0
DO_RELEASE=0

for arg in "$@"; do
  case "$arg" in
    --login) DO_LOGIN=1 ;;
    --register) DO_REGISTER=1 ;;
    --upload) DO_UPLOAD=1 ;;
    --release) DO_UPLOAD=1; DO_RELEASE=1 ;;
    --all) DO_LOGIN=1; DO_REGISTER=1; DO_BUILD=1; DO_UPLOAD=1 ;;
    --no-build) DO_BUILD=0 ;;
    -h|--help)
      sed -n '2,20p' "$0"
      exit 0
      ;;
    *)
      echo "Unknown option: $arg" >&2
      exit 1
      ;;
  esac
done

need_snapcraft() {
  if ! command -v snapcraft >/dev/null 2>&1; then
    echo "snapcraft is not installed."
    echo "Run:  sudo snap install snapcraft --classic"
    exit 1
  fi
}

prepare_payload() {
  echo "==> Ensuring release binary exists…"
  local bin="$ROOT/src-tauri/target/release/devmonitor"
  if [[ ! -x "$bin" ]]; then
    echo "==> Building release binary…"
    cd "$ROOT"
    npm run build
    (
      cd "$ROOT/src-tauri"
      cargo build --release
    )
  fi
  if [[ ! -x "$bin" ]]; then
    echo "ERROR: missing $bin" >&2
    exit 1
  fi

  echo "==> Staging snap payload…"
  local payload="$ROOT/snap/local/payload"
  rm -rf "$payload"
  mkdir -p "$payload/icons"
  cp -f "$bin" "$payload/devmonitor"
  chmod 755 "$payload/devmonitor"
  cp -f "$ROOT/src-tauri/icons/32x32.png" "$payload/icons/32x32.png"
  cp -f "$ROOT/src-tauri/icons/128x128.png" "$payload/icons/128x128.png"
  cp -f "$ROOT/snap/local/metainfo.xml" "$payload/metainfo.xml"
  cat > "$payload/devmonitor.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Version=1.0
Name=DevMonitor
GenericName=System Monitor
Comment=Linux developer system monitor and environment doctor
Exec=devmonitor
Icon=devmonitor
Terminal=false
Categories=System;Monitor;Development;
StartupNotify=false
StartupWMClass=devmonitor
Keywords=monitor;cpu;memory;ports;developer;doctor;
EOF
}

build_snap() {
  need_snapcraft
  prepare_payload
  cd "$ROOT"
  echo "==> Building snap (this may take several minutes)…"
  # Prefer managed build; fall back to host if providers unavailable.
  if snapcraft pack --destructive-mode 2>/tmp/snapcraft-pack.err; then
    :
  elif snapcraft 2>/tmp/snapcraft-build.err; then
    :
  else
    echo "snapcraft failed. Last errors:" >&2
    tail -40 /tmp/snapcraft-pack.err /tmp/snapcraft-build.err 2>/dev/null || true
    echo >&2
    echo "Tips:" >&2
    echo "  sudo snap install snapcraft --classic" >&2
    echo "  sudo snap install lxd && sudo lxd init --auto" >&2
    echo "  # or: sudo snap install multipass" >&2
    exit 1
  fi

  local snap
  snap="$(ls -1t "$ROOT"/${NAME}_*_amd64.snap 2>/dev/null | head -1 || true)"
  if [[ -z "$snap" ]]; then
    snap="$(ls -1t "$ROOT"/${NAME}_*.snap 2>/dev/null | head -1 || true)"
  fi
  if [[ -z "$snap" || ! -f "$snap" ]]; then
    echo "ERROR: .snap file not found in $ROOT" >&2
    exit 1
  fi
  mkdir -p "$ROOT/installer"
  cp -f "$snap" "$ROOT/installer/"
  echo "==> Snap ready: $snap"
  echo "    Copied to installer/$(basename "$snap")"
  echo "$snap"
}

do_login() {
  need_snapcraft
  echo "==> Snap Store login (opens browser / asks for Ubuntu One credentials)…"
  snapcraft login
  snapcraft whoami || true
}

do_register() {
  need_snapcraft
  echo "==> Registering snap name '$NAME' on the Snap Store…"
  if snapcraft register "$NAME"; then
    echo "Registered '$NAME'."
  else
    echo "Register returned non-zero — name may already be yours. Continuing…"
  fi
  cat <<EOF

IMPORTANT — classic confinement approval
----------------------------------------
DevMonitor uses confinement: classic (required for system monitoring).

Classic packaging notes:
  - snapcraft.yaml enables build-attributes: [enable-patchelf]
  - GTK/WebKit runtime is staged into the snap (not taken from host)
  - Keep the GitHub repo public for store reviewers

Forum request:
  https://forum.snapcraft.io/t/classic-confinement-for-devmonitor/54290

Dashboard:
  https://dashboard.snapcraft.io/snaps/${NAME}/

Until approved, upload to the edge channel only, or publishing to stable
may be held by automated review.

EOF
}

do_upload() {
  need_snapcraft
  local snap
  snap="$(ls -1t "$ROOT"/installer/${NAME}_*.snap "$ROOT"/${NAME}_*.snap 2>/dev/null | head -1 || true)"
  if [[ -z "$snap" || ! -f "$snap" ]]; then
    echo "No .snap found — building first…"
    snap="$(build_snap | tail -1)"
  fi
  local channel="$CHANNEL_EDGE"
  if [[ "$DO_RELEASE" -eq 1 ]]; then
    channel="$CHANNEL_STABLE"
  fi
  echo "==> Uploading $(basename "$snap") → $channel …"
  snapcraft upload --release="$channel" "$snap"
  echo
  echo "Published. Install with:"
  echo "  sudo snap install $NAME --channel=$channel --classic"
  echo
  echo "Store page (after indexing): https://snapcraft.io/$NAME"
}

if [[ "$DO_LOGIN" -eq 1 ]]; then do_login; fi
if [[ "$DO_REGISTER" -eq 1 ]]; then do_register; fi
if [[ "$DO_BUILD" -eq 1 && "$DO_UPLOAD" -eq 0 ]]; then build_snap >/dev/null; fi
if [[ "$DO_UPLOAD" -eq 1 ]]; then do_upload; fi

echo "Done."
