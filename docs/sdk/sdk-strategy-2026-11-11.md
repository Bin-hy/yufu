# yufu Runtime SDK 化调研：开发解耦与多语言产出方案

> - 状态：调研稿，未实施
> - 基线：`5b8ad81`（2026-11-11）
> - 目标：回答“如何让业务开发与 SDK 解耦”，以及“Rust 服务如何为 Go / Python / Java 接入方提供稳定的 Runtime SDK”
> - 方法：本仓库源码静态阅读 + 公开规范/工具资料调研；未部署外部产品，未运行完整测试
> - 注意：工作区已有未提交的前端改动，本次调研未触碰
>
> **范围决策（2026-11-12）**：只做 **Runtime SDK**；语言顺序 **Go → Python → Java**；**M2M 不做**；Management SDK 不在本期范围。

---

## 0. 一页结论

### 0.1 核心判断

**SDK 不应是 Rust 服务内部实现的一层包装，而应是“版本化 HTTP/JSON 契约 + OAuth/OIDC 标准协议”的独立消费者。**

对本项目，SDK 分为两条产品线，但本期只做第一条：

| 产品线 | 面向对象 | 契约来源 | 建议 |
|---|---|---|---|
| **Runtime SDK** | 接入的 Web 后端、SPA 后端、资源服务器 | OIDC Discovery、JWKS、RFC 6749/7662/7009/9068、本仓库的 token/introspection 语义 | **本期唯一范围**；手写薄封装 + 成熟 JOSE/OIDC 库 |
| **Management SDK** | 管理员/开发者自动化、CI、运维平台 | 管理面 OpenAPI | **本期不做**；当前管理面是浏览器 Cookie + CSRF，不适合服务端 SDK |

**价值判断**：在多团队、多语言持续接入的前提下，SDK 的价值成立。它不是替代标准 OIDC 库，而是把 yufu 的契约差异、安全默认值、backchannel logout 校验和升级维护集中到一个可版本化的交付物里；如果只是少数内部应用，则应优先做文档 + fixtures，而不是三语言 SDK。

### 0.2 已确认决策

| 决策 | 内容 |
|---|---|
| 范围 | 只做 Runtime SDK；Management SDK 不进入本期 |
| 接入场景 | 多团队、多语言持续接入；SDK 按长期产品维护，而非一次性交付 |
| 语言顺序 | Go → Python → Java |
| Runtime 契约 | OIDC Discovery、JWKS、RFC 6749/7662/7009/9068；OpenAPI 不是 Runtime SDK 的前置条件 |
| 非目标 | M2M/`client_credentials`、权限同步、管理面 Cookie/CSRF API、`/enforce`、legacy RBAC、Management CRUD |

### 0.3 推荐方案（TL;DR）

1. **Runtime 契约优先**：以 OIDC Discovery/JWKS/标准协议为契约；OpenAPI 3.1 留给未来 Management SDK/文档，不作为 Runtime 的前置条件。
2. **服务端先做 Runtime 契约整形**：补齐 Discovery 元数据、固化 managed 用户 token profile、typed Runtime DTO；管理面的 `Json<Value>`/分页/错误统一属于 Management backlog，不在本期。
3. **协议与 HTTP 层全部手写**：PKCE、token 刷新、JWKS 校验、introspection、backchannel logout 由各语言薄客户端实现；Runtime 不设 `gen/`，避免生成器风格与协议状态机割裂。
4. **三语言共享契约测试**：同一组 Discovery/token/JWKS/logout fixtures 驱动 Go/Python/Java；语言实现独立，但行为必须一致。OpenAPI 生成器只服务于未来的 Management SDK。
5. **不采用 UniFFI / FFI / WASM**：本项目 SDK 是 HTTP 客户端，不是进程内原生库；FFI 会引入 ABI、内存、跨平台分发问题，收益为负。
6. **本期不做 M2M 和管理面 SDK**：`client_credentials`、权限同步、`/account/*`、`/developer/*`、`/admin/*` 均不进入 Runtime SDK；管理面未来若要做，先增加一套机器认证的独立管理 API。
7. **发布与兼容**：SDK 走 SemVer；Runtime 以 fixtures 契约测试作为兼容 gate；用 `Deprecation` / `Sunset` 头做弃用；未来 Management 才使用 `/api/v1` + `oasdiff`。

### 0.4 Runtime SDK 进入封装前必须解决的阻塞（M2M 已排除）

| # | 阻塞 | 影响 | 处理建议 |
|---|---|---|---|
| R1 | Discovery 未声明 backchannel logout 能力 | 若首期包含 backchannel logout，SDK 无法自动发现，只能硬编码 | 补 `backchannel_logout_supported` / `backchannel_logout_session_supported` |
| R2 | backchannel logout token 校验规则未固化为契约 | 若首期包含 backchannel logout，三语言实现可能不一致 | 写清 `typ=logout+jwt`、`aud`、`events`、`sid`/`sub`、`jti`、`exp-iat ≤ 120s`，并加 fixtures |
| R3 | Runtime 端点错误/序列化语义未完全 typed | SDK 可能依赖偶然 JSON 形状 | 收敛 token/introspection/revocation/error 的 typed DTO 与 fixtures |
| R4 | 现有示例只有 TypeScript，没有后端 SDK 参考实现 | 接入方仍要自己摸索 | Go reference SDK + Quickstart 先行 |

> M2M（`client_credentials`）、权限同步、legacy token profile 均不在本期范围，不再作为 Runtime SDK 的阻塞项。Management SDK 的阻塞（`Json<Value>`、OpenAPI 覆盖、Cookie/CSRF）保留在 1.5，作为未来立项输入。

---

## 1. 项目现状盘点

### 1.1 代码与运行时

- Rust workspace：根二进制 `yufu`、`yufu` crate、`libs/auth`。
- Web 框架：Axum 0.8；ORM：SeaORM 2.0-rc；OpenAPI：`utoipa` 6 + `utoipa-swagger-ui` 10。
- 产品定位：单企业 OIDC/SSO 鉴权中心，外部应用注册 OIDC client，按显式权限和 token audience 接入。
- 已有接入示例：`examples/oidc-web`（机密 Web 客户端）、`examples/oidc-spa`（公共 SPA + PKCE），但都是 TypeScript 示例，不是后端 SDK。

### 1.2 对外能力分层

| 层 | 路径 | 认证方式 | 是否适合 SDK | 说明 |
|---|---|---|---|---|
| OIDC 协议 | `/.well-known/openid-configuration`、`/.well-known/jwks.json`、`/oidc/authorize`、`/oidc/token`、`/oidc/userinfo`、`/oidc/introspect`、`/oidc/revoke`、`/oidc/end-session` | PKCE / client secret / Bearer | **适合** | 标准协议，SDK 应优先覆盖 |
| 服务间运行时 | `/application/sync-permissions`、`/oidc/mcp-tokens/*` | client secret / Bearer | **本期不纳入** | M2M/权限同步非目标；MCP 需单独评估 |
| 浏览器管理面 | `/identity/*`、`/account/*`、`/developer/*`、`/admin/*` | SSO Cookie + CSRF + `idempotency-key` + `expected_revision` | **不适合直接做服务端 SDK** | 为浏览器控制台设计 |
| 旧 OIDC admin | `/oidc/clients`、`/oidc/client/{id}`、`/oidc/scope` | Bearer + 用户上下文/RBAC | **不建议** | `create_client` 当前直接返回 400，属于 legacy |
| 旧 RBAC/auth | `/api/v1/users`、`/role`、`/authority`、`/rbac`、`/enforce` | Bearer + 用户上下文/RBAC | **不建议纳入首期** | `/enforce` 当前恒 401，旧接口语义与新身份域并存 |

### 1.3 认证与鉴权模型

