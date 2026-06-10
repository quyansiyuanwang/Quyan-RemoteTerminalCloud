#!/usr/bin/env bash
set -euo pipefail

PLIST_TARGET="/Library/LaunchDaemons/com.remote-terminal-cloud.agent.plist"

echo "Prepare macOS launchd uninstall"
echo "Recommended commands:"
echo "  launchctl bootout system ${PLIST_TARGET} || true"
echo "  rm -f ${PLIST_TARGET}"