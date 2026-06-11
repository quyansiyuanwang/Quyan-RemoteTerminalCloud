# Architecture

## Target topology

`Browser Terminal -> Session Gateway -> Agent Outbound Tunnel -> Local SSH/PTy`

## Current implementation slice

当前已经切换到 `Rust workspace + Tauri` 的产品架构：

- `rtc-agent-protocol`：统一协议类型与 JSON 结构
- `rtc-agent-config`：配置加载、默认路径、token 持久化
- `rtc-agent-platform`：平台识别、shell 与 SSH 探测
- `rtc-agent-runtime`：注册、心跳、WebSocket 隧道、PTY 与会话生命周期
- `rtc-agent-service`：服务模式适配层
- `rtc-agent-packaging`：构建、bundle、artifact、NSIS、MSI 编排
- `rtc-agentd`：Agent CLI / runtime
- `rtc-agent-installer`：安装、配置、服务辅助
- `rtc-agent-desktop`：桌面窗口、托盘、开机自启、后台 agent 管理
- `xtask`：统一构建与发布入口

## Planned modules

- `apps/rtc-agentd/`：后台 agent 运行时与 CLI
- `apps/rtc-agent-installer/`：平台安装与控制入口
- `apps/rtc-agent-desktop/`：桌面产品界面与托盘主程序
- `crates/rtc-agent-*`：共享能力模块
- `NodeBackend/`：设备管理 API、注册令牌、会话网关、审计服务
- `Frontend/`：设备列表、在线终端、会话列表、审计中心

## Design constraints

- Agent 必须保持纯出站连接，不要求被控机暴露公网端口
- 控制面与数据面分离：HTTP 管控制，`WebSocket` 管终端字节流
- 平台差异由 Agent 适配层承接，不能泄漏到业务编排主流程
- 桌面端负责用户体验与管理，不承载协议主循环
- 打包与安装编排统一由 Rust 管理，脚本只允许保留为薄包装层
