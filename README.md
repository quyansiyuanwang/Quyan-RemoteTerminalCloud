# Remote Terminal Cloud

`Remote Terminal Cloud` 是一个面向外部客户售卖的浏览器远程终端 SaaS。目标设备安装 Agent 后，Agent 主动出站连接平台；浏览器端通过平台建立会话，当前优先支持 `Agent -> 本机 SSH / 本地 Shell`。

## Status

- 当前仓库已完全切换为纯 `Go` Agent 项目
- 原来的顶层 TypeScript Agent、`pnpm` 工作区、Node 发布脚本已移除
- 当前仓库只保留 Go 源码、平台打包脚本和 CI 发布链

## Structure

- `cmd/rtc-agent/` — Agent 启动入口
- `cmd/rtc-release/` — 发布目录与平台制品构建入口
- `internal/agent/` — Agent 运行时主逻辑
- `internal/protocol/` — Agent 内部共享协议类型
- `internal/buildinfo/` — 版本与构建信息
- `packaging/` — Windows / Linux / macOS 安装与服务脚手架
- `docs/` — 架构、安全、部署、路线图等文档
- `VERSION` — 当前发布版本号

## Requirements

- `Go` 1.26+

如果需要构建原生安装器，还需要：

- Windows `NSIS` / `WiX`
- Linux `dpkg-deb`
- macOS `pkgbuild`

## Quick Start

1. 启动你的后端控制面，默认开发地址示例：

```text
http://localhost:10001
```

2. 配置 Agent 环境变量：

```env
RTC_REGISTRATION_TOKEN=dev-registration-token
RTC_DEFAULT_SHELL=system-default
RTC_ENABLED_SHELLS=system-default,cmd,powershell,pwsh
RTC_DISABLE_HEARTBEAT=0
RTC_DISABLE_TUNNEL=0
```

常用可选项：

- `RTC_CONFIG_FILE=/path/to/config.json`
- `RTC_PREFERENCES_FILE=/path/to/preferences.json`

3. 本地启动 Agent：

```bash
go run ./cmd/rtc-agent
```

## Build

构建当前平台 Agent：

```bash
go build ./...
```

运行基础校验：

```bash
go test ./...
```

构建当前目标平台二进制：

```bash
go run ./cmd/rtc-release build
```

可通过环境变量指定目标平台：

```bash
RTC_TARGET_PLATFORM=linux RTC_TARGET_ARCH=x64 go run ./cmd/rtc-release build
```

## Runtime Configuration

Agent 的服务端地址不再由用户配置：

- 本地开发直接运行 `go run ./cmd/rtc-agent` 时固定连接 `http://localhost:10001`
- 通过 `go run ./cmd/rtc-release build|bundle|artifact` 生成的发布二进制固定连接 `https://api.qysyw.cn`

其余运行参数继续使用“环境变量优先，配置文件兜底”的配置模式。

默认配置文件位置：

- Windows: `%APPDATA%\remote-terminal-cloud-agent\config.json`
- macOS: `~/Library/Application Support/remote-terminal-cloud-agent/config.json`
- Linux: `~/.config/remote-terminal-cloud-agent/config.json`

可覆盖的环境变量：

- `RTC_CONFIG_FILE`
- `RTC_REGISTRATION_TOKEN`
- `RTC_DEFAULT_SHELL`
- `RTC_ENABLED_SHELLS`
- `RTC_DISABLE_HEARTBEAT`
- `RTC_DISABLE_TUNNEL`
- `RTC_PREFERENCES_FILE`

配置文件示例：

```json
{
  "registrationToken": "replace-with-real-token",
  "runHeartbeat": true,
  "runTunnel": true,
  "defaultShellType": "system-default",
  "enabledShellTypes": ["system-default", "bash", "pwsh"],
  "preferencesFilePath": "/var/lib/remote-terminal-cloud-agent/preferences.json"
}
```

## Release

版本号由根目录的 `VERSION` 文件提供。

生成发布 bundle：

```bash
go run ./cmd/rtc-release bundle
```

输出目录：

```text
release/remote-terminal-cloud-agent-<version>/
```

生成当前目标平台制品：

```bash
go run ./cmd/rtc-release artifact
```

输出目录：

```text
release/artifacts/<platform>-<arch>/
```

当前支持的制品：

- Windows: `zip`、`NSIS exe`、`WiX msi`
- Linux: `tar.gz`、`deb`
- macOS: `tar.gz`、`pkg`

所有发布制品中的 Agent 二进制都会内置 `https://api.qysyw.cn`，安装阶段不再要求输入服务器地址。

## Windows Packaging

准备 NSIS 安装器输入目录：

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\nsis\prepare-installer-stage.ps1 `
  -AgentBundleRoot "D:\path\to\remote-terminal-cloud-agent-0.1.0" `
  -Force
```

构建 NSIS 安装器：

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\nsis\build-installer.ps1 `
  -AgentBuildRoot "D:\path\to\remote-terminal-cloud-agent-0.1.0\artifacts\windows\installer-build-root"
```

准备 WiX MSI 输入目录：

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\wix\prepare-msi-stage.ps1 `
  -AgentBundleRoot "D:\path\to\remote-terminal-cloud-agent-0.1.0" `
  -Force
```

构建 WiX MSI：

```powershell
powershell -ExecutionPolicy Bypass -File packaging\windows\wix\build-msi.ps1 `
  -AgentBuildRoot "D:\path\to\remote-terminal-cloud-agent-0.1.0\artifacts\windows\msi-build-root" `
  -OutputDir "D:\path\to\remote-terminal-cloud-agent-0.1.0\artifacts\windows\msi-build-root\artifacts\windows\out"
```

## CI

当前仓库包含：

- `.github/workflows/build-multi-platform.yml`

当前 CI 会：

- 用 `VERSION` 校验发布 tag
- 用 Go 构建 Linux / macOS / Windows 平台制品
- 上传归档包与原生安装包
- 在 `v*` tag 时自动发布 GitHub Release

## Notes

- Agent 始终是纯出站连接
- 浏览器端 shell 选项来自 Agent 实时上报的 `availableShells`
- `RTC_ENABLED_SHELLS` 用于限制最终上报与开放的 shell 集合
- `build/`、`release/` 都是本地产物目录，可随时清理
