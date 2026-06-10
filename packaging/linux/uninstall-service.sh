#!/usr/bin/env bash
set -euo pipefail

SYSTEMD_UNIT="/etc/systemd/system/remote-terminal-cloud-agent.service"

echo "Prepare Linux service uninstall"
echo "Recommended commands:"
echo "  systemctl disable --now remote-terminal-cloud-agent || true"
echo "  rm -f ${SYSTEMD_UNIT}"
echo "  systemctl daemon-reload"