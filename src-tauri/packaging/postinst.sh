#!/bin/sh
set -e

# Ensure system-wide graphical autostart (login after reboot / power on).
AUTOSTART_DIR="/etc/xdg/autostart"
DESKTOP_SRC="/usr/share/applications/DevMonitor.desktop"
AUTOSTART_DST="$AUTOSTART_DIR/DevMonitor.desktop"

mkdir -p "$AUTOSTART_DIR"

if [ -f "$AUTOSTART_DST" ]; then
  :
elif [ -f "$DESKTOP_SRC" ]; then
  # Fallback if packaging files map was skipped for any reason
  cat > "$AUTOSTART_DST" <<'EOF'
[Desktop Entry]
Type=Application
Version=1.0
Name=DevMonitor
Comment=DevMonitor — start after login so monitoring continues across restarts
Exec=/usr/bin/devmonitor
Icon=devmonitor
Terminal=false
Categories=System;Monitor;
StartupNotify=false
X-GNOME-Autostart-enabled=true
X-GNOME-Autostart-Delay=2
Hidden=false
NoDisplay=false
EOF
fi

# Refresh desktop/icon caches when tools are available (non-fatal).
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -q /usr/share/icons/hicolor 2>/dev/null || true
fi

exit 0
