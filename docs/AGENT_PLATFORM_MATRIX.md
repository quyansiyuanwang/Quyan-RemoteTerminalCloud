# Agent Platform Matrix

| Platform | Install | Startup | MVP terminal mode | Phase 2 terminal mode | Key risks |
| --- | --- | --- | --- | --- | --- |
| `Windows` | `MSI` / `EXE` | `Windows Service` | Forward local `OpenSSH Server` | `PowerShell` / `cmd` PTY | `UAC`、Defender、服务权限、中文路径 |
| `Linux` | `deb` / `rpm` / binary | `systemd` | Forward local `sshd` | `/bin/bash` / `/bin/sh` PTY | `glibc`、SELinux、发行版差异、只读文件系统 |
| `macOS` | `pkg` / signed helper | `launchd` | Forward local `Remote Login(OpenSSH)` | `zsh` / `sh` PTY | notarization、Full Disk Access、Intel/ARM 差异 |

## Stable capability keys

- `sshForward`
- `nativePty`
- `selfUpdate`
- `proxyAware`
- `serviceManaged`
- `sessionRecording`

## MVP verification baseline

- `Windows + OpenSSH Server`
- `Linux + sshd`
- `macOS + Remote Login`