| 场景 | 当前实现 | SDK 含义 |
|---|---|---|
| Web 后端登录 | authorization code + PKCE（S256） | SDK 负责生成 authorize URL、校验 state/nonce、兑换 code、刷新、登出 URL |
| SPA 登录 | 公共 client + PKCE，无 secret | SDK 只提供浏览器侧 helper；secret 不能进前端 |
| 资源服务器校验 | 本地 RS256 校验 + 可选 introspection | SDK 负责 JWKS 缓存、`kid` 轮转、`iss/aud/typ/exp` 校验、introspection |
| 服务账号 | `client_credentials` | **M2M 非目标**，本期不提供 |
| 权限同步 | `/application/sync-permissions` | **非目标**，本期不提供 |
| 管理操作 | Cookie + CSRF + revision | 不进服务端 SDK；如需自动化，另建机器管理 API |

### 1.4 OpenAPI / 契约现状

| 项目 | 现状 | 证据 |
|---|---|---|
| OpenAPI 文档 | `ApiDoc` 只 nest 了 `AuthApi`、`UserApi`、`RoleApi`、`AuthorityApi`、`RecordingApi`、`RbacApi`、`UserAuthorityApi` | `yufu/src/swagger_docs.rs:1-30` |
| Swagger 挂载 | 仅 `openapi` feature 开启时挂到 `/docs`，spec 在 `/api-doc/openapi.json` | `yufu/src/lib.rs:435-437` |
| OIDC 路由 | 未进入 `ApiDoc` | `yufu/src/route/oidc.rs`、`yufu/src/route/identity_token.rs` |
| 管理面路由 | 未进入 `ApiDoc` | `yufu/src/route/account.rs`、`developer.rs`、`enterprise_admin.rs`、`application_governance.rs` |
| 返回类型 | 至少 24 处 `Json<serde_json::Value>` / type alias | `grep` 结果；见 1.5 |

### 1.5 关键代码证据与问题

> 范围提示：B1/B2/B3/B6/B7 主要是 Management SDK 的阻塞；本期 Runtime SDK 重点看 B4、B5、B8 中的 token/discovery 部分。

#### B1：`Json<Value>` 让 schema 失真

至少 24 处路由返回值使用 `Json<serde_json::Value>`，例如：

- `yufu/src/route/developer.rs:79`：`ManagementResponse = Result<(HeaderMap, Json<serde_json::Value>), AppError>`
- `yufu/src/route/enterprise_admin.rs:53-189`：多个管理 handler
- `yufu/src/route/account.rs:39-207`：账户面
- `yufu/src/route/application_governance.rs:25-56`：应用治理
- `yufu/src/route/identity_authorization.rs:162-200`

`developer.rs:80-90` 的 `response<T>` 虽然内部是 typed `T`，但最后 `serde_json::to_value` 擦除类型；生成器只能看到空对象/任意对象。这是未来 Management SDK 模型质量的最大阻塞；Runtime 不受影响。

#### B2：OpenAPI 覆盖面与真实路由不一致

`swagger_docs.rs` 只覆盖旧 RBAC/auth；而实际挂载的路由包括：

- `sub_auth_router`：旧用户/角色/权限 + `oidc_admin` + `enforce` + application/permission 管理；
- `sub_public_router`：旧 auth/user/rbac + `sync_permissions`；
- `get_no_prefix_routes()`：OIDC public routes；
- `identity_browser::get_routes()`：`/identity/*`、`/account/*`、`/admin/*` 等浏览器面。

见 `yufu/src/lib.rs:346-430`、`yufu/src/route/mod.rs:24-70`。

#### B3：错误响应不统一

- 身份域错误：`{"code": "...", "request_id": "..."}`，见 `yufu/src/error.rs:92-101`。
- 浏览器拒绝：`{"code": "...", "message": "..."}`，见 `yufu/src/error.rs:102-108`。
- `SecurityError` / `NotFound` / `BadRequest` 等：多数直接返回纯文本，见 `yufu/src/error.rs:109-125`。
- OAuth token 端点：`{"error": "invalid_grant", "request_id": "..."}`，见 `yufu/src/route/identity_token.rs:83-100`。

结果：未来 Management SDK 无法用一套错误模型覆盖所有端点；生成器也无法从 spec 生成稳定的异常类型。Runtime 只需按 RFC 6749 建模 OAuth 错误。

#### B4：`client_credentials` 是 legacy 有限实现（M2M 非目标）

代码中确实有 `client_credentials` 分支，但：

1. `identity_token.rs` 只允许 `authorization_code | refresh_token | client_credentials` 进入（`identity_token.rs:191-197`）。
2. `client_credentials` 只在 `management_state == "legacy_unmanaged"` 分支被分派（`identity_token.rs:218-232`）。
3. managed client 进入新路径后，只处理 `authorization_code` 和 `refresh_token`，其他 grant 返回 `unsupported_grant_type`（`identity_token.rs:249-300`）。
4. Discovery 的 `grant_types_supported` 只声明 `authorization_code`、`refresh_token`，没有 `client_credentials`（`oidc.rs:274-277`）。
5. legacy 签发路径 `OidcService::issue_client_credentials_token` 调用 `sign_access_token_for_client`，而后者使用 `TokenManager::sign_access_token`：
   - header 只设置 `kid`，没有设置 `typ=at+jwt`（`token_manager.rs:576-586`）；
   - claims 是 legacy `AccessTokenClaims`，没有 `token_use`、`principal_kind`、`application`、`bounds` 等字段（`claims.rs:46-63`）；
   - `aud` 被设为 `client_id`，`scope` 被设为空串（`yufu/src/service/oidc.rs:493-499`）。
6. managed 资源服务器的严格校验使用 `SecurityAccessClaims`，且 `deny_unknown_fields`（`libs/auth/src/security/credential.rs:108-129`），legacy token 无法通过。

**结论：M2M 不进入本期范围。** 若未来要做，需先决定 managed client 的 `client_credentials` 契约，再发布 SDK。

#### B5：两套 access token profile 并存（legacy 不在本期）

| 路径 | claims 类型 | header `typ` | 典型使用 |
|---|---|---|---|
| authorization_code / refresh（managed） | `SecurityAccessClaims` | `at+jwt`（`token_manager.rs:294-306`） | 用户会话访问资源 |
| client_credentials（legacy） | `AccessTokenClaims` | 未设置（默认 `JWT`） | 旧服务账号 |

Runtime SDK 只支持 managed 用户 token（`SecurityAccessClaims` + `at+jwt`）；legacy 服务账号不纳入本期。资源服务器若仍需兼容 legacy token，需要单独配置校验器，不能依赖 Runtime SDK 的默认行为。

#### B6：管理面是浏览器认证，不是机器认证

- 浏览器路由：`identity_browser.rs:32-38`。
- 写操作要求 `idempotency-key`：`developer.rs:72-78`。
- 并发控制用 body 里的 `expected_revision`：`enterprise_admin.rs`、`application_governance.rs`。
- 管理写操作前调用 `browser::verify_write`（Cookie + CSRF + Origin 校验）。

服务端 SDK 若模拟 Cookie/CSRF，会把浏览器安全模型带入机器调用，既脆弱又不符合 OAuth BCP。**不建议。**

#### B7：`/api/v1/enforce` 当前不可用

`service/permission.rs:123-127` 明确返回 401：

```rust
pub async fn enforce(_db: &DatabaseConnection, _req: &EnforceRequest) -> Result<EnforceResponse> {
    Err(AppError::Unauthorized(
        "verified subject evidence required".into(),
    ))
}
```

SDK 不应把 `/enforce` 作为可承诺能力；本期直接列为非目标。新授权决策应走 `security::authorization` + `AuthenticatedContext`，或另设带签名证据的 PDP API。

#### B8：契约细节不统一

- 分页：旧接口用 `page_size/page_number/total_pages`（`user.rs:75-94`），developer 应用列表用 `after/limit`（`developer.rs:104-118`）。
- token 响应：managed 路径 `identity_token.rs:309-316` 返回 `scope`；legacy 分派 `oidc.rs:405-418` 的 `TokenResponse` 不返回 `scope`。Runtime SDK 只解析 managed 响应，`scope` 作为可选字段。
- JWKS：直接返回 signer 的 JSON，无 typed schema（`oidc.rs:317-327`）。
- Discovery：未声明 `backchannel_logout_supported` / `backchannel_logout_session_supported`，但文档描述了后通道退出语义（`docs/integration/enterprise-oidc.md:116`、`:183`）。

