# cobotusr 鉴权中心

项目提供独立的用户与应用鉴权中心：用户、角色、URI 权限、应用权限、OAuth 2.0 / OpenID Connect、PKCE、会话与刷新令牌、MCP 访问令牌、PDP 策略判断、撤销和操作审计。管理前端采用 Vue 3。

Rust workspace 包含根 CLI、`cobotusr` 和 `libs/auth`，唯一二进制是 `cobotusr`。外部应用先注册客户端和权限，再按明确的角色授权接入；系统不会自动授予业务应用访问权限。

## 本机开发

```sh
pnpm install --frozen-lockfile
pnpm build:cobotusr
cp conf/cobotusr.example.toml conf/cobotusr.local.toml
# 配置数据库、管理员和签名密钥后启动
cargo run --bin cobotusr --features webui -- --config conf/cobotusr.local.toml --allow-insecure-loopback-cookies
```

原有本地鉴权配置 `conf/cobotrix.local/cobotusr.toml` 仍可用于 `--config`，其中旧业务客户端 bootstrap 项已清理，其他鉴权配置和密钥保留。

`--allow-insecure-loopback-cookies` 仅适用于本机 HTTP，要求监听地址与 issuer 都为 loopback。生产使用 HTTPS 和默认 Secure Cookie。启动前会检查认证时钟；独立诊断使用 `--check-security-clock`，运行期间 `/api/v1/health/ready` 报告就绪状态。

开发前端可运行 `pnpm dev:cobotusr`，Vite 将 API 代理到本机鉴权服务。`--features webui` 嵌入 `assets/cobotusr`，因此先构建前端才能得到完整管理页面。

## 数据库与部署

`compose.yaml` 仅提供鉴权 PostgreSQL，默认本机端口 5433，凭据应按部署环境配置。`conf/cobotusr.service` 是独立服务模板。

启动执行版本化数据库迁移。旧 IM / 默认业务授权迁移保留版本标记，新的数据库不创建这些业务表或默认业务授权。已有库的历史数据、已注册客户端及授权不会自动删除，设备和网关执行凭据不再被认证入口接受。数据库与配置中的历史业务条目需由管理员根据实际归属清理。

OIDC discovery：`/.well-known/openid-configuration`。API 前缀默认 `/api/v1`；应用权限同步为 `/application/sync-permissions`，PDP 为 `/enforce`。MCP 令牌创建请求必须指定已注册应用的 `aud`，用户还须具有相应 `mcp:access` 权限。

## 验证与构建

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features -- --test-threads=1
pnpm --filter @cobotweb/cobotusr test
pnpm --filter @cobotweb/cobotusr check
pnpm --filter @cobotweb/cobotusr lint
pnpm build:cobotusr
cargo build --bin cobotusr --features webui --release
```

集成测试使用 testcontainers 创建隔离 PostgreSQL，需要可用的 Docker 兼容容器运行时，不连接本地业务数据库。具体精简验收见 `docs/specs/auth-center-only/`。
