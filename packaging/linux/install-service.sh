#!/usr/bin/env bash
set -euo pipefail

INSTALL_ROOT="/opt/remote-terminal-cloud-agent"
CONFIG_ROOT="/etc/remote-terminal-cloud-agent"
STATE_ROOT="/var/lib/remote-terminal-cloud-agent"
LOG_ROOT="/var/log/remote-terminal-cloud-agent"
SYSTEMD_UNIT="/etc/systemd/system/remote-terminal-cloud-agent.service"
SERVICE_USER="remote-terminal-agent"
SERVICE_GROUP="remote-terminal-agent"

NOLOGIN_SHELL="$(command -v nologin || true)"
if [[ -z "${NOLOGIN_SHELL}" ]]; then
  NOLOGIN_SHELL="/bin/false"
fi

echo "Prepare Linux service installation"
echo "Install root: ${INSTALL_ROOT}"
echo "Config root: ${CONFIG_ROOT}"

if ! getent group "${SERVICE_GROUP}" >/dev/null 2>&1; then
  groupadd --system "${SERVICE_GROUP}"
fi

if ! id -u "${SERVICE_USER}" >/dev/null 2>&1; then
  useradd --system --home-dir "${INSTALL_ROOT}" --no-create-home --shell "${NOLOGIN_SHELL}" --gid "${SERVICE_GROUP}" "${SERVICE_USER}"
fi

mkdir -p "${CONFIG_ROOT}" "${STATE_ROOT}" "${LOG_ROOT}"

if [[ ! -f "${CONFIG_ROOT}/config.json" ]]; then
  cp "$(dirname "$0")/agent.config.json" "${CONFIG_ROOT}/config.json"
fi

if [[ ! -f "${CONFIG_ROOT}/agent.env" ]]; then
  cp "$(dirname "$0")/agent.env.example" "${CONFIG_ROOT}/agent.env"
fi

cp "$(dirname "$0")/agent.env.example" "${CONFIG_ROOT}/agent.env.example"

cp "$(dirname "$0")/remote-terminal-cloud-agent.service" "${SYSTEMD_UNIT}"
chown -R "${SERVICE_USER}:${SERVICE_GROUP}" "${STATE_ROOT}" "${LOG_ROOT}"

echo "Installed systemd unit template to ${SYSTEMD_UNIT}"
echo "Next step: place runtime under ${INSTALL_ROOT}, edit ${CONFIG_ROOT}/config.json if needed, then run:"
echo "  systemctl daemon-reload"
echo "  systemctl enable --now remote-terminal-cloud-agent"