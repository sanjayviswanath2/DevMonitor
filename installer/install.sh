#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
DEB="DevMonitor_0.1.0_amd64.deb"
if [[ ! -f "$DEB" ]]; then
  echo "Missing $DEB" >&2
  exit 1
fi
echo "Installing DevMonitor (autostart on login enabled)…"
sudo apt install -y "./$DEB" || {
  sudo dpkg -i "./$DEB"
  sudo apt-get install -f -y
}
echo
echo "Installed. Starting DevMonitor…"
nohup devmonitor >/dev/null 2>&1 &
echo "Done. It will also start automatically after every reboot / power-on (when you log in)."