---

## 2. “开发与 SDK 解耦”到底指什么

### 2.1 依赖方向

推荐的目标依赖关系（本期 Runtime SDK）：

```
接入应用业务代码
        │ 只依赖公开 API
        ▼
Runtime SDK（Go / Python / Java，手写薄客户端）
        │ OIDC Discovery / OAuth 2.0 / JWKS / RFC 7662 / RFC 7009
        ▼
yufu HTTP 接口
        │
        ▼
handler → domain / service → DB
```

关键约束：

1. **SDK 不依赖服务端 crate**：Go/Python/Java SDK 不 import Rust 类型、不依赖 SeaORM 模型、不复制 domain 逻辑。
2. **服务端不依赖 SDK**：服务端只负责实现协议；SDK 是外部消费者。
3. **唯一共享物是协议契约**：OIDC Discovery、JWKS、RFC 6749/7662/7009/9068 的 wire 语义，以及跨语言共用的契约测试向量。
4. **Runtime 不引入 OpenAPI 中间层**：Runtime 端点数量有限、协议由标准定义，手写 DTO 更小、更可控；OpenAPI 只属于未来的 Management SDK。
5. **服务端内部重构不影响 SDK**：只要 HTTP 行为不变，Rust 侧可以自由调整 handler、service、DB 结构。

### 2.2 为什么不能“从 Rust 类型直接生成三语言 SDK”

- Rust 的 `serde`/`utoipa` 可以产出 JSON Schema/OpenAPI，但 Runtime 的核心不是 CRUD 模型，而是 OIDC 协议状态机（PKCE、nonce、刷新轮转、JWKS 轮转、logout token 校验）。这些逻辑无法从类型定义生成。
- 直接做 FFI/UniFFI 只适合进程内原生库；HTTP SDK 会引入 ABI、内存管理、平台二进制分发、崩溃隔离等问题。
- 直接用 WASM 做 SDK，接入方需要 WASM runtime，且网络/证书/代理能力受限，不适合主流后端。
- 因此：**Runtime SDK 由各语言手写薄客户端；Rust 的职责是实现协议、修正契约、提供可复用的契约测试向量。** 只有未来的 Management CRUD SDK 才适合“Rust 产出 OpenAPI → 多语言生成”。

### 2.3 “契约优先”的务实落地

Runtime 的“契约”不是 OpenAPI 文档，而是以下四类可验证行为：

1. **Discovery 元数据**：issuer、authorization/token/userinfo/jwks/introspection/revocation/end-session 端点，`grant_types_supported`、`scopes_supported`、`token_endpoint_auth_methods_supported`、`id_token_signing_alg_values_supported`、backchannel logout 能力。
2. **OAuth/OIDC 协议语义**：PKCE、state/nonce、code 兑换、refresh 轮转、错误码、introspection/revocation 响应。
3. **Managed 用户 token profile**：JWT header（`alg`、`kid`、`typ=at+jwt`）、claims（`iss`、`aud`、`sub`、`exp`、`iat`、`scope`、`client_id`、`jti`）、ID Token 与 logout token 的额外校验；legacy 服务账号 token 不在本期契约内。
4. **契约测试向量**：一组固定的请求/响应 JSON 与签名测试 token，三语言 SDK 必须用同一组向量通过测试。

落地方式：

- 服务端先修 Discovery 与 token 输出，使上述行为稳定；
- 把请求/响应样例和测试 token 放入 `sdk/contract-fixtures/`，作为跨语言共享物；
- SDK 仓库只消费 fixtures 与公开协议，不读取 Rust 源码；
- 任何 wire 行为变更必须先改 fixtures、再改服务端、最后改三语言 SDK。

OpenAPI 仍然有价值，但它是未来 Management SDK 的契约，不是 Runtime SDK 的前置条件。

---

## 3. 契约设计与治理最佳实践

### 3.1 端点分类先于 SDK 设计

在写任何 SDK 之前，先把路由标记为四类：

| 类别 | 例子 | SDK 策略 |
|---|---|---|
| Protocol | `/oidc/authorize`、`/oidc/token`、`/.well-known/*` | 手写协议 facade；用标准库做 JOSE/JWT |
| Runtime | `/oidc/introspect`、`/oidc/revoke` | 手写薄 client；与 Protocol 共用 token/错误模型 |
| Management | `/developer/*`、`/admin/*`、`/oidc/clients` | **本期不纳入**；未来先补机器认证，再生成 |
| Legacy | `/api/v1/users`、`/role`、`/enforce` | 不纳入本期；单独标注 deprecated |

这份分类应成为 `docs/sdk/route-classification.md` 或代码中的 `RouteClass` 元数据。Runtime SDK 只覆盖 Protocol + Runtime；OpenAPI tag 与未来 Management SDK 复用同一分类。

### 3.2 OpenAPI 规范约定（未来 Management SDK）

> 本节不阻塞本期 Runtime SDK。Runtime 的契约由 2.3 的 Discovery/协议/token profile/fixtures 定义；以下约定只在未来做 Management SDK 或对外 API 文档时启用。

建议在 `api/openapi/` 下维护：

- `yufu.v1.yaml`：canonical，OpenAPI 3.1；
- `yufu.v1.0.3.yaml`：给仍不支持 3.1 的生成器用的兼容产物；
- `CHANGELOG.md`：面向接入方的变更说明；
- `oasdiff.yaml`：breaking-change 规则/例外。

规范约定：

1. **operationId 稳定**：`listApplications`、`getApplication`、`createApplication`，不要用 Rust handler 名。
2. **tag 稳定**：`Applications`、`OIDC`、`Sessions`、`Audit`、`Permissions`。
3. **schema 具体化**：禁止 `type: object` + 空 `properties`；禁止 `additionalProperties: true` 作为主要返回类型。
4. **避免泛型 envelope**：很多生成器对 `Page<T>` 支持差；为每个资源定义具体 `XxxPage`，或 `PageMeta + items: Xxx[]`。
5. **枚举可扩展**：Java/Go 生成 sealed enum 时，新增枚举值可能破坏客户端；建议生成“字符串常量 + 未知值兜底”，或在 spec 中标注 `x-extensible-enum`。
6. **字段命名冻结**：当前 wire format 基本是 `snake_case`；v1 不要改成 camelCase。SDK 内部可以映射成语言惯用命名，但 wire 不变。
7. **可空性明确**：3.1 用 `type: [string, "null"]`；3.0 兼容产物用 `nullable: true`；不要用“字段缺失”和“null”表达不同语义却不写清楚。
8. **错误响应逐 operation 声明**：至少 400/401/403/404/409/412/422/429/503。
9. **安全方案显式声明**：`bearerAuth`、`oauth2`（clientCredentials）、`clientSecretBasic`、`clientSecretPost`；public 端点写 `security: []`。
10. **示例值**：每个请求/响应至少一个 example；生成器会用它做文档和测试夹具。

### 3.3 统一错误模型

**Runtime（本期）只使用 OAuth/OIDC 标准错误体**，不要强行改成 RFC 9457：

- Token：`{"error":"invalid_grant","error_description":"...","request_id":"..."}`
- Introspection/Revocation：按 RFC 7662/7009 返回。

**管理面（未来 Management SDK）**建议统一为 RFC 9457 `application/problem+json`：

```json
{
  "type": "https://auth.example.com/problems/invalid-scope",
  "title": "Invalid scope",
  "status": 400,
  "detail": "The requested scope is not allowed for this client.",
  "code": "invalid_scope",
  "request_id": "2f1c...",
  "errors": [
    {
      "field": "scope",
      "code": "not_allowed",
      "message": "scope 'admin:write' is not granted"
    }
  ]
}
```

要求：

- `code` 是稳定机器码，不随文案变化；
- `request_id` 同时出现在 body 和 `X-Request-ID`；
- 4xx 不返回堆栈、SQL、内部路径；
- 5xx 只返回通用 `internal_server_error` + request_id；
- Runtime SDK 只建模 OAuth 错误；管理面 problem 错误留给未来的 Management SDK。

