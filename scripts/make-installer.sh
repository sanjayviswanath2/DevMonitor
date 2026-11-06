#!/usr/bin/env bash
# Build a release .deb installer with login autostart enabled.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="$ROOT/installer"

if [[ -f "$HOME/.local/tauri-deps/env.sh" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.local/tauri-deps/env.sh"
fi
if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/src-tauri/target}"
# Help tauri-cli detect appindicator for bundling (runtime .so is on the system).
export PKG_CONFIG_PATH="$ROOT/src-tauri/packaging/pkgconfig:${PKG_CONFIG_PATH:-}"
cd "$ROOT"

echo "==> Building DevMonitor release .deb…"
set +e
npm run tauri -- build --bundles deb
BUILD_RC=$?
set -e

BUNDLE_DIR="$(ls -1td "$ROOT"/src-tauri/target/release/bundle/deb/DevMonitor_*_amd64 2>/dev/null | head -1 || true)"
BIN="$ROOT/src-tauri/target/release/devmonitor"
if [[ ! -x "$BIN" ]]; then
  echo "ERROR: release binary missing (build rc=$BUILD_RC)" >&2
  exit 1
fi

# If bundling aborted after compile, assemble a minimal package tree from the binary.
if [[ ! -d "$BUNDLE_DIR/data" || ! -d "$BUNDLE_DIR/control" ]]; then
  BUNDLE_DIR="$ROOT/src-tauri/target/release/bundle/deb/DevMonitor_0.1.0_amd64"
  echo "==> Assembling deb tree manually at $BUNDLE_DIR"
  mkdir -p \
    "$BUNDLE_DIR/data/usr/bin" \
    "$BUNDLE_DIR/data/usr/share/applications" \
    "$BUNDLE_DIR/data/etc/xdg/autostart" \
    "$BUNDLE_DIR/data/usr/share/icons/hicolor/32x32/apps" \
    "$BUNDLE_DIR/data/usr/share/icons/hicolor/128x128/apps" \
    "$BUNDLE_DIR/control"
  cp -f "$BIN" "$BUNDLE_DIR/data/usr/bin/devmonitor"
  chmod 755 "$BUNDLE_DIR/data/usr/bin/devmonitor"
  cp -f "$ROOT/src-tauri/packaging/autostart.desktop" \
    "$BUNDLE_DIR/data/etc/xdg/autostart/DevMonitor.desktop"
  cat > "$BUNDLE_DIR/data/usr/share/applications/DevMonitor.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Version=1.0
Name=DevMonitor
Comment=DevMonitor — lightweight Linux developer system monitor
Exec=devmonitor
Icon=devmonitor
Terminal=false
Categories=System;Monitor;Development;
StartupNotify=false
StartupWMClass=devmonitor
EOF
  cp -f "$ROOT/src-tauri/icons/32x32.png" \
    "$BUNDLE_DIR/data/usr/share/icons/hicolor/32x32/apps/devmonitor.png"
  cp -f "$ROOT/src-tauri/icons/128x128.png" \
    "$BUNDLE_DIR/data/usr/share/icons/hicolor/128x128/apps/devmonitor.png"
  cat > "$BUNDLE_DIR/control/control" <<'EOF'
Package: dev-monitor
Version: 0.1.0
Architecture: amd64
Installed-Size: 9000
Maintainer: DevMonitor
Priority: optional
Depends: libwebkit2gtk-4.1-0, libgtk-3-0, libayatana-appindicator3-1 | libappindicator3-1
Description: Linux developer system monitor
 DevMonitor watches CPU, memory, ports, and developer activity. After install it
 starts automatically on login. Closing the window keeps it running in the tray.
EOF
  cp -f "$ROOT/src-tauri/packaging/postinst.sh" "$BUNDLE_DIR/control/postinst"
  cp -f "$ROOT/src-tauri/packaging/postrm.sh" "$BUNDLE_DIR/control/postrm"
  chmod 755 "$BUNDLE_DIR/control/postinst" "$BUNDLE_DIR/control/postrm"
else
  # Refresh binary into existing tree (covers partial-bundle cases).
  cp -f "$BIN" "$BUNDLE_DIR/data/usr/bin/devmonitor"
  chmod 755 "$BUNDLE_DIR/data/usr/bin/devmonitor"
fi

BASENAME="$(basename "$BUNDLE_DIR").deb"
mkdir -p "$OUT_DIR"

# Tauri's ar packing can produce a corrupt .deb on some hosts; rebuild with dpkg-deb.
REPACK="$(mktemp -d)"
trap 'rm -rf "$REPACK"' EXIT
cp -a "$BUNDLE_DIR/data/." "$REPACK/"
mkdir -p "$REPACK/DEBIAN"
cp -a "$BUNDLE_DIR/control/." "$REPACK/DEBIAN/"
chmod 755 "$REPACK/DEBIAN/"* 2>/dev/null || true
find "$REPACK/DEBIAN" -type f ! -name 'post*' ! -name 'pre*' -exec chmod 644 {} \;
chmod 755 "$REPACK/DEBIAN/postinst" "$REPACK/DEBIAN/postrm" 2>/dev/null || true
dpkg-deb --build "$REPACK" "$OUT_DIR/$BASENAME"
cp -f "$OUT_DIR/$BASENAME" "$BUNDLE_DIR.deb"

cat > "$OUT_DIR/INSTALL.txt" <<EOF
DevMonitor installer
====================

File: $BASENAME

Install (Ubuntu / Debian):
  sudo apt install -y ./$BASENAME

  # or:
  sudo dpkg -i ./$BASENAME
  sudo apt-get install -f -y

After install:
  • App is available as "DevMonitor" in your app menu
  • It starts automatically when you log in after reboot / power off-on
  • Closing the window keeps it running in the system tray (background)
  • Tray menu: Open DevMonitor / Quit DevMonitor
  • Autostart entry: /etc/xdg/autostart/DevMonitor.desktop

Launch now:
  devmonitor

Disable autostart (optional):
  sudo rm -f /etc/xdg/autostart/DevMonitor.desktop

Uninstall:
  sudo apt remove -y dev-monitor
EOF

cat > "$OUT_DIR/install.sh" <<EOF
#!/usr/bin/env bash
set -euo pipefail
cd "\$(dirname "\$0")"
DEB="$BASENAME"
if [[ ! -f "\$DEB" ]]; then
  echo "Missing \$DEB" >&2
  exit 1
fi
echo "Installing DevMonitor (autostart on login enabled)…"
sudo apt install -y "./\$DEB" || {
  sudo dpkg -i "./\$DEB"
  sudo apt-get install -f -y
}
echo
echo "Installed. Starting DevMonitor…"
nohup devmonitor >/dev/null 2>&1 &
echo "Done. It will also start automatically after every reboot / power-on (when you log in)."
EOF
chmod +x "$OUT_DIR/install.sh"

echo
echo "Installer ready:"
echo "  $OUT_DIR/$BASENAME"
echo "  $OUT_DIR/install.sh"
echo
echo "Install with:"
echo "  cd installer && ./install.sh"
