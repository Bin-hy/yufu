# cobotusr 鉴权中心

项目提供独立的用户与应用鉴权中心：用户、角色、URI 权限、应用权限、OAuth 2.0 / OpenID Connect、PKCE、会话与刷新令牌、MCP 访问令牌、PDP 策略判断、撤销和操作审计。管理前端采用 Vue 3。

Rust workspace 包含根 CLI、`cobotusr` 和 `libs/auth`，唯一二进制是 `cobotusr`。外部应用先注册客户端和权限，再按明确的角色授权接入；系统不会自动授予业务应用访问权限。

## 产品定位与入口

本仓库按单企业独立部署的统一身份与 SSO 平台交付：issuer 固定，账户中心 `/account/*`、开发者控制台 `/developer/*` 和企业管理控制台 `/admin/*` 属于同一个产品，不另建开放平台服务。应用接入采用企业邀约与审批制，开发者自助管理本人应用，管理员控制生产环境、组与访问分配；不提供公开应用市场、多租户、SAML/LDAP/SCIM 或 MFA。

标准协议入口是 `/.well-known/openid-configuration` 与 `/oidc/*`（授权、token、userinfo、内省、撤销、RP 发起退出），浏览器登录、同意与继续流程在 `/identity/*`。对已登记回调的应用，中心退出会按环境能力投递 back-channel logout；`2xx` 只表示对端接受通知，本地授权撤销在此之前已经生效。

旧 RBAC 管理页已退役：`/users`、`/roles`、`/authority`、`/permissions`、`/recordings` 会重定向到新控制台的账户、用户组、应用与审计页面，后端旧管理接口仅为既有客户端保留兼容。

`examples/oidc-web` 是 `openid-client` 服务端 Web 参考实现（含独立保护 API），`examples/oidc-spa` 是 `oidc-client-ts` 浏览器 SPA 参考实现；两者可作为第三方接入的起点。

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

API 前缀默认 `/api/v1`；应用权限同步为 `/application/sync-permissions`，PDP 为 `/enforce`。MCP 令牌创建请求必须指定已注册应用的 `aud`，用户还须具有相应 `mcp:access` 权限。旧管理路由仍接受登记凭据，但账户、应用与授权的写入统一走身份治理，不再绕过访问代次与审计。

## 验证与构建

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features -- --test-threads=1
cargo test --workspace --doc --all-features
pnpm --filter @cobotweb/cobotusr test
pnpm --filter @cobotweb/cobotusr check
pnpm --filter @cobotweb/cobotusr lint
pnpm --filter './examples/*' check
pnpm --filter './examples/*' test
pnpm build:cobotusr
cargo build --bin cobotusr --features webui --release
cargo machete
cargo audit
```

集成测试使用 testcontainers 创建隔离 PostgreSQL，需要可用的 Docker 兼容容器运行时，不连接本地业务数据库。企业 SSO 的定位、实现计划与逐项验收见 `docs/specs/enterprise-sso-provider/`，尚未完成的缺口集中在 `docs/specs/enterprise-sso-provider/gaps.md`，早期精简验收见 `docs/specs/auth-center-only/`。
