# 企业统一身份接入指南（OIDC）

本文面向在本企业内接入统一身份的应用开发者，说明 Provider 的定位、应用注册与审批、标准 OIDC 参数、两个参考示例的运行方式，以及保护 API 的在线检查边界。

## 1. 定位与边界

本 Provider 是**单企业独立部署**的统一身份与 SSO 提供方。一个进程同时提供三个入口：

- 账户中心：`/account/*`，本人资料、授权应用、同意、会话与 RP 会话。
- 开发者控制台：`/developer/*`，本人应用的注册、配置、版本、提交审批、成员与 Reader。
- 企业管理控制台：`/admin/*`，账户、应用、组、分配、审批、审计、投递与历史纳管。

接入采用**企业邀约/审批制**，不提供公开应用市场。当前明确不做：多租户、外部 IdP 联邦、公开开发者自助注册、强认证（WebAuthn 等）。这些能力在规格中列为后续范围，接入方不得依赖。

## 2. 接入前置

1. 管理员为开发者授予开发者资格；未获资格时 `/developer/*` 返回拒绝，不能通过旧接口绕过。
2. 开发者在 `/developer/apps` 注册应用，分别配置 `development` 与 `production` 环境。
3. 生产环境配置提交后进入审批；管理员在 `/admin/apps/:id` 审阅固定申请快照并批准。未批准的生产配置不能用于真实登录。
4. 管理员在 `/admin/apps/:id/assignments` 为账户或组分配应用；测试环境可额外授予测试用户访问。
5. 用户首次登录时在同意页确认请求的 scope；未获分配、未批准或未同意的请求不会签发 code。

开发环境用于联调，生产环境用于真实用户。两环境有独立的 client、回调白名单、access audience、secret 与配置版本；不要把开发凭据带到生产。

## 3. Discovery 与端点

Issuer 为部署的公开基址（生产必须 HTTPS）。Discovery 位于：

```
GET {issuer}/.well-known/openid-configuration
GET {issuer}/.well-known/jwks.json
```

当前发布的端点：

| 端点 | 路径 | 说明 |
| --- | --- | --- |
| Authorization | `/oidc/authorize` | 仅 `response_type=code`、`response_mode=query`、PKCE `S256` |
| Token | `/oidc/token` | `authorization_code` 与 `refresh_token` |
| UserInfo | `/oidc/userinfo` | Bearer access token |
| Introspection | `/oidc/introspect` | Reader 凭据（`client_secret_basic`/`post`） |
| Revocation | `/oidc/revoke` | 支持 access/refresh token |
| End session | `/oidc/end-session` | 仅在 RS256 签名时发布 |
| JWKS | `/.well-known/jwks.json` | RS256 公钥 |

支持的 scope 为 `openid`、`profile`、`email`。`openid` 必填。签名算法以部署配置为准；参考示例按 RS256 验证。

## 4. 客户端类型与回调

### 4.1 机密 Web 客户端

- 服务端持有 `client_secret`，使用 `client_secret_basic` 或 `client_secret_post` 兑换 code。
- 回调地址必须与注册值**逐字符一致**，包括 scheme、host、port、path，不允许通配或前缀匹配。
- 浏览器只接触 issuer、client id、redirect URI、scope；secret 与 reader 凭据不得进入前端。

### 4.2 公共 SPA 客户端

- 不配置 `client_secret`；授权码 + PKCE `S256`。
- code 兑换由 SPA 直接完成或由同站 BFF 完成；本仓库示例选择 SPA 直接兑换，配套 API 只校验 bearer，不代换 code。
- 回调页必须是 SPA 的注册回调；不要复用 Web 客户端的回调。

### 4.3 CORS 与 Origin

- 保护 API 只接受注册的 SPA origin。请求带 `Origin` 时必须精确匹配 scheme/host/port；不匹配直接 403，且在读取 token 之前拒绝。
- 预检只允许 `authorization`、`content-type` 请求头与 `POST, OPTIONS` 方法。
- 中心的管理/账户接口使用同源 Cookie + CSRF，不使用旧 Bearer；跨站调用不应依赖中心 Cookie。

