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

## Packaging foundation

当前仓库已补充安装交付基础骨架，但**仍未产出正式 `msi/pkg/deb/rpm`**：

- `scripts/build-release-bundle.mjs`：生成跨平台发布目录骨架
- `packaging/windows/`：Windows Service 安装/卸载脚本模板
- `packaging/linux/`：`systemd` unit 与安装/卸载脚本模板
- `packaging/macos/`：`launchd` plist 与安装/卸载脚本模板

构建发布目录：

1. 在产品根目录执行 `pnpm build:bundle`
2. 输出目录位于 `release/`
3. 后续可在此基础上继续接入 `msi/pkg/deb/rpm` 打包器与代码签名流程

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