### 3.4 分页、幂等、并发、追踪（Management 为主）

> 分页、`Idempotency-Key`、`ETag`/`expected_revision` 只适用于未来的 Management SDK；Runtime 端点没有这些语义。追踪、超时、重试与 `Retry-After` 同时适用于 Runtime。

| 主题 | 建议 |
|---|---|
| 分页 | 新 API 用 cursor：`limit` + `cursor` + `next_cursor`；旧 page 接口在 spec 中标注 legacy；不要在新 SDK 中混用两套 |
| 幂等 | 所有非天然幂等 POST 接受 `Idempotency-Key`；服务端保存 key + 请求指纹 + 响应；SDK 自动生成 UUID 并支持调用方覆盖 |
| 并发 | 管理面用 `ETag` + `If-Match`；过渡期保留 `expected_revision`，但 SDK 不把它当通用机制 |
| 追踪 | `X-Request-ID` 入站/出站；支持 W3C `traceparent`；SDK 暴露 request-id 便于排障 |
| 重试 | GET/HEAD 可重试；POST 仅在 `Idempotency-Key` 存在时重试；429/502/503/504 指数退避 + jitter，尊重 `Retry-After` |
| 超时 | 连接/总超时可配置；默认不要无限等待；token 刷新使用 single-flight |

### 3.5 版本、弃用与兼容

1. **Runtime API 版本**：OIDC 端点不按路径版本化，按标准协议增量扩展；新增能力通过 Discovery 元数据与可选字段表达。
2. **Management API 版本**（未来）：继续 `/api/v1`；破坏性变更走 `/api/v2`。
3. **SDK 版本**：SemVer。Runtime SDK major 与协议兼容性对齐；未来 Management SDK major 与 API major 对齐。
4. **兼容规则**：
   - 允许：新增可选字段、新增端点、新增错误码、放宽校验；
   - 禁止（v1 内）：删除/重命名字段、改类型/格式、把可选改必填、改变字段语义、删除枚举值；
   - 谨慎：新增枚举值、改变默认值、改变分页顺序。
5. **弃用**：使用 `Deprecation`（RFC 9745）+ `Sunset`（RFC 8594）+ `Link rel="deprecation"`；SDK 对应方法加 `@Deprecated`/`Deprecated:` 注释。
6. **支持窗口**：公开 SDK 至少支持最近 2 个 minor；弃用公告至少提前 6 个月或 2 个 minor（取较长者）。
7. **Runtime 契约 gate**：三语言共用 fixtures；服务端 wire 变更必须同步更新 fixtures 并让三语言测试通过。
8. **Management 契约 gate**（未来）：`oasdiff breaking` 在 PR 上失败；例外必须写入 allowlist 并由 API owner 批准。

### 3.6 安全基线

- 所有 token、client secret、authorization code、PKCE verifier 不得进日志、URL、错误消息。
- SDK 默认开启 TLS 校验；允许企业自定义 CA，但不能默认关闭。
- JWT 校验：算法 allowlist（RS256）、`typ` 精确匹配、`iss` 精确、`aud` 包含期望值、`exp/nbf` + 时钟偏移、`jti`、`sub`；managed access token 要求 `typ=at+jwt`；ID Token 额外校验 `nonce`、`at_hash`、`azp`（多 audience 时）。
- JWKS：按 `kid` 缓存，未知 `kid` 时刷新一次；遵循 Cache-Control；限制响应大小；不要每请求拉 JWKS。
- Token 缓存：按过期时间减 skew；刷新 single-flight；refresh token 轮转后旧 token 立即失效。
- 遵循 RFC 9700（OAuth 2.0 Security BCP）与 RFC 8725（JWT BCP）。
- 资源服务器可参考 RFC 9728（Protected Resource Metadata）暴露自身 metadata；当前项目未实现，可作为后续增强。

---

## 4. Rust → Go / Python / Java 的技术路线

### 4.1 方案对比

> 版本号于 2026-11-11 通过各项目 releases/latest 与 npm 查询；未对每个生成器做完整基准测试。

> **适用范围**：Runtime SDK 不走下表生成器；三语言均手写薄客户端。下表用于未来 Management SDK 的选型参考。

| 方案 | 覆盖语言 | 产出风格 | OpenAPI 3.1 | 定制成本 | 适用结论 |
|---|---|---|---|---|---|
| **OpenAPI Generator** `v7.27.0` | 50+，含 Go/Python/Java | 模板成熟，风格偏“生成器味” | 部分支持，3.1 仍有缺口 | 中 | **多语言基线**；Java 首选 |
| **oapi-codegen** `v2.8.2` | Go | 地道 Go，支持 std/chi/echo/gin | 支持 3.0/3.1 | 低 | **Go 首选** |
| **openapi-python-client** `v0.30.0` | Python | attrs + httpx，类型化 | 支持 3.0/3.1 | 低 | **Python 首选** |
| **Kiota** `v1.38.0` | C#/Go/Java/Python/TS 等 |  fluent builder，微软生态 | 以 3.0 为主 | 中 | 统一生成备选；适合微软生态 |
| **Fern / Speakeasy / Stainless** | 多语言 | 商业级、文档/示例完整 | 视产品 | 高（费用/锁定） | 团队缺 SDK 工程能力时的加速选项 |
| **全手写** | 任意 | 最佳语言体验 | 不适用 | 最高 | **Runtime 本期方案**；Management 只用于 facade |

### 4.2 推荐组合

#### 4.2.1 Runtime SDK（本期）：手写 + 成熟标准库

| 语言 | HTTP | JOSE/JWT | OIDC/OAuth 辅助 | 说明 |
|---|---|---|---|---|
| Go | `net/http` + `golang.org/x/oauth2` | `github.com/go-jose/go-jose/v4` 或 `github.com/lestrrat-go/jwx/v3` | 手写 Discovery/PKCE/refresh/JWKS | 标准库优先；不引入重型框架；context 贯穿 |
| Python | `httpx` | `authlib` 或 `PyJWT` + `cryptography` | 手写同步/异步 facade | 类型标注 + `py.typed`；支持 3.10+ |
| Java | `java.net.http.HttpClient` | Nimbus `JOSE+JWT` | 手写 facade | Java 17；不强制 Spring；可选 Spring Boot starter |

原则：

- 协议状态机手写，JOSE/JWT 密码学交给成熟库；
- 三语言共享同一组 fixtures 与错误语义；
- 每语言只暴露一个高层 client + 少量 typed model，不生成全量 HTTP 层；
- 不把 OpenAPI 生成器引入 Runtime 构建链。

#### 4.2.2 Management SDK（未来）：生成器

**推荐“一个 spec，多生成器，统一 facade 风格”**：

| 语言 | 生成器 | 生成内容 | 手写 facade |
|---|---|---|---|
| Go | `oapi-codegen` | types + client + server（可选） | `NewClient`、context、options、错误映射、分页 helper |
| Python | `openapi-python-client` | typed models + httpx client | sync/async client、异常、分页 helper |
| Java | OpenAPI Generator `java` + `okhttp-gson` | models + api classes | builder、异常、分页 helper、AutoCloseable |

如果团队希望减少工具数量，可先用 OpenAPI Generator 统一生成三语言作为 v0；等接入方反馈后再替换为语言原生生成器。**不要把“统一生成器”当成目标，SDK 的语言体验才是目标。** 该组合只用于未来 Management SDK，不影响本期 Runtime 手写方案。

### 4.3 UniFFI / FFI / WASM 专项评估

#### 4.3.1 UniFFI（Mozilla）

调研对象：<https://github.com/mozilla/uniffi-rs>。截至 2026-11-12，crates.io 最新版本为 `0.32.2`；README 明确“可用于生产，但距离 1.0 仍很远”。结论：**不采用**。

