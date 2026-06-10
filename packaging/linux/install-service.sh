#!/usr/bin/env bash
set -euo pipefail

INSTALL_ROOT="/opt/remote-terminal-cloud-agent"
CONFIG_ROOT="/etc/remote-terminal-cloud-agent"
SYSTEMD_UNIT="/etc/systemd/system/remote-terminal-cloud-agent.service"

echo "Prepare Linux service installation"
echo "Install root: ${INSTALL_ROOT}"
echo "Config root: ${CONFIG_ROOT}"

mkdir -p "${CONFIG_ROOT}"

if [[ ! -f "${CONFIG_ROOT}/agent.env" ]]; then
  cp "$(dirname "$0")/agent.env.example" "${CONFIG_ROOT}/agent.env"
fi

cp "$(dirname "$0")/remote-terminal-cloud-agent.service" "${SYSTEMD_UNIT}"

echo "Installed systemd unit template to ${SYSTEMD_UNIT}"
echo "Next step: create service user, place runtime under ${INSTALL_ROOT}, then run:"
echo "  systemctl daemon-reload"
echo "  systemctl enable --now remote-terminal-cloud-agent"