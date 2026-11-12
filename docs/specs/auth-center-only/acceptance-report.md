# 鉴权中心精简验收

2026-11-08，Asia/Shanghai。

保留根 CLI、yufu、libs/auth 和管理前端；唯一二进制为 yufu。删除设备、网关、映射、IM、联网组件及对应前端、配置、启动器和测试。开发命令、CI、compose、README、锁文件已同步。共享库保留外部应用的通用资源权限模型，执行消息与设备/网关凭据入口已移除。

MCP 要求明确指定注册应用的 audience，并执行当前权限及原始令牌权限上限检查。新库不创建旧业务表或隐式业务授权；旧迁移版本仍可识别，已有数据库数据不在启动时自动删除。现有本地鉴权配置路径及签名密钥保留，旧服务配置与本地鉴权配置中的退役业务 bootstrap 已清理。

## 验证结果

- `cargo test --workspace --all-features -- --test-threads=1`：45 个 auth 单元测试、32 个 yufu 单元测试、114 个隔离集成用例、8 个文档测试全部通过。1 个既有 require_perm 伪代码文档示例仍显式忽略。
- 集成测试包含真实 Chromium / Firefox / WebKit 登录、刷新、退出，新旧数据库迁移，退役 IM 接口 JSON 404，缺少或未注册 MCP audience 的拒绝，以及权限/会话/刷新/撤销的并发和故障行为。数据库、HTTP 和 NTP 使用隔离测试实例。
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`、`cargo fmt --all -- --check` 通过。
- 前端 6 个测试文件、66 个测试通过；类型检查、lint、构建通过。`pnpm install --frozen-lockfile` 通过。
- `cargo build --bin yufu --features webui` 和 CLI help 检查通过。
- Cargo metadata 仅列出 yufu-root、yufu、auth；二进制仅 yufu。六个退役源目录均不存在。
- TOML / JSON / YAML 配置语法及 Git diff 空白检查通过。

测试未连接用户业务数据库或操作其服务。既有数据与已注册业务应用仍由管理员维护；本地私钥和真实鉴权配置不进入提交。Swagger UI 改用依赖自带资源，避免构建阶段另行下载 ZIP 失败。