| 维度 | 事实 | 对本项目的影响 |
|---|---|---|
| 定位 | 把 Rust 库编译为 `cdylib`，通过 UDL 或 proc-macro 生成 Kotlin/Swift/Python/Ruby 等绑定 | 解决的是“进程内调用 Rust”，不是“HTTP 服务客户端” |
| 一方支持 | Kotlin、Swift、Python、Ruby 等由项目维护 | Python 可用，但 Go/Java 不在核心一方支持范围 |
| Go/Java | 均为第三方绑定（Go：NordSecurity；Java：IronCoreLabs），不在核心一方支持范围 | 恰好是本项目优先级最高的两个语言，风险不可接受 |
| 稳定性 | 仍处于 `0.x`（最新 `0.32.2`），API 与生成代码存在 churn | SDK 需要长期兼容，不能把接入方绑到快速变动的绑定层 |
| 分发 | 需要为每个 OS/架构分发 `.so`/`.dylib`/`.dll`，并处理符号、ABI、崩溃隔离 | 后端接入方部署环境复杂，发布与排障成本远高于纯语言包 |
| 异步/网络 | 绑定层对 async、回调、线程模型支持有限 | OIDC 的 HTTP、超时、重试、代理、TLS 自定义都要重新桥接 |
| 打包 | 不提供端到端的多语言包发布、文档、版本矩阵 | 仍需自建 Go module/PyPI/Maven 工程，收益进一步降低 |

**适用场景**：桌面/移动 App 内嵌 Rust 核心、CLI、本地高性能组件。

**不适用场景**：本项目这种“接入方后端通过 HTTP 调用鉴权中心”的 Runtime SDK。FFI 会把网络客户端变成原生库分发问题，且 Go/Java 支持不在核心路径。

**如果未来确实要复用 Rust 本地能力**：优先做 **sidecar**（独立进程 + 本地 HTTP/gRPC），而不是 UniFFI。sidecar 保留进程隔离、可独立升级、可用标准 HTTP 工具链排障；只有“必须零拷贝/极低延迟”的嵌入式场景才重新评估 FFI。

#### 4.3.2 其他方案

| 方案 | 适用 | 不适用本项目的原因 |
|---|---|---|
| 手写 C ABI | 嵌入式/高性能本地库 | 跨语言字符串/错误/异步处理成本高；不适合 HTTP JSON |
| WASM | 浏览器/沙箱插件 | 后端接入需要 WASM runtime；网络/证书/代理能力弱；调试困难 |

SDK 是网络客户端，**HTTP + JSON + 标准协议**已经是跨语言最稳定的边界。

### 4.4 Rust 侧需要产出的东西

#### 4.4.1 Runtime SDK 必需（本期）

1. **Discovery 元数据修正**：确保 `grant_types_supported` 只声明实际支持的 `authorization_code` / `refresh_token`；补齐 `token_endpoint_auth_methods_supported`、`scopes_supported`、`backchannel_logout_supported`、`backchannel_logout_session_supported`。
2. **固化 managed TokenResponse**：统一 `access_token`、`token_type`、`expires_in`、`refresh_token`、`scope`、`id_token` 的字段与语义；错误体保持 RFC 6749。legacy 响应不纳入本期。
3. **固化 managed 用户 access token profile**：明确 `typ=at+jwt`、`aud`、`scope`、`client_id`、`jti` 的取值规则；legacy 服务账号 token 不纳入本期。
4. **Backchannel Logout 契约**：写清 logout token 的 `typ=logout+jwt`、`aud=client_id`、`events`、`sid`/`sub`、`jti`、`exp-iat ≤ 120s` 校验规则，并在 Discovery 声明。
5. **Runtime typed DTO（Rust 内部）**：至少把 token、introspection、revocation、error 的序列化收敛到 typed struct，避免 SDK 依赖 `serde_json::Value` 的偶然形状。
6. **契约 fixtures + mock server**：在 `sdk/contract-fixtures/` 提供固定请求/响应、JWKS、签名测试 token；三语言共用。

> Runtime 不需要先建 `libs/api-contract`，也不需要 OpenAPI emitter。先把协议行为稳定下来，收益更高。

#### 4.4.2 Management SDK 未来需要

1. **contract crate**：`libs/api-contract`（建议名），只依赖 `serde` + `utoipa`，不依赖 SeaORM/domain。
2. **typed handlers**：所有管理面公开路由返回 `Json<Dto>`；错误统一映射。
3. **OpenAPI emitter**：独立二进制，不启动服务器、不连数据库：

```rust
// yufu/src/bin/emit-openapi.rs（示意；需在 crate 暴露 pub fn openapi_document()）
fn main() {
    let spec = yufu::openapi_document();
    println!("{}", spec.to_pretty_json().expect("serialize OpenAPI"));
}
```

4. **spec 产物**：CI 生成 `api/openapi/yufu.v1.json`；与已提交版本 diff 必须为空。
5. **兼容产物**：若生成器不支持 3.1，用 `@openapi-spec/downgrader` 生成 3.0.3 输入；不要手改两份 spec。

当前 `utoipa` 6 默认输出 OpenAPI 3.1.0，因此未来 Management SDK 的 canonical 3.1 改造成本最低；3.0.3 只作为生成器的兼容输入。

### 4.5 SDK 目录布局与代码所有权

建议布局（初期可在本仓库，稳定后拆独立仓库）：

```
sdk/
  contract-fixtures/          # 三语言共享，受 API review 管控
    discovery.json
    token-success.json
    token-error.json
    jwks.json
    logout-token.jwt
  go/
    yufu/                 # public facade（手写）
    internal/oidc/            # 协议实现（手写，不对外）
    go.mod
  python/
    src/yufu_sdk/         # public facade（手写）
    tests/
    pyproject.toml
  java/
    src/main/java/com/yufu/sdk/   # public facade（手写）
    src/test/java/...
    pom.xml
  management/                 # 未来 Management SDK
    gen/                      # 生成代码，禁止手改
```

规则：

- **Runtime 全部手写**，不设 `gen/`；公开 API 保持稳定，内部实现可重构。
- **fixtures 是共享契约**：任何字段/错误码变化先改 fixtures，再改服务端与三语言 SDK。
- **未来 Management 生成代码**放 `management/gen/`，文件头标注生成器版本与源 spec，CI 重新生成并检查 diff。
- 生成文件头示例：`// Code generated by oapi-codegen v2.8.2 from yufu.v1.yaml; DO NOT EDIT.`

### 4.6 打包与发布

| 语言 | 包形态 | 发布 | 关键点 |
|---|---|---|---|
| Go | module `github.com/<org>/yufu-sdk-go` | Git tag `v0.1.0`（v2+ 才需要 `/v2`） | module path 一旦发布不要改；`go vet` + `go test`；避免 cgo |
| Python | wheel + sdist，包名 `yufu-sdk` | PyPI Trusted Publishing（OIDC） | `py.typed`；支持 3.10+；httpx；同步/异步 |
| Java | Maven `com.yufu:yufu-sdk` | Maven Central `central-publishing-maven-plugin` | Java 17；sources/javadoc；GPG 签名；OkHttp 4.x |
| 文档 | 每语言 README + Quickstart + API reference | 与包同版本 | 示例必须可运行；不要复制过期片段 |

发布流水线建议：

1. 服务端 release tag（如 `v1.4.0`）附带 `contract-fixtures` 版本号 + SHA256；
2. Runtime SDK 仓库消费 fixtures，跑三语言契约测试；
3. 编译、测试、签名、发布；
4. 发布说明包含：支持的协议/Discovery 版本、fixtures 版本、breaking changes、迁移链接。

> Runtime SDK 不消费 OpenAPI artifact；未来 Management SDK 才需要服务端额外发布 OpenAPI spec。

### 4.7 仓库策略：monorepo 还是独立 SDK 仓库

| 策略 | 优点 | 缺点 | 建议 |
|---|---|---|---|
| 本仓库 `sdk/` | 改动原子化；调试方便 | 语言工具链混在一起；发布权限耦合 | **初期验证用** |
| 独立 `cobot-sdk` 仓库 | 边界清晰；独立版本/发布/权限 | 需要同步 fixtures/协议版本 | **稳定后推荐** |

推荐路径：先在服务端仓库用 `sdk/` 打通手写客户端与 fixtures 契约测试，协议稳定后拆到独立仓库；服务端仓库只负责发布 fixtures 与协议变更说明。

