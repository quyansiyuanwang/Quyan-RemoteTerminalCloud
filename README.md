# Remote Terminal Cloud

`Remote Terminal Cloud` 是一个面向外部客户售卖的浏览器 SSH 远控 SaaS。用户在无公网 IP 的设备上安装 Agent，Agent 主动连回平台；浏览器端通过平台发起会话，首版优先支持 `Agent -> 本机 SSH`。

## Current scope

- 当前产品仓库已收敛为 Agent 主项目，核心实现直接位于根级 `src/`
- 已开始实现跨平台 Agent 核心：平台识别、能力模型、SSH 预检查、主机快照输出
- 已补充多平台矩阵、安全、架构、路线图与安装交付文档

## Structure

- `src/` — 多平台 Agent 主逻辑
- `packaging/` — Windows/Linux/macOS 安装与服务脚手架
- `packages/protocol/` — Agent 与平台侧共享协议类型
- `scripts/` — 发布目录生成脚本
- `docs/` — PRD/架构/安全/平台矩阵等产品文档

## Quick start

1. 在当前产品根目录执行 `pnpm install`
2. 先启动主后端 `NodeBackend`（默认 `http://127.0.0.1:10001`）
3. 设置 Agent 环境变量：

   - `RTC_SERVER_BASE_URL=http://127.0.0.1:10001`
   - `RTC_REGISTRATION_TOKEN=dev-registration-token`
   - 可选：`RTC_CONFIG_FILE=/path/to/config.json`
   - 可选：`RTC_DEFAULT_SHELL=system-default`
   - 可选：`RTC_ENABLED_SHELLS=system-default,cmd,powershell,pwsh`（先由 Agent 自动探测，再按此白名单裁剪并上报服务端）
   - 可选：`RTC_DISABLE_HEARTBEAT=1`
   - 可选：`RTC_DISABLE_TUNNEL=1`
   - 可选：`RTC_PREFERENCES_FILE=/path/to/preferences.json`

4. 执行 `pnpm dev`
5. Agent 会输出当前主机的平台快照、注册结果、心跳与隧道日志

## Agent config file

Agent 现在支持“配置文件 + 环境变量覆写”的部署模式，便于后续作为系统服务安装运行。

- Windows 默认配置：`%APPDATA%\remote-terminal-cloud-agent\config.json`
- macOS 默认配置：`~/Library/Application Support/remote-terminal-cloud-agent/config.json`
- Linux 默认配置：`~/.config/remote-terminal-cloud-agent/config.json`

环境变量优先级高于配置文件。配置文件示例：

```json
{
  "serverBaseUrl": "https://your-domain.example.com",
  "registrationToken": "replace-with-real-token",
  "runHeartbeat": true,
  "runTunnel": true,
  "defaultShellType": "system-default",
  "enabledShellTypes": ["system-default", "bash", "pwsh"],
  "preferencesFilePath": "/var/lib/remote-terminal-cloud-agent/preferences.json"
}
```

安装包内置的默认 `config.json` 模板会把 `serverBaseUrl` 与 `registrationToken` 留空；在你填入真实值之前，系统服务会保持存活并自动重试，不会因为占位模板直接退出。

## Packaging foundation

当前仓库已补充安装交付基础骨架，并可生成以下正式安装产物：

- `scripts/build-release-bundle.mjs`：生成跨平台发布目录骨架
- `packaging/windows/`：Windows Service 安装/卸载脚本模板与 `msi` 构建骨架
- `packaging/linux/`：`systemd` unit、安装脚本与 `deb` 构建脚本
- `packaging/macos/`：`launchd` plist、安装脚本与 `pkg` 构建脚本

构建发布目录：

1. 在产品根目录执行 `pnpm build:bundle`
2. 输出目录位于 `release/`
3. 当前可直接继续生成 `zip/tar.gz/msi/deb/pkg`，后续仍可接入 `rpm` 与代码签名流程

## CI multi-platform build

仓库已新增 GitHub Actions 工作流：`.github/workflows/build-multi-platform.yml`

触发方式：

- push 到 `main` / `master`
- push `v*` tag（例如 `v0.1.0`）
- pull request
- 手动 `workflow_dispatch`

当前 CI 行为：

- Linux：构建并上传 `tar.gz` 与 `deb` 平台制品
- macOS：构建并上传 `tar.gz` 与 `pkg` 平台制品
- Windows：构建并上传 `zip` 平台制品，以及基于现有 WiX 骨架生成 `msi`
- 对 `v*` tag：自动创建或更新 GitHub Release，并附带上述制品与 `SHA256SUMS.txt`

Tag release 规则：

- tag 名必须与 `package.json` 版本一致，例如当前版本是 `0.1.0`，则发布 tag 必须是 `v0.1.0`
- 若 tag 与版本不一致，Release 工作流会直接失败，避免错版发布

本地也可直接构建当前平台制品：

1. 执行 `pnpm build:artifact`
2. 输出位于 `release/artifacts/<platform>-<arch>/`
3. 其中包含平台归档包、原生安装包（Linux/macOS）以及解压后的制品目录

## Local integration

- Agent 始终是出站连接；即使目标设备没有公网 IP，也只需要它能访问你的后端。
- 当前开发模式已迁移为“主后端内嵌网关 + 主前端页面”，控制面位于 `NodeBackend/`，页面位于 `Frontend/`。
- 浏览器端远程终端页面在主工程 `Frontend/src/views/relay/RemoteTerminalView.vue`，通过主后端控制面创建会话。
- 浏览器创建会话时的 shell 选项来自 Agent 实时上报的 `availableShells`；Agent 侧 `RTC_DEFAULT_SHELL` 作为 `system-default` 的兜底默认值。
- 如需安装时或后续限制用户可选终端，可通过 `RTC_ENABLED_SHELLS` 配置允许的 shell 集合；服务端只接受 Agent 实际上报的终端能力。

## Implementation status

当前实现聚焦 Agent 基础层，后续优先顺序：

1. 设备注册与心跳协议
2. Agent 出站隧道与会话配对
3. 主后端控制面 API 与主前端设备页持续增强
4. 浏览器终端与审计日志
