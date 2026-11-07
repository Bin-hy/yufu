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
- 已实际验证两个独立 PostgreSQL 容器的数据隔离，以及模拟设备的实际请求计数。可注入时钟夹具将在预算接口建立时补齐；T02 保持进行中，不提前标完整完成。

## T03 身份契约

- 已先新增身份命名空间、非法身份反序列化、秘密值输出三类测试；首轮退出码101，原因是尚未实现的类型未定义，确认测试未空跑。
- 完成 PrincipalKind、PrincipalRef、TokenUse、IdentityBinding、SecretToken 和安全错误分类；身份构造/反序列化校验保持一致，issuer保留精确字符串，不依赖URL规范化合并身份。
- `cargo test -p auth --lib security_t03 --locked`：退出码0，3 passed、0 failed、0 ignored、1 filtered out。
- `cargo clippy -p auth --lib --tests --locked -- -D warnings`：退出码0。
- `cargo fmt --all -- --check`、`git diff --check`：退出码0。
- T03 已完成。共享契约当前尚未替换业务路由，不能据此宣称任何现有鉴权漏洞已修复。

## 验收状态

checklist.md 全部条目保持未验证。只在取得对应实际证据后勾选，不用本文件中的环境检查替代端到端验收。