### 4.8 语言惯用 facade 示例

以下仅为 API 形态示意，不是最终命名：

**Go**

```go
client, err := yufu.New(yufu.Config{
    Issuer:       "https://auth.example.com",
    ClientID:     "web",
    ClientSecret: os.Getenv("OIDC_CLIENT_SECRET"),
    RedirectURI:  "https://app.example.com/callback",
})
if err != nil { return err }
defer client.Close()

authURL, verifier, err := client.AuthorizeURL(yufu.AuthorizeOptions{
    Scopes: []string{"openid", "profile", "email"},
})
tokens, err := client.ExchangeCode(ctx, code, verifier)
claims, err := client.VerifyAccessToken(ctx, tokens.AccessToken)
```

**Python**

```python
async with YufuClient(
    issuer="https://auth.example.com",
    client_id="web",
    client_secret=os.environ["OIDC_CLIENT_SECRET"],
    redirect_uri="https://app.example.com/callback",
) as client:
    auth_url, verifier = client.authorize_url(scopes=["openid", "profile", "email"])
    tokens = await client.exchange_code(code, verifier)
    claims = await client.verify_access_token(tokens.access_token)
```

**Java**

```java
try (YufuClient client = YufuClient.builder()
        .issuer("https://auth.example.com")
        .clientId("web")
        .clientSecret(System.getenv("OIDC_CLIENT_SECRET"))
        .redirectUri("https://app.example.com/callback")
        .build()) {
    AuthorizeUrl url = client.authorizeUrl(List.of("openid", "profile", "email"));
    TokenSet tokens = client.exchangeCode(code, url.codeVerifier());
    AccessClaims claims = client.verifyAccessToken(tokens.accessToken());
}
```

统一原则：

- 构造器/builder 只接受 issuer、client 凭据、HTTP transport、超时、日志钩子；
- token 刷新、JWKS 缓存、错误映射由 facade 内部完成；
- 内部协议实现不暴露给最终用户（Go 放 `internal/`，Python 用私有模块，Java 放 `internal` 包）；
- 所有异常都携带 `request_id` 和 HTTP status，便于接入方排障。

---

## 5. 推荐的目标架构

### 5.1 分层

```
┌─────────────────────────────────────────────────────────────┐
│ 接入应用（Go / Python / Java）                               │
│  ┌───────────────────────┐  ┌────────────────────────────┐  │
│  │ Runtime SDK facade    │  │ Management SDK（本期不做） │  │
│  │ OIDC/PKCE/JWKS/Token  │  │ generated CRUD + typed     │  │
│  └───────────┬───────────┘  └─────────────┬──────────────┘  │
│              │ HTTP/JSON                  │                 │
└──────────────┼────────────────────────────┼─────────────────┘
               ▼                            ▼
        OIDC / runtime endpoints      /api/v1 management
               │                            │
        ┌──────┴────────────────────────────┴──────┐
        │ yufu (Axum)                           │
        │ handler → contract DTO → domain/service   │
        └──────────────────┬───────────────────────┘
                           ▼
                    domain / DB / auth
```

### 5.2 Runtime SDK 的能力清单（首期候选）

| 能力 | 方法（示意） | 首期 | 说明 |
|---|---|---|---|
| Discovery | `Discover(ctx)` | 是 | 缓存 `/.well-known/openid-configuration`；校验 issuer |
| 登录 URL | `AuthorizeURL(state, nonce, PKCE)` | 是 | 生成 URL；保存 state/nonce/verifier |
| Code 兑换 | `ExchangeCode(ctx, code, verifier)` | 是 | 机密 client 用 secret；公共 client 不带 secret |
| 刷新 | `Refresh(ctx, refreshToken)` | 是 | single-flight；轮转后更新缓存 |
| 用户信息 | `UserInfo(ctx, accessToken)` | 是 | Bearer |
| 登出 | `EndSessionURL(idTokenHint, postLogoutRedirectURI)` | 是 | 生成浏览器跳转 URL，不服务端调用 |
| JWKS | `Verifier(issuer, audience)` | 是 | 缓存、kid 轮转、RS256；要求 `typ=at+jwt` |
| Introspection | `Introspect(ctx, token)` | 待定 | Reader 凭据；返回 active/scope/client_id/sub/exp |
| Revocation | `Revoke(ctx, token, hint)` | 待定 | access/refresh |
| Backchannel Logout | `VerifyLogoutToken(token)` | 待定（建议首期） | `typ=logout+jwt`、aud=client_id、events、jti、sid/sub、exp-iat ≤ 120s |

> M2M（`ClientCredentials`）、权限同步（`SyncPermissions`）和管理面能力均不在本期范围，不进入 Runtime SDK 的公开 API。

### 5.3 Management SDK 的前置条件（未来立项，不在本期）

如果产品确实需要管理 SDK，建议新增一套机器管理 API，而不是复用浏览器面：

- 认证：未来管理面专用的机器认证 + scopes（如 `admin:clients:read`、`admin:clients:write`），与本期 Runtime SDK 无关；
- 端点：`/api/v1/admin/*` 或 `/api/v1/machine/*`；
- 无 Cookie/CSRF；用 Bearer + scope；
- 写操作：`Idempotency-Key` + `If-Match`（ETag）；
- 审计：actor 为 client_id，记录 request_id；
- 错误：RFC 9457；
- 限流：按 client/scope；
- 权限模型：与浏览器管理员权限分离，最小权限。

在这套 API 就绪前，Management SDK 只能做“只读/低风险”或干脆不做。

### 5.4 SDK facade 的职责边界

| Runtime 手写层（本期） | 未来 Management 生成层 |
|---|---|
| Discovery 缓存、issuer 校验 | 模型、序列化、HTTP 方法、URL 拼接 |
| PKCE、state/nonce、code 兑换、refresh single-flight | 基础状态码与低层 client |
| JWKS 缓存、kid 轮转、JWT 校验 | 管理资源 CRUD 的 typed client |
| OAuth 错误映射、retry/backoff、request-id | 生成文档骨架 |
| 语言惯用 API、builder/options、context | 不直接暴露给最终用户，由 facade 包装 |

**Runtime 没有生成层**：所有代码都是可审查的手写实现；未来 Management 的生成层放 internal，facade 仍是 public。

---

## 6. 本仓库分阶段落地计划

### Phase 0：Runtime 范围冻结与契约审计（1–2 天）

- [ ] 冻结本期范围：只做 Runtime SDK；Management 不进入本期。
- [ ] 冻结非目标：M2M/`client_credentials`、权限同步、`/enforce`、legacy RBAC。
- [ ] 输出 `route-classification.md`：Protocol / Runtime / Management / Legacy。
- [ ] 确认 Introspection / Revocation / Backchannel Logout 是否都在首期公开 API 中。
- [ ] 审计 Discovery、token、JWKS、introspection、revocation、backchannel logout 的现状与差异。

**产出**：范围决策记录 + 路由清单 + Runtime 契约差异表。

### Phase 1：Runtime 契约修正（5–8 天）

- [ ] 修 Discovery：确保 `grant_types_supported` 只含 `authorization_code` / `refresh_token`；补齐 `token_endpoint_auth_methods_supported`、`scopes_supported`、backchannel metadata。
- [ ] 固化 managed `TokenResponse`：字段集合、`scope` 语义、错误体；legacy 不纳入。
- [ ] 固化 managed 用户 access token profile（`typ=at+jwt`、`aud`、`scope`、`client_id`、`jti`）。
- [ ] 若首期包含 backchannel logout：明确 logout token 的签发与校验规则。
- [ ] 把 Runtime 端点的序列化收敛到 typed DTO，避免依赖 `serde_json::Value`。
- [ ] 统一 `X-Request-ID` 与 OAuth 错误映射。

**验收**：Discovery 声明与实际行为一致；managed token 响应稳定；错误路径有固定样例。

### Phase 2：Runtime 契约测试与兼容性 gate（3–5 天）

