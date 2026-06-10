#!/usr/bin/env bash
set -euo pipefail

INSTALL_ROOT="/Library/Application Support/RemoteTerminalCloudAgent"
PLIST_TARGET="/Library/LaunchDaemons/com.remote-terminal-cloud.agent.plist"

echo "Prepare macOS launchd installation"
echo "Install root: ${INSTALL_ROOT}"

cp "$(dirname "$0")/com.remote-terminal-cloud.agent.plist" "${PLIST_TARGET}"

echo "Installed launchd plist template to ${PLIST_TARGET}"
echo "Next step: place runtime under ${INSTALL_ROOT}, then run:"
echo "  launchctl bootstrap system ${PLIST_TARGET}"
echo "  launchctl enable system/com.remote-terminal-cloud.agent"