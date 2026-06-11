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

如果首次启动时未配置 `RTC_REGISTRATION_TOKEN`，交互式终端下会提示输入 token，并自动保存到默认 `config.json`，后续无需重复填写。

也可以单独运行配置向导：

```bash
go run ./cmd/rtc-agent configure
go run ./cmd/rtc-agent conf
```

常用 CLI 命令：

```bash
go run ./cmd/rtc-agent help
go run ./cmd/rtc-agent help status
go run ./cmd/rtc-agent version
go run ./cmd/rtc-agent paths
go run ./cmd/rtc-agent config
go run ./cmd/rtc-agent status
go run ./cmd/rtc-agent doctor
go run ./cmd/rtc-agent shells
```

Windows 安装版建议直接使用开始菜单中的 `Remote Terminal Cloud Agent` 管理入口，不必手动进入安装目录找 `bin\rtc-agent.exe`。

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

在交互式终端直接运行 Agent 时，如果 `registrationToken` 和 `RTC_REGISTRATION_TOKEN` 都为空，程序会提示输入 token，并将其写回配置文件；Windows 服务等非交互环境仍保持自动重试，不会阻塞启动。

如需手动更新 token，可使用 `rtc-agent configure` 或 `rtc-agent conf` 启动交互向导并保存到配置文件。

CLI 提供了一组便于排查的本地命令：

- `rtc-agent help` — 查看全部命令
- `rtc-agent help <command>` — 查看单个命令说明
- `rtc-agent version` — 查看版本与内置服务端地址
- `rtc-agent paths` — 查看配置文件、偏好文件和当前工作目录
- `rtc-agent config` — 查看生效中的运行配置，不输出 token 明文
- `rtc-agent status` — 查看主机、shell、SSH、token 等当前状态
- `rtc-agent doctor` — 输出本地诊断摘要和建议
- `rtc-agent shells` — 查看 shell 配置与探测结果

## Windows Quick Use

Windows 安装完成后，可以直接从开始菜单打开 `Remote Terminal Cloud Agent` 文件夹，其中包含：

- `Agent Manager` — 打开独立的图形管理器程序，无需命令行，可查看状态和执行常用操作
- `Configure Agent` — 直接进入 token 配置向导，会打开可见窗口用于安全输入 token
- `Open Config Folder` — 打开配置目录
- `Open Logs` — 打开日志目录

如果运行 `rtc-agent configure` 或 `rtc-agent conf`，输入 token 时字符会显示为 `*`，这样用户能看到正在输入，但不会明文暴露 token。

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
