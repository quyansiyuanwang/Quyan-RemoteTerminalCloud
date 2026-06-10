# Architecture

## Target topology

`Browser Terminal -> Session Gateway -> Agent Outbound Tunnel -> Local SSH/PTy`

## Current implementation slice

当前已开始实现的是 `Agent Platform Core`：

- 统一平台识别：`Windows` / `Linux` / `macOS`
- 统一能力模型：`sshForward`、`nativePty`、`selfUpdate`、`proxyAware` 等
- 统一主机快照：设备名、平台、架构、Agent 版本、诊断信息
- 统一 SSH 预检查：在真正做设备注册前先知道目标机是否具备 MVP 条件

## Planned modules

- `cmd/rtc-agent/`：Agent 启动入口
- `internal/agent/`：Agent 注册、心跳、隧道、会话执行、升级器
- `internal/protocol/`：Agent 运行时共享协议类型
- `NodeBackend/`：设备管理 API、注册令牌、会话网关、审计服务
- `Frontend/`：设备列表、在线终端、会话列表、审计中心

## Design constraints

- Agent 必须保持纯出站连接，不要求被控机暴露公网端口
- 控制面与数据面分离：HTTP 管控制，`WebSocket` 管终端字节流
- 平台差异由 Agent 适配层承接，不能泄漏到业务编排主流程
