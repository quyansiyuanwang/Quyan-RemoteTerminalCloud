# Remote Terminal Cloud

`Remote Terminal Cloud` 是一个面向外部客户售卖的浏览器远程终端 SaaS。目标设备安装 Agent 后，Agent 主动出站连接平台；浏览器端通过平台建立会话，当前优先支持 `Agent -> 本机 SSH / 本地 Shell`。

## Status

- 当前仓库已切换为 `全 Rust workspace + Tauri`
- 主程序由 `rtc-agent-desktop` 承担，负责窗口、托盘、开机自启、后台 agent 生命周期
- 后台运行时由 `rtc-agentd` 承担
- 安装/配置/Windows 服务辅助由 `rtc-agent-installer` 承担
- 构建、bundle、artifact、桌面安装包编排统一由 `cargo xtask` 承担

## Structure

- `apps/rtc-agentd/` — Rust Agent CLI / runtime 入口
- `apps/rtc-agent-installer/` — Rust 安装/服务管理入口
- `apps/rtc-agent-desktop/` — Tauri + Vue 桌面管理器
- `crates/rtc-agent-*` — Rust 共享协议、配置、平台、运行时、服务、打包 crate
- `xtask/` — Rust 构建与打包入口
- `packaging/` — Windows / Linux / macOS 安装与服务脚手架
- `docs/` — 架构、安全、部署、路线图等文档
- `VERSION` — 当前发布版本号

## Requirements

- `Rust` 1.92+
- `Node.js` 24+

如果需要构建原生安装器，还需要：

- Windows `NSIS`
- Linux `dpkg-deb`
- macOS `pkgbuild`

## Quick Start

1. 配置 Agent 环境变量：

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

2. 运行 Rust CLI：

```bash
cargo run -p rtc-agentd -- status --json
```

如果首次启动时未配置 `RTC_REGISTRATION_TOKEN`，交互式终端下会提示输入 token，并自动保存到默认 `config.json`，后续无需重复填写。

也可以单独运行配置向导：

```bash
cargo run -p rtc-agentd -- configure
cargo run -p rtc-agentd -- conf
```

常用命令：

```bash
cargo run -p rtc-agentd -- --help
cargo run -p rtc-agentd -- version --json
cargo run -p rtc-agentd -- paths --json
cargo run -p rtc-agentd -- config --json
cargo run -p rtc-agentd -- status --json
cargo run -p rtc-agentd -- doctor --json
cargo run -p rtc-agentd -- shells --json
```

Windows 安装版建议直接从开始菜单打开 `Remote Terminal Cloud Agent`，或者使用托盘管理，无需手动进入安装目录寻找 `bin\`。

## Build

基础校验：

```bash
cargo check
```

桌面端前端构建：

```bash
cd apps/rtc-agent-desktop
npm install
npm run build
```

Rust 发布入口：

```bash
cargo xtask build
cargo xtask bundle
cargo xtask artifact
cargo xtask package
```

版本切换也可以直接走统一命令：

```bash
cargo run -p xtask -- version 0.3.1
```

这会自动同步：

- 根目录 `VERSION`
- Rust workspace 版本
- Tauri 桌面端版本
- 前端 `package.json` / `package-lock.json`
- Windows NSIS / WiX 版本元数据
- 测试与 mock 中的产品版本号

可通过环境变量指定目标平台：

```bash
RTC_TARGET_PLATFORM=linux RTC_TARGET_ARCH=x64 cargo xtask build
```

## Runtime Configuration

发布制品中的服务端地址固定内置为 `https://api.qysyw.cn`，安装和首次使用阶段不再要求用户填写服务器地址。

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

如需手动更新 token，可使用 `rtc-agent configure` 或 `rtc-agent conf` 启动交互向导并保存到配置文件。

## Windows Quick Use

Windows 安装完成后，可以直接从开始菜单打开 `Remote Terminal Cloud Agent` 文件夹，其中包含：

- `Remote Terminal Cloud Agent` — 默认打开桌面管理器与托盘主程序
- `Configure Token` — 直接进入 token 配置入口
- `Open Config Folder` — 打开配置目录
- `Open Logs` — 打开日志目录

桌面端默认负责：

- 首次启动引导
- token 保存
- 托盘常驻
- 后台启动/停止 `rtc-agent`
- 开机自启

Windows 服务模式已降为可选兼容模式，不再是默认用户路径。

## Release

版本号由根目录的 `VERSION` 文件提供。

统一发布入口：

```bash
cargo xtask build
cargo xtask bundle
cargo xtask artifact
```

输出目录：

```text
release/remote-terminal-cloud-agent-<version>/
release/artifacts/<platform>-<arch>/
```

当前支持的制品：

- Windows: `zip`、`Tauri NSIS exe`
- Linux: `tar.gz`、`deb`
- macOS: `tar.gz`、`pkg`

默认产物位置：

- Windows bundle archive: `release/artifacts/win32-x64/*.zip`
- Windows desktop installer: `release/artifacts/windows-installers/tauri/nsis/*.exe`
- Linux archive/installer: `release/artifacts/linux-x64/`
- macOS archive/installer: `release/artifacts/darwin-arm64/`

## Windows Packaging

默认推荐的 Windows 出包顺序：

```bash
cargo xtask package
```

这条链现在会强制使用生产环境后端 `https://api.qysyw.cn`，并在构建后做一次产物自检。
本机 `.env`、`RTC_SERVER_BASE_URL`、`RTC_REGISTRATION_TOKEN` 等开发环境变量不会再污染正式打包结果。

默认产物会落在：

```text
release/artifacts/windows-installers/tauri/nsis/
```

如需指定 Tauri bundler 目标架构：

```bash
cargo xtask windows-desktop-bundle \
  --bundles nsis \
  --target x86_64-pc-windows-msvc
```

MSI/WiX 已从默认发布链路下线，当前 Windows 只保留 `Tauri NSIS` 主线。

## CI

当前 CI 会：

- 用 `VERSION` 校验发布 tag
- Windows 默认构建 desktop-first `Tauri NSIS`
- Windows 构建会准备 Node/Rust/NSIS 所需依赖
- 统一通过 `cargo xtask` 编排发布
- 上传 `zip`、`exe`、`deb`、`pkg` 并生成统一 `SHA256SUMS.txt`
- 在 `v*` tag 时自动发布 GitHub Release

## Notes

- Agent 始终是纯出站连接
- 浏览器端 shell 选项来自 Agent 实时上报的 `availableShells`
- `RTC_ENABLED_SHELLS` 用于限制最终上报与开放的 shell 集合
- `build/`、`release/` 都是本地产物目录，可随时清理