## 5. 身份映射与令牌语义

- `sub` 是稳定主体标识；同一 `(iss, sub)` 在 A、B 两个应用间保持一致。应用应把 `iss + sub` 作为本地用户键，不要用 email 或 username 作为主键。
- `profile` scope 披露 `name`、`preferred_username`；`email` scope 披露 `email`、`email_verified`。未授予的 scope 不披露对应字段。
- ID Token 的 `aud` 是客户端 id，包含 `nonce`、`auth_time`、`sid`；`sid` 是协议会话标识，用于退出联动。
- Access Token 的 `typ` 为 `at+jwt`，`aud` 是应用环境的 access audience（例如 `urn:app:production`），不是 client id。保护 API 必须同时校验签名、issuer、audience、`typ` 与有效期。
- 中心内省响应包含 `active`、`scope`、`client_id`、`username`、`token_type`、`exp`、`sub`；不保证返回协议 `sid`。需要按会话清理缓存时，同时按 `sid` 与 `sub` 清理。
- Access Token 中的 `session_id` 是凭据会话 id，不是协议 `sid`；不要把两者混用。

## 6. Web 示例（`examples/oidc-web`）

机密 Web 客户端，服务端会话 + 服务端持有 token。

### 6.1 配置

复制 `examples/oidc-web/.env.example` 并填写：

```dotenv
ISSUER=https://id.example.com
CLIENT_ID=your-web-client
CLIENT_SECRET=issued-secret
REDIRECT_URI=https://app.example.com/callback
POST_LOGOUT_REDIRECT_URI=https://app.example.com/
SCOPE=openid profile email
SESSION_SECRET=at-least-32-random-characters
READER_CLIENT_ID=your-reader
READER_CLIENT_SECRET=issued-reader-secret
ALLOW_INSECURE_LOOPBACK=false
PORT=3000
```

`ALLOW_INSECURE_LOOPBACK=true` 仅用于 `127.0.0.1`/`localhost` 本地联调；生产必须为 `false`。

### 6.2 运行

```bash
pnpm install
pnpm --filter @cobotrix/oidc-web build
pnpm --filter @cobotrix/oidc-web start
```

### 6.3 流程

1. `GET /login` 生成 state、nonce、PKCE verifier，写入待处理 Cookie 后 302 到 `/oidc/authorize`。
2. 回调校验 state、nonce、code、issuer、audience 与 ID Token 签名，服务端兑换 token。
3. 本地会话只保存业务资料与服务端 token；浏览器 Cookie 为 HttpOnly。
4. `POST /api/operation` 先校验同源与 CSRF，再做在线内省，最后执行受保护写入。
5. `POST /logout` 只清本应用会话并尽力撤销本应用授权；`GET /logout/central` 走 RP 发起的中心退出。
6. `POST /oidc/backchannel-logout` 校验 logout token，按 `sid`/`sub` 清理本地会话与授权缓存，重复 `jti` 幂等确认。

## 7. SPA 示例（`examples/oidc-spa`）

公共 SPA + 独立保护 API。浏览器包只包含 `VITE_*` 公共参数；Reader 凭据只在 API 进程。

### 7.1 配置

公共参数（构建时注入）：

```dotenv
VITE_ISSUER=https://id.example.com
VITE_CLIENT_ID=your-spa-client
VITE_REDIRECT_URI=https://spa.example.com/callback
VITE_POST_LOGOUT_REDIRECT_URI=https://spa.example.com/
VITE_SCOPE=openid profile email
VITE_API_BASE_URL=https://api.example.com
```

API 进程参数：

```dotenv
API_AUDIENCE=urn:app:production
SPA_CLIENT_ID=your-spa-client
READER_CLIENT_ID=your-reader
READER_CLIENT_SECRET=issued-reader-secret
ALLOWED_ORIGIN=https://spa.example.com
ALLOW_INSECURE_LOOPBACK=false
PORT=3001
```

