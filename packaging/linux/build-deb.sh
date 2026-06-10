#!/usr/bin/env bash
set -euo pipefail

STAGE_ROOT="${1:?stage root is required}"
OUTPUT_PATH="${2:?output path is required}"
PACKAGE_VERSION_RAW="${3:?package version is required}"
TARGET_ARCH_RAW="${4:?target arch is required}"

PACKAGE_NAME="remote-terminal-cloud-agent"
INSTALL_ROOT="/opt/remote-terminal-cloud-agent"
CONFIG_ROOT="/etc/remote-terminal-cloud-agent"
SYSTEMD_UNIT="/etc/systemd/system/remote-terminal-cloud-agent.service"

map_debian_arch() {
  case "$1" in
    x64)
      echo "amd64"
      ;;
    arm64)
      echo "arm64"
      ;;
    armv7l)
      echo "armhf"
      ;;
    *)
      echo "$1"
      ;;
  esac
}

normalize_debian_version() {
  printf '%s' "$1" | sed 's/-/~/1; s/+/./g; s/-/./g'
}

PACKAGE_ARCH="$(map_debian_arch "${TARGET_ARCH_RAW}")"
PACKAGE_VERSION="$(normalize_debian_version "${PACKAGE_VERSION_RAW}")"

WORK_ROOT="$(mktemp -d)"
trap 'rm -rf "${WORK_ROOT}"' EXIT

DEB_ROOT="${WORK_ROOT}/root"
DEBIAN_ROOT="${DEB_ROOT}/DEBIAN"

mkdir -p \
  "${DEBIAN_ROOT}" \
  "${DEB_ROOT}${INSTALL_ROOT}" \
  "${DEB_ROOT}${CONFIG_ROOT}" \
  "${DEB_ROOT}$(dirname "${SYSTEMD_UNIT}")"

cp -R "${STAGE_ROOT}/." "${DEB_ROOT}${INSTALL_ROOT}/"
install -m 0644 "${STAGE_ROOT}/packaging/linux/agent.config.json" "${DEB_ROOT}${CONFIG_ROOT}/config.json"
install -m 0644 "${STAGE_ROOT}/packaging/linux/agent.env.example" "${DEB_ROOT}${CONFIG_ROOT}/agent.env.example"
install -m 0644 "${STAGE_ROOT}/packaging/linux/remote-terminal-cloud-agent.service" "${DEB_ROOT}${SYSTEMD_UNIT}"

cat > "${DEBIAN_ROOT}/control" <<EOF
Package: ${PACKAGE_NAME}
Version: ${PACKAGE_VERSION}
Section: admin
Priority: optional
Architecture: ${PACKAGE_ARCH}
Maintainer: Remote Terminal Cloud
Description: Outbound remote terminal cloud agent with bundled Node.js runtime.
EOF

cat > "${DEBIAN_ROOT}/conffiles" <<EOF
${CONFIG_ROOT}/config.json
${CONFIG_ROOT}/agent.env.example
EOF

cat > "${DEBIAN_ROOT}/postinst" <<'EOF'
#!/bin/sh
set -eu

SERVICE_NAME="remote-terminal-cloud-agent"
SERVICE_USER="remote-terminal-agent"
SERVICE_GROUP="remote-terminal-agent"
INSTALL_ROOT="/opt/remote-terminal-cloud-agent"
CONFIG_ROOT="/etc/remote-terminal-cloud-agent"
STATE_ROOT="/var/lib/remote-terminal-cloud-agent"
LOG_ROOT="/var/log/remote-terminal-cloud-agent"

NOLOGIN_SHELL="$(command -v nologin || true)"
if [ -z "${NOLOGIN_SHELL}" ]; then
  NOLOGIN_SHELL="/bin/false"
fi

if ! getent group "${SERVICE_GROUP}" >/dev/null 2>&1; then
  groupadd --system "${SERVICE_GROUP}" >/dev/null 2>&1 || true
fi

if ! id -u "${SERVICE_USER}" >/dev/null 2>&1; then
  useradd --system --home-dir "${INSTALL_ROOT}" --no-create-home --shell "${NOLOGIN_SHELL}" --gid "${SERVICE_GROUP}" "${SERVICE_USER}" >/dev/null 2>&1 || true
fi

mkdir -p "${CONFIG_ROOT}" "${STATE_ROOT}" "${LOG_ROOT}"

if [ ! -f "${CONFIG_ROOT}/agent.env" ] && [ -f "${CONFIG_ROOT}/agent.env.example" ]; then
  cp "${CONFIG_ROOT}/agent.env.example" "${CONFIG_ROOT}/agent.env"
fi

chown -R "${SERVICE_USER}:${SERVICE_GROUP}" "${STATE_ROOT}" "${LOG_ROOT}" >/dev/null 2>&1 || true
chmod 0750 "${STATE_ROOT}" "${LOG_ROOT}" >/dev/null 2>&1 || true

if command -v systemctl >/dev/null 2>&1; then
  systemctl daemon-reload >/dev/null 2>&1 || true
fi

echo "Remote Terminal Cloud Agent installed."
echo "Edit ${CONFIG_ROOT}/config.json and optionally ${CONFIG_ROOT}/agent.env, then run:"
echo "  systemctl enable --now ${SERVICE_NAME}"
EOF

cat > "${DEBIAN_ROOT}/prerm" <<'EOF'
#!/bin/sh
set -eu

SERVICE_NAME="remote-terminal-cloud-agent"

if command -v systemctl >/dev/null 2>&1; then
  case "${1:-}" in
    remove)
      systemctl disable --now "${SERVICE_NAME}" >/dev/null 2>&1 || true
      ;;
    upgrade|deconfigure)
      systemctl stop "${SERVICE_NAME}" >/dev/null 2>&1 || true
      ;;
  esac
fi
EOF

cat > "${DEBIAN_ROOT}/postrm" <<'EOF'
#!/bin/sh
set -eu

if command -v systemctl >/dev/null 2>&1; then
  systemctl daemon-reload >/dev/null 2>&1 || true
fi
EOF

chmod 0755 "${DEBIAN_ROOT}/postinst" "${DEBIAN_ROOT}/prerm" "${DEBIAN_ROOT}/postrm"
mkdir -p "$(dirname "${OUTPUT_PATH}")"
dpkg-deb --build --root-owner-group "${DEB_ROOT}" "${OUTPUT_PATH}"