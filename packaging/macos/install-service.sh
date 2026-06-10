#!/usr/bin/env bash
set -euo pipefail

INSTALL_ROOT="/Library/Application Support/RemoteTerminalCloudAgent"
LOG_ROOT="/Library/Logs/RemoteTerminalCloudAgent"
PLIST_TARGET="/Library/LaunchDaemons/com.remote-terminal-cloud.agent.plist"

echo "Prepare macOS launchd installation"
echo "Install root: ${INSTALL_ROOT}"

mkdir -p "${INSTALL_ROOT}" "${LOG_ROOT}"

if [[ ! -f "${INSTALL_ROOT}/config.json" ]]; then
	cp "$(dirname "$0")/agent.config.json" "${INSTALL_ROOT}/config.json"
fi

cp "$(dirname "$0")/agent.env.example" "${INSTALL_ROOT}/agent.env.example"
cp "$(dirname "$0")/com.remote-terminal-cloud.agent.plist" "${PLIST_TARGET}"

chown -R root:wheel "${INSTALL_ROOT}" "${LOG_ROOT}"
chmod 644 "${PLIST_TARGET}"
chown root:wheel "${PLIST_TARGET}"

echo "Installed launchd plist template to ${PLIST_TARGET}"
echo "Next step: place runtime under ${INSTALL_ROOT}, then run:"
echo "  launchctl bootstrap system ${PLIST_TARGET}"
echo "  launchctl enable system/com.remote-terminal-cloud.agent"