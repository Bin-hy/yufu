# 网关安全整改执行证据

## 环境与 T01

- 分支：codex/gateway-oidc-authorization；保留原有两个 assets/.gitkeep 删除及审查文档。
- rustc 1.97.1；cargo 1.97.1；pnpm 10.33.0；protoc 35.1。
- Docker Desktop 4.68.0，Engine 29.3.1；授权访问 Docker socket 后 client/server 检查退出码0。
- `cargo metadata --locked --format-version 1`：授权写 Cargo 缓存后退出码0，补齐锁定的 WinDivert 子模块及缺失依赖；未升级 Cargo.lock。
- `pnpm install --offline --frozen-lockfile`：退出码0，6个workspace项目依赖已就绪。
- 基础测试 `cargo test -p auth --lib --locked`：退出码0，1 passed，0 failed，0 ignored。此为原有基础库基线，不代表整改测试或全项目通过。

## T02 测试入口

- `cargo test --test mod --no-run --locked`：退出码0，集成测试入口编译成功。
- `cargo test --test mod security_t02 --locked -- --nocapture`：首次编译暴露 SeaORM 2 的原始 Statement 接口差异，已改用 execute_raw/query_one_raw；重跑退出码0，2 passed、0 failed、0 ignored。
- 已实际验证两个独立 PostgreSQL 容器的数据隔离，以及模拟设备的实际请求计数。
- 补齐可注入时钟夹具后再次执行相同命令：退出码0，3 passed、0 failed、0 ignored、48 filtered out。新增用例用同一夹具验证安全预算与授权新鲜期，包含时间回拨及同步状态失败。T02 已完成。

## T03 身份契约

- 已先新增身份命名空间、非法身份反序列化、秘密值输出三类测试；首轮退出码101，原因是尚未实现的类型未定义，确认测试未空跑。
- 完成 PrincipalKind、PrincipalRef、TokenUse、IdentityBinding、SecretToken 和安全错误分类；身份构造/反序列化校验保持一致，issuer保留精确字符串，不依赖URL规范化合并身份。
- `cargo test -p auth --lib security_t03 --locked`：退出码0，3 passed、0 failed、0 ignored、1 filtered out。
- `cargo clippy -p auth --lib --tests --locked -- -D warnings`：退出码0。
- `cargo fmt --all -- --check`、`git diff --check`：退出码0。
- T03 已完成。共享契约当前尚未替换业务路由，不能据此宣称任何现有鉴权漏洞已修复。

## T04–T06 资源、预算与授权有效期

- 先新增T04–T06行为测试，再运行 `cargo test -p auth --lib security_t0 --locked`：首轮退出码101，缺少待实现的类型，日志保存在本轮临时测试日志中；实现后退出码0，14 passed、0 failed、0 ignored、1 filtered out。
- T04：ResourceRef 按资源种类要求完整字段，反序列化执行相同校验；规范标识使用resource:v1结构化编码，网关/设备中的分隔符不能混淆目标。授权范围必须显式携带应用和资源选择器，空授权集合是deny-all。
- T05：新增可信时钟接口和SecurityBudget。排队及子调用共享原始单调截止期；外部剩余时间只能缩短预算；实际异步等待超时测试使用暂停的Tokio时钟，无需等待真实2秒。
- T06：源头新鲜期最多5秒，与凭据、父凭据及已继承期限取最小值；预留1秒时钟误差，无过期宽限。本地缓存绑定后还受固定单调截止期约束，时间回拨不恢复已过期结论。
- `cargo test -p auth --lib --locked`：退出码0，15 passed、0 failed、0 ignored、0 filtered out，包含原有基础库测试。
- `cargo clippy -p auth --lib --tests --locked -- -D warnings`：退出码0。
- `cargo fmt --all -- --check`、`git diff --check`：退出码0。
- T04、T05、T06 已完成。当前只完成共享安全契约；服务路由尚未接入，跨实例10秒撤权和完整业务链2秒预算仍未验收。

## 验收状态

checklist.md 全部条目保持未验证。只在取得对应实际证据后勾选，不用本文件中的环境检查替代端到端验收。