- [ ] 建立 `sdk/contract-fixtures/`：Discovery、token 成功/失败、JWKS、introspection、revocation、logout token。
- [ ] 建立 mock server（可用 WireMock/Prism 或轻量 Rust 测试服务）。
- [ ] 为服务端加契约测试：真实响应与 fixtures 一致。
- [ ] CI gate：fixtures 变更必须经过 review；服务端行为变更必须同步 fixtures。
- [ ] 安全测试向量：过期 token、错误 `aud`、错误 `typ`、未知 `kid`、重放 logout token。

**验收**：不依赖 OpenAPI，也能阻止 Runtime wire 行为无意漂移。

### Phase 3：三语言 Runtime SDK（Go → Python → Java，建议 4–6 周）

顺序理由：Go 的 module 发布最简单、标准库最完整，适合作为 reference SDK 先验证 fixtures 与协议状态机；Python 复用同一套测试矩阵的成本最低；Java 的企业接入价值高但工程与发布链路最重，放在最后可复用前两者的经验。

#### Phase 3.1 Go（reference SDK，1.5–2 周）

- [ ] `Discover` + issuer 校验 + 缓存；
- [ ] `AuthorizeURL` + PKCE + state/nonce；
- [ ] `ExchangeCode`、`Refresh`、`UserInfo`；
- [ ] JWKS 缓存 + JWT 校验（`typ=at+jwt`、`aud`、`exp/nbf`）；
- [ ] `Introspect`、`Revoke`、`VerifyLogoutToken`；
- [ ] 错误映射、超时、重试、request-id；
- [ ] fixtures 契约测试 + mock server 集成测试；
- [ ] README/Quickstart + 发布演练。

#### Phase 3.2 Python（1–1.5 周）

- [ ] 复用 Go 的 fixtures 与测试矩阵；
- [ ] 同步 + 异步 client；
- [ ] 类型标注 + `py.typed`；
- [ ] 其余能力与 Go 对齐；
- [ ] PyPI 发布演练（Trusted Publishing）。

#### Phase 3.3 Java（1.5–2 周）

- [ ] 复用同一套 fixtures；
- [ ] Java 17 + `HttpClient` + Nimbus JOSE；
- [ ] builder、`AutoCloseable`、可选 Spring Boot starter；
- [ ] 其余能力与 Go 对齐；
- [ ] Maven Central 发布演练。

**统一验收**：三语言 Quickstart 都能完成“Discovery → 登录 → 回调 → 校验 token → userinfo → 刷新 → 登出”；错误与安全测试向量全部通过。

### Phase 4：发布与兼容运营（持续）

- [ ] 建立 `cobot-sdk` 仓库/发布流水线。
- [ ] 版本策略、弃用策略、支持矩阵。
- [ ] 安全公告渠道、SDK issue 模板。
- [ ] 每季度复核依赖（JOSE/HTTP 库）与安全公告；不自动升级。
- [ ] 维护 fixtures 版本与协议变更日志。

### Phase 5：Management SDK（未来，可选，2–4 周）

- [ ] 设计机器管理 API + scope 模型。
- [ ] 实现 Bearer + scope + Idempotency-Key + ETag。
- [ ] 建 `libs/api-contract` + OpenAPI emitter。
- [ ] 用生成器产出 Management SDK。
- [ ] 与浏览器管理面共用 domain/service，但传输层分离。

---

## 7. 风险清单

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| **Runtime**：接入方误用 legacy token 或期望 M2M | 中 | 高 | 文档明确只支持 managed 用户 token；M2M/legacy 不在公开 API |
| **Runtime**：范围蔓延到 M2M/Management | 中 | 高 | 范围冻结；非目标写入 README 与 SDK 文档 |
| **Runtime**：Discovery 声明与实际行为不一致 | 高 | 高 | Phase 1 对齐；契约测试校验 Discovery |
| **Runtime**：backchannel logout 校验规则不清晰（若纳入首期） | 中 | 高 | 固定 `typ/aud/events/sid/sub/jti` 规则 + 测试向量 |
| **Runtime**：三语言对同一 fixtures 解释不一致 | 中 | 高 | 共享 fixtures + 统一测试矩阵；CI 三语言都跑 |
| **Runtime**：JOSE/JWT 库安全漏洞或算法配置错误 | 低 | 极高 | 算法 allowlist；依赖审计；安全 review；日志脱敏 |
| **Runtime**：Maven/PyPI/Go 发布工程复杂 | 中 | 中 | 提前演练；Trusted Publishing / central plugin |
| **Runtime**：SDK 版本与协议行为错配 | 中 | 高 | SemVer + fixtures 版本 + 发布清单 |
| **Management-only**：`Json<Value>` 导致 schema 失真 | 高 | 中 | 未来 Phase 5 强制 typed DTO；OpenAPI 校验 |
| **Management-only**：OpenAPI 3.1 生成器兼容性 | 中 | 中 | canonical 3.1 + 3.0.3 兼容产物；锁定生成器 |
| **Management-only**：生成代码风格差 | 中 | 中 | 手写 facade；按语言选生成器 |
| 管理面被误当作 Runtime SDK 入口 | 中 | 高 | 明确路由分类；SDK 只覆盖 Protocol/Runtime |
| 支持负担（三语言 issue） | 中 | 中 | 明确支持矩阵；模板化文档；优先修契约 |

---

## 8. 需要用户/团队确认的问题

1. **Introspection / Revocation**：是否进入首期公开 API，还是只做 JWKS 本地校验？
2. **Backchannel Logout**：是否首期必须支持，还是放到 v0.2？
3. **仓库策略**：SDK 放本仓库 `sdk/`，还是一开始就独立 `cobot-sdk` 仓库？
4. **发布账号**：Go module path、PyPI 项目名、Maven groupId/artifactId 是否已确定？

---

## 9. 参考资料

> 访问日期：2026-11-11。版本号来自各项目 releases/latest 或 npm registry 查询；未做完整生成基准。

### 规范

- OpenAPI Specification 3.1.0：https://spec.openapis.org/oas/v3.1.0
- OpenID Connect Core 1.0：https://openid.net/specs/openid-connect-core-1_0.html
- OpenID Connect Discovery 1.0：https://openid.net/specs/openid-connect-discovery-1_0.html
- OpenID Connect Back-Channel Logout 1.0：https://openid.net/specs/openid-connect-backchannel-1_0.html
- RFC 6749（OAuth 2.0）：https://www.rfc-editor.org/rfc/rfc6749
- RFC 7009（Token Revocation）：https://www.rfc-editor.org/rfc/rfc7009
- RFC 7636（PKCE）：https://www.rfc-editor.org/rfc/rfc7636
- RFC 7662（Token Introspection）：https://www.rfc-editor.org/rfc/rfc7662
- RFC 8414（Authorization Server Metadata）：https://www.rfc-editor.org/rfc/rfc8414
- RFC 8594（Sunset Header）：https://www.rfc-editor.org/rfc/rfc8594
- RFC 8725（JWT BCP）：https://www.rfc-editor.org/rfc/rfc8725
- RFC 9068（JWT Profile for OAuth 2.0 Access Tokens）：https://www.rfc-editor.org/rfc/rfc9068
- RFC 9457（Problem Details）：https://www.rfc-editor.org/rfc/rfc9457
- RFC 9700（OAuth 2.0 Security BCP）：https://www.rfc-editor.org/rfc/rfc9700
- RFC 9728（Protected Resource Metadata）：https://www.rfc-editor.org/rfc/rfc9728
- RFC 9745（Deprecation Header）：https://www.rfc-editor.org/rfc/rfc9745
- Semantic Versioning 2.0.0：https://semver.org/

### API 设计参考

- Google AIP-180（Backwards compatibility）：https://google.aip.dev/180
- Google AIP-185（Versioning）：https://google.aip.dev/185
- Google AIP-193（Errors）：https://google.aip.dev/193
- Google AIP-158（Pagination）：https://google.aip.dev/158
- Microsoft REST API Guidelines：https://github.com/microsoft/api-guidelines

### 工具