### 7.2 运行

```bash
pnpm install
pnpm --filter @cobotrix/oidc-spa build
pnpm --filter @cobotrix/oidc-spa start:api
pnpm --filter @cobotrix/oidc-spa dev
```

### 7.3 流程

1. SPA 使用 `oidc-client-ts` 发起 code + PKCE；`sessionStorage` 隔离标签页，不启用 iframe 静默续期。
2. 回调完成 state/nonce 校验后保存用户；刷新只走 refresh token，本标签页内 single-flight，避免并发刷新。
3. `POST {API}/api/operation` 携带 `Authorization: Bearer <access_token>`。
4. API 先本地校验 RS256、issuer、`aud=API_AUDIENCE`、`typ=at+jwt`、有效期；再用 Reader 凭据内省，确认 `active`、`client_id=SPA_CLIENT_ID`、`sub` 与 JWT 一致。
5. 退出：本地清用户；中心退出走 RP 发起端点；后通道通知到达时按 `sid`/`sub` 清缓存。

## 8. 保护 API 的在线检查

两个示例共用同一套语义：

- 正向结果默认缓存 **2 秒**；超过 2 秒必须重新内省。
- 任何结果（含层间传递）绝对年龄不得超过 **5 秒**；超过即失败关闭。
- 年龄从**内省请求开始**的单调时刻计算，慢内省不会获得新的 2 秒窗口。
- 层间传递保留原始 `verifiedAt`，接收层不得重置 TTL。
- token 已过期、内省不可用、`active=false`、client 不匹配、subject 不匹配都拒绝；拒绝时不执行受保护副作用。
- 后通道退出按 `sid` 与 `sub` 清理；由于中心内省不保证返回 `sid`，只按 `sid` 清理是不够的。

5 秒是硬上限，不是“每 5 秒刷新一次”的授权周期。需要更强实时性的写操作应缩短缓存或直接内省。

## 9. 退出语义

- 本地退出：只清本应用会话，不影响其他应用。
- 当前根退出：清当前中心浏览器会话及其派生的 RP 会话。
- 所有根退出：清该用户全部中心根与 RP 会话。
- RP 发起退出：使用 `id_token_hint` 与 `post_logout_redirect_uri`，端点由 discovery 发布。
- 后通道退出：`typ=logout+jwt`、RS256、`aud=client_id`、`events` 仅含后通道事件、必须有 `jti` 与 `sid` 或 `sub`、不得含 `nonce`、`exp-iat ≤ 120` 秒。接收方必须校验签名/issuer/audience/事件/时效，并按 `jti` 幂等。

## 10. 验证与当前证据

示例工程的当前证据：

```bash
pnpm check:examples
pnpm test:examples
pnpm build:examples
```

截至 2026-11-09：`oidc-web` 54 项、`oidc-spa` 59 项单测通过，两包类型检查与构建通过；SPA 产物扫描不含 Reader secret 或 Reader client id。示例的缓存时限由可控单调时钟单测覆盖。

**尚未完成**：真实跨站 TLS 与 Chromium/Firefox/WebKit 三引擎的 A→B SSO、新标签页、退出联动验收（T95—T98），以及真实 HTTPS 后通道成功投递。本文不声称这些场景已通过；在补齐前，94 项验收清单保持未勾选。

## 11. 常见错误

- 把 client id 当作 access token 的 `aud`：会拒绝所有保护请求。应使用环境注册的 access audience。
- 在 SPA 中放 `client_secret` 或 Reader 凭据：会进入浏览器包，属于凭据泄漏。
- 回调地址使用通配或尾斜杠不一致：授权请求会被拒绝。
- 只按 `sid` 清理授权缓存：中心内省可能没有 `sid`，必须同时按 `sub`。
- 把 ID Token 当 access token 调用 API：`typ` 不是 `at+jwt`，必须拒绝。
- 用旧 Bearer 访问中心管理接口：中心控制台使用同源 Cookie + CSRF，旧路径不作为接入方式。
