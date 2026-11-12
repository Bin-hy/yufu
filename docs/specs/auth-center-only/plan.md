# 实施方案

Rust workspace 仅包含根 CLI、yufu、libs/auth；唯一二进制 yufu。前端仅保留 yufu。移除 IM 路由与后台任务，删除消息执行契约。资源策略类型保留为外部应用的鉴权模型。旧 IM 和默认业务应用 seed 改为兼容空迁移，已有业务数据不自动删除。MCP 目标改为显式注册应用。