- OpenAPI Generator：https://openapi-generator.tech/ ；`v7.27.0`
- oapi-codegen：https://github.com/oapi-codegen/oapi-codegen ；`v2.8.2`
- openapi-python-client：https://github.com/openapi-generators/openapi-python-client ；`v0.30.0`
- Kiota：https://github.com/microsoft/kiota ；`v1.38.0`
- oasdiff：https://github.com/oasdiff/oasdiff ；`v1.37.0`
- Spectral：https://github.com/stoplightio/spectral ；`v6.17.1`
- Redocly CLI：https://github.com/Redocly/redocly-cli ；`v2.77.0`
- `@openapi-spec/downgrader`：`v0.8.0`
- `@scalar/openapi-upgrader`：`v0.4.8`
- Fern：https://www.buildwithfern.com/
- Speakeasy：https://www.speakeasy.com/
- Stainless：https://www.stainless.com/
- UniFFI（已评估，不采用）：https://github.com/mozilla/uniffi-rs ；crates.io 最新 `0.32.2`

### 本仓库参考

- `README.md`
- `CLAUDE.md`
- `Cargo.toml`、`yufu/Cargo.toml`
- `yufu/src/lib.rs`
- `yufu/src/route/oidc.rs`
- `yufu/src/route/identity_token.rs`
- `yufu/src/route/identity_browser.rs`
- `yufu/src/route/developer.rs`
- `yufu/src/route/enterprise_admin.rs`
- `yufu/src/route/application_governance.rs`
- `yufu/src/error.rs`
- `yufu/src/swagger_docs.rs`
- `yufu/src/service/permission.rs`
- `yufu/src/service/identity/legacy_client.rs`
- `libs/auth/src/token_manager.rs`
- `libs/auth/src/security/credential.rs`
- `docs/integration/enterprise-oidc.md`
- `docs/specs/enterprise-sso-provider/spec.md`、`gaps.md`

---

## 附录 A：建议的 ADR 草案

### ADR-001：Runtime SDK 只依赖 OIDC/HTTP 契约

- 状态：Proposed
- 决策：Go/Python/Java Runtime SDK 只依赖 OIDC Discovery、JWKS、RFC 6749/7662/7009/9068 与公开 HTTP 行为，不依赖 Rust crate、SeaORM 模型、`serde_json::Value` 形状或内部 domain。
- 后果：服务端可自由重构；需要维护 fixtures 与契约测试；Runtime 不要求 OpenAPI。

### ADR-002：Runtime 手写，Management 未来生成

- 状态：Proposed
- 决策：Runtime 的 OIDC/PKCE/JWKS/token 刷新/backchannel logout 由三语言手写 facade + 成熟 JOSE 库实现；未来 Management CRUD 才由 OpenAPI 生成。
- 后果：协议正确性可控；Runtime 构建链不引入生成器；生成器只负责未来管理面的机械代码。

### ADR-003：OpenAPI 3.1 作为未来 Management 契约

- 状态：Proposed
- 决策：未来 Management SDK 的 canonical spec 用 OpenAPI 3.1；为不支持 3.1 的生成器自动降级生成 3.0.3。Runtime SDK 不依赖该 spec。
- 后果：需要测试降级后的 nullable/oneOf/示例是否保真；禁止手改两份 spec；该决策不阻塞本期。

### ADR-004：不采用 UniFFI 作为 Runtime SDK 绑定层

- 状态：Proposed
- 决策：不采用 `mozilla/uniffi-rs`；Runtime SDK 走 HTTP + 标准协议。
- 理由：UniFFI 面向进程内 `cdylib`；Go/Java 非核心一方支持；仍处 `0.x`；需分发平台二进制；无法替代 HTTP 客户端的超时/代理/TLS/重试能力。
- 后果：三语言各自维护薄客户端；若未来需要本地复用 Rust，优先 sidecar，而非 FFI。

### ADR-005：Runtime SDK 首期只覆盖用户登录，不含 M2M

- 状态：Proposed
- 决策：首期不提供 `client_credentials`、权限同步、管理面能力；只覆盖 authorization code + PKCE、refresh、userinfo、JWKS、end-session、backchannel logout，以及可选的 introspection/revocation。
- 理由：M2M 当前是 legacy 有限实现，managed 路径未完成；强行纳入会扩大契约改造面并稀释首期价值。
- 后果：Runtime SDK 的公开 API 不出现服务账号概念；M2M 若未来要做，作为独立产品线重新立项。

---

## 附录 B：首期最小 PR 清单

如果要把调研转成实施，建议按以下顺序拆 PR，先证明 Runtime 契约闭环：

1. **Discovery 修正**：校准 `grant_types_supported`（只保留 `authorization_code` / `refresh_token`）、`token_endpoint_auth_methods_supported`、`scopes_supported`，补 backchannel logout metadata；加契约测试。
2. **managed TokenResponse 固化**：字段集合、`scope` 语义与错误体对齐；加成功/失败 fixtures。
3. **managed 用户 token profile 固化**：`typ=at+jwt`、`aud`、`scope`、`client_id`、`jti`；加安全测试向量。
4. **Go SDK skeleton**：Discovery + AuthorizeURL/PKCE + ExchangeCode + Refresh + JWKS 校验 + 错误映射；用 mock server 跑通。
5. **fixtures 固化**：把 Go 使用的请求/响应/JWKS/logout token 抽到 `sdk/contract-fixtures/`，服务端 CI 校验真实响应。
6. **Python SDK**：复用 fixtures，补同步/异步与类型标注。
7. **Java SDK**：复用 fixtures，补 builder/AutoCloseable 与 Nimbus JOSE。

不要先做 `libs/api-contract` 或 OpenAPI emitter；它们属于未来 Management SDK。Runtime 的第一目标是让三语言对同一组协议行为达成一致。

---

## 附录 C：Go Runtime SDK 实施进展（2026-11-12）

首期 Go 参考实现已落地，范围严格限定为 Runtime：

- `sdk/go`：零第三方依赖的 Go module，覆盖 Discovery、Authorization Code + PKCE、Code Exchange、Refresh single-flight、UserInfo、JWKS 缓存与 RS256 校验、Access/ID Token 校验、Introspection、Revocation、End Session、Back-channel Logout 校验、OAuth/HTTP 错误映射。
- `examples/sdk/go`：可运行的 Web 示例（登录、回调、保护 API、退出、后通道退出）、mock issuer 契约测试，以及 `//go:build integration` 的真实联调测试。`go test ./...` 只跑离线契约测试，不依赖真实服务或 Docker；真实联调必须显式运行 `./run-integration.sh`，缺少配置会直接失败而不是跳过。脚本默认自建本地 harness（生成 `issuer = http://127.0.0.1:14000` 并启动真实服务）；设置 `YUFU_ISSUER` 后改为测试既有部署，不再启动本地服务。SDK 本身不读取环境变量、不提供默认 issuer。
- 真实联调覆盖：管理员登录、通过管理 API 注册 browser SPA 应用、Discovery、授权码 + PKCE、ID/access token 验签、错误 nonce/audience 拒绝、UserInfo、Refresh、Revoke 后 Refresh 失败、End Session URL。
- 服务端 Discovery 已补 `backchannel_logout_supported` 与 `backchannel_logout_session_supported`，并在 `tests/yufu/routes/identity_cors.rs` 增加断言。
- 未纳入：M2M/`client_credentials`、权限同步、`/enforce`、Management SDK。

验证命令：

```sh
(cd sdk/go && go test ./... && go vet ./...)
(cd examples/sdk/go && go test ./... && go vet ./...)
(cd examples/sdk/go && ./run-integration.sh)  # 需要 Docker；启动真实 yufu 联调
```

已知服务端限制：confidential client 的 `client_secret` 校验（bcrypt）会超过 `MAX_SECURITY_BUDGET`（2s），换 token 返回 `503 deadline_exceeded`。联调因此使用 public browser SPA client + PKCE；SDK 本身支持 `ClientSecret`，该问题不属于 SDK 范围。

下一步：把 Go 测试中的请求/响应/JWKS/logout token 抽到 `sdk/contract-fixtures/`，再以同一组 fixtures 驱动 Python（同步 + 异步）与 Java（Java 17 + Nimbus JOSE）实现。
