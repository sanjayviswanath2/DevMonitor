#!/bin/sh
set -e

# On purge/remove, drop system autostart entry.
if [ "$1" = "remove" ] || [ "$1" = "purge" ]; then
  rm -f /etc/xdg/autostart/DevMonitor.desktop
fi

exit 0
