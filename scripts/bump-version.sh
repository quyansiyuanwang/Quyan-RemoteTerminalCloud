#!/usr/bin/env bash
set -euo pipefail

NEW="$1"
OLD="$(tr -d '\r\n' < VERSION)"

if [ -z "$NEW" ]; then echo "Usage: $0 <version>" >&2; exit 1; fi
if [ "$OLD" = "$NEW" ]; then echo "Already at $NEW"; exit 0; fi

sed_inplace() { sed -i "s|$1|$2|g" "$3"; }

echo "$NEW" > VERSION
sed_inplace "version = \"$OLD\""           "version = \"$NEW\""           Cargo.toml
sed_inplace "\"version\": \"$OLD\""        "\"version\": \"$NEW\""        apps/rtc-agent-desktop/package.json
sed_inplace "\"version\": \"$OLD\""        "\"version\": \"$NEW\""        apps/rtc-agent-desktop/package-lock.json
sed_inplace "\"version\": \"$OLD\""        "\"version\": \"$NEW\""        apps/rtc-agent-desktop/src-tauri/tauri.conf.json
sed_inplace "Version=\"$OLD\""             "Version=\"$NEW\""             packaging/windows/wix/RemoteTerminalCloudAgent.wxs
sed_inplace "!define AGENT_VERSION \"$OLD\"" "!define AGENT_VERSION \"$NEW\"" packaging/windows/nsis/agent.nsi
sed_inplace "\"version\": \"$OLD\""        "\"version\": \"$NEW\""        apps/rtc-agent-desktop/public/mock/status.json
sed_inplace "\"agentVersion\": \"$OLD\""   "\"agentVersion\": \"$NEW\""   crates/rtc-agent-runtime/tests/mock_backend.rs

# Sync Cargo.lock so it reflects the new workspace version and can be committed
cargo update --workspace

echo "Bumped $OLD -> $NEW"
