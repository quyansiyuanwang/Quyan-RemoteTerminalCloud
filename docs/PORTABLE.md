# 便携版使用指南

便携版是单个无依赖可执行文件，不需要安装，适合服务器 CLI 部署、自动化脚本调用、容器环境。

## 下载

从 [GitHub Releases](https://github.com/quyansiyuanwang/Quyan-RemoteTerminalCloud/releases/latest) 下载对应平台的便携版：

| 文件名 | 平台 |
|--------|------|
| `rtc-agent-<ver>-linux-x64` | Linux x86_64 |
| `rtc-agent-<ver>-linux-arm64` | Linux ARM64（树莓派、服务器） |
| `rtc-agent-<ver>-darwin-arm64` | macOS Apple Silicon |
| `rtc-agent-<ver>-darwin-x64` | macOS Intel |
| `rtc-agent-<ver>-win32-x64.exe` | Windows x64 |
| `rtc-agent-<ver>-win32-arm64.exe` | Windows ARM64 |

## 快速上手

### Linux / macOS

```bash
# 赋予执行权限
chmod +x rtc-agent-<ver>-linux-x64

# 重命名方便调用（可选）
mv rtc-agent-<ver>-linux-x64 rtc-agent

# 首次配置 token（交互式向导）
./rtc-agent configure

# 或者直接通过环境变量运行
RTC_REGISTRATION_TOKEN=your-token ./rtc-agent run
```

### Windows

```powershell
# 重命名方便调用（可选）
Rename-Item rtc-agent-<ver>-win32-x64.exe rtc-agent.exe

# 首次配置 token（交互式向导）
.\rtc-agent.exe configure

# 或者通过环境变量运行
$env:RTC_REGISTRATION_TOKEN = "your-token"
.\rtc-agent.exe run
```

## 常用命令

```bash
rtc-agent                    # 默认：启动 agent 持续运行（前台）
rtc-agent run                # 同上，显式前台持续运行
rtc-agent once               # 运行一次后退出
rtc-agent start              # 后台无窗口运行（类服务模式，写 PID 文件）
rtc-agent stop               # 停止后台运行的 agent
rtc-agent install-path       # 将当前 rtc-agent 所在目录注册到系统 PATH
rtc-agent configure          # 交互式配置向导（保存 token 到配置文件）
rtc-agent status --json      # 查看当前状态
rtc-agent version --json     # 查看版本信息
rtc-agent config --json      # 查看当前配置
rtc-agent paths --json       # 查看配置/日志文件路径
rtc-agent doctor --json      # 诊断连接与环境问题
rtc-agent shells --json      # 列出可用 shell
```

## 配置方式

便携版支持三种配置方式，优先级从高到低：

**1. 环境变量（最高优先级）**

```bash
export RTC_REGISTRATION_TOKEN=your-token
export RTC_DEFAULT_SHELL=system-default
export RTC_ENABLED_SHELLS=system-default,bash,pwsh
export RTC_DISABLE_HEARTBEAT=0
export RTC_DISABLE_TUNNEL=0
```

**2. 配置文件**

默认路径：
- Linux: `~/.config/remote-terminal-cloud-agent/config.json`
- macOS: `~/Library/Application Support/remote-terminal-cloud-agent/config.json`
- Windows: `%APPDATA%\remote-terminal-cloud-agent\config.json`

可通过 `RTC_CONFIG_FILE` 指定自定义路径：

```bash
RTC_CONFIG_FILE=/etc/rtc-agent/config.json ./rtc-agent run
```

配置文件格式：

```json
{
  "registrationToken": "your-token",
  "runHeartbeat": true,
  "runTunnel": true,
  "defaultShellType": "system-default",
  "enabledShellTypes": ["system-default", "bash", "pwsh"]
}
```

**3. 交互式向导（最简单）**

首次运行时若无 token，终端下会自动提示输入并保存：

```bash
./rtc-agent
# → 提示：请输入 Registration Token:
```

也可以主动启动向导：

```bash
./rtc-agent configure
```

## 后台运行（类服务模式）

便携版内置后台运行支持，无需依赖系统服务管理器：

```bash
# 启动后台运行（无黑窗/无控制台输出）
rtc-agent start

# 停止后台运行
rtc-agent stop
```

- PID 文件写入配置目录（`rtc-agent.pid`），重复调用 `start` 会检测是否已在运行。
- Windows：进程以 `CREATE_NO_WINDOW | DETACHED_PROCESS` 标志启动，不会出现黑色控制台窗口。
- Linux / macOS：标准输出/错误重定向到日志目录下的 `rtc-agent.log`。

## 注册 PATH（方便全局调用 rtc-agent）

```bash
# Windows（写入 HKCU\Environment\Path，无需管理员权限）
rtc-agent install-path

# macOS / Linux（追加 export PATH 到 ~/.zshrc, ~/.bashrc, ~/.profile）
./rtc-agent install-path
```

注册后打开新终端即可直接使用 `rtc-agent start`、`rtc-agent stop` 等命令，无需输入完整路径。



便携版本身不负责服务注册，但可以配合系统工具实现开机自启：

### Linux (systemd)

```ini
# /etc/systemd/system/rtc-agent.service
[Unit]
Description=Remote Terminal Cloud Agent
After=network.target

[Service]
ExecStart=/usr/local/bin/rtc-agent run
Restart=on-failure
RestartSec=10
Environment=RTC_REGISTRATION_TOKEN=your-token

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl enable --now rtc-agent
```

### macOS (launchd)

参考 `packaging/macos/com.remote-terminal-cloud.agent.plist`，将 `ProgramArguments` 改为便携版路径。

### Windows (NSSM / Task Scheduler)

使用 [NSSM](https://nssm.cc/) 注册为服务：

```powershell
nssm install RtcAgent "C:\path\to\rtc-agent.exe" run
nssm set RtcAgent AppEnvironmentExtra RTC_REGISTRATION_TOKEN=your-token
nssm start RtcAgent
```

## 注意事项

- 便携版与安装版共用同一套配置文件路径，可以并存
- Agent 始终是**纯出站连接**，不需要开放任何入站端口
- 服务端地址已内置为 `https://api.qysyw.cn`，无需配置
