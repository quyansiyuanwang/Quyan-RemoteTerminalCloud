#!/usr/bin/env bash
set -euo pipefail

STAGE_ROOT="${1:?stage root is required}"
OUTPUT_PATH="${2:?output path is required}"
PACKAGE_VERSION="${3:?package version is required}"
TARGET_ARCH="${4:?target arch is required}"

PACKAGE_IDENTIFIER="com.remote-terminal-cloud.agent"
INSTALL_ROOT="/Library/Application Support/RemoteTerminalCloudAgent"
PLIST_TARGET="/Library/LaunchDaemons/com.remote-terminal-cloud.agent.plist"
LOG_ROOT="/Library/Logs/RemoteTerminalCloudAgent"

WORK_ROOT="$(mktemp -d)"
trap 'rm -rf "${WORK_ROOT}"' EXIT

PKG_ROOT="${WORK_ROOT}/root"
PKG_SCRIPTS_ROOT="${WORK_ROOT}/scripts"

mkdir -p \
  "${PKG_ROOT}${INSTALL_ROOT}" \
  "${PKG_ROOT}$(dirname "${PLIST_TARGET}")" \
  "${PKG_ROOT}${LOG_ROOT}" \
  "${PKG_SCRIPTS_ROOT}"

cp -R "${STAGE_ROOT}/." "${PKG_ROOT}${INSTALL_ROOT}/"
install -m 0644 "${STAGE_ROOT}/packaging/macos/agent.config.json" "${PKG_ROOT}${INSTALL_ROOT}/config.json"
install -m 0644 "${STAGE_ROOT}/packaging/macos/agent.env.example" "${PKG_ROOT}${INSTALL_ROOT}/agent.env.example"
install -m 0644 "${STAGE_ROOT}/packaging/macos/com.remote-terminal-cloud.agent.plist" "${PKG_ROOT}${PLIST_TARGET}"

cat > "${PKG_SCRIPTS_ROOT}/postinstall" <<EOF
#!/bin/sh
set -eu

INSTALL_ROOT="${INSTALL_ROOT}"
PLIST_TARGET="${PLIST_TARGET}"
LOG_ROOT="${LOG_ROOT}"

mkdir -p "\${LOG_ROOT}"
chown -R root:wheel "\${INSTALL_ROOT}" "\${LOG_ROOT}" >/dev/null 2>&1 || true
chmod 0755 "\${INSTALL_ROOT}" "\${LOG_ROOT}" >/dev/null 2>&1 || true
chown root:wheel "\${PLIST_TARGET}" >/dev/null 2>&1 || true
chmod 0644 "\${PLIST_TARGET}" >/dev/null 2>&1 || true

echo "Remote Terminal Cloud Agent installed for ${TARGET_ARCH}."
echo "Edit \${INSTALL_ROOT}/config.json, then run:"
echo "  launchctl bootstrap system \${PLIST_TARGET}"
echo "  launchctl enable system/com.remote-terminal-cloud.agent"
EOF

chmod 0755 "${PKG_SCRIPTS_ROOT}/postinstall"
mkdir -p "$(dirname "${OUTPUT_PATH}")"
pkgbuild \
  --root "${PKG_ROOT}" \
  --identifier "${PACKAGE_IDENTIFIER}" \
  --version "${PACKAGE_VERSION}" \
  --install-location "/" \
  --ownership recommended \
  --scripts "${PKG_SCRIPTS_ROOT}" \
  "${OUTPUT_PATH}"