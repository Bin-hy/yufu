# 鉴权与权限 Roadmap（Casdoor 风格）

把 cobotsrv 演进成一个可以承载多个业务应用（cobotrix、未来还会有更多）的统一鉴权 + 权限平台，参考 Casdoor 的核心思路：**身份分散验证（每个应用本地验签 JWT）+ 权限集中决策（PDP API）+ 应用注册 / 资源 / 权限集中管理**。

## 目标终态

```
              ┌────────────────────────────────────────────┐
              │  cobotsrv（身份提供者 + 权限管理）          │
              │                                            │
              │  ─ OIDC: /authorize /token /jwks /userinfo │
              │  ─ Application 表（per-app cert）           │
              │  ─ Resource × Action × Permission           │
              │  ─ POST /api/v1/enforce  ← 集中 PDP         │
              │  ─ Casdoor 风格管理 UI                      │
              └─────┬────────────────────┬─────────────────┘
                    │ JWKS 拉取（启动）   │ enforce 调用（细粒度）
                    ↓                    ↓
              ┌──────────────┐    ┌──────────────┐
              │  cobotrix    │    │  其他业务应用 │
              │  ─ JWT 本地验│    │  （同一套接入）│
              │  ─ require_  │    └──────────────┘
              │    perm 守卫 │
              └──────────────┘
```

| 概念 | 在我们这里的归属 |
|------|------------------|
| Identity Provider (IdP) | cobotsrv |
| OAuth2/OIDC client | cobotrix（以及未来每个新应用） |
| Policy Administration Point (PAP) | cobotsrv 管理后台 |
| Policy Decision Point (PDP) | cobotsrv `/enforce` |
| Policy Enforcement Point (PEP) | 各业务应用本地 middleware + 路由守卫 |

## 阶段划分

| 阶段 | 范围 | 状态 | 解决 |
|------|------|------|------|
| 0. 止血 | cobotrix 自验 JWT（HS256 共享 secret） | ✅ 完成 | 业务接口裸奔 |
| 1. OIDC client + RS256 | cobotrix 走 OAuth2 Authorization Code + PKCE，access_token 转 RS256，靠 JWKS 验签 | ✅ 完成 | 跨应用密钥隔离、SSO 体验 |
| 2. Application + Permission（自研）| cobotsrv 加 Application 维度 + 独立 permission 表，提供 `/enforce` PDP | ⏳ 进行中 (2.A schema 已落) | 多应用权限隔离 + 集中决策 |
| 3. 管理 UI | Application 注册 / Resource 定义 / Permission 分配三页面 | ⏳ 待启动 | 可视化运维 |

---

## 阶段 0 — 已完成

**目的**：堵住 cobotrix 业务接口零鉴权的安全洞，统一前后端身份语义。

实际改动：

- [cobotsrv/src/route/proxy.rs](../cobotsrv/src/route/proxy.rs) — `/mgmt/*` 反代不再 strip `Authorization`，让下游能再验签；`X-User-Id`/`X-Username`/`X-Role-Id` 保留为 advisory enrichment
- [cobotrix/Cargo.toml](../cobotrix/Cargo.toml) — 加 `auth = { workspace = true }`
- [cobotrix/src/config.rs](../cobotrix/src/config.rs) — 新增 `[auth] { secret, exp_seconds }`，默认从 env `AUTH_SECRET` 读，与 cobotsrv 自动一致；移除已死的 `http.cobotsrv_url`
- [cobotrix/src/route/mod.rs](../cobotrix/src/route/mod.rs) — 拆 `get_protected_routes()`（device/task/remote）/ `get_public_routes()`（hello）
- [cobotrix/src/lib.rs](../cobotrix/src/lib.rs) — 构 `AuthState`，`auth::access_middleware::<IdType>` 套到 protected 路由；删旧 `/api/*` 反代挂载
- 删除 `cobotrix/src/route/auth_proxy.rs`（死代码）
- `conf/cobotrix.example.toml` 加 `[auth]` 注释段；新建 `conf/cobotnil/cobotrix.toml` 给 dev cluster

**遗留小事**（不阻塞 Phase 1）：

- handler 暂未读 `Claims<i32>` 取 user_id（Phase 2 接 permission 时必读）
- CORS `vec!["*"]` 写死，建议挪到 config
- 缺集成测试覆盖"无 token 401 / 有效 token 200"

---

## 阶段 1 — OIDC client + RS256

**目的**：让 cobotrix 与 cobotsrv 的密钥**真正解耦**——cobotrix 只持有 cobotsrv 的公钥（通过 JWKS 拉取），不再共享 HS256 secret。同时把前端登录从"裸的 username/password POST"换成标准 OIDC Authorization Code + PKCE。

### 1.1 cobotsrv：access_token 改用 RS256 签发

现状：cobotsrv 的所有 access_token 走 [TokenManager.generate_token](../libs/auth/src/token_manager.rs)（HS256）。OIDC `id_token` 已经走 [IdTokenSigner](../libs/auth/src/token_manager.rs)（RS256）。

改动：

- 在 `IdTokenSigner` 增加 `sign_access_token<IDTYPE>(...)` 方法，复用 `Claims<IDTYPE> { id, exp }` 结构（不是 OIDC ID Token claims，避免 audience 等约束影响业务 token），用 RS256 签
- cobotsrv 所有签 access_token 的位置（[`/api/v1/login`](../cobotsrv/src/route/auth.rs)、[`/oidc/token`](../cobotsrv/src/service/oidc.rs)、[`refresh`](../cobotsrv/src/route/auth.rs)）改用新方法
- `[auth].secret` 不再用于签 access_token，仅保留为兼容字段（Phase 2 移除）

### 1.2 libs/auth：JWKS 客户端 + RS256 验签

新增到 `libs/auth`：

- `JwksClient`：启动时从 `{cobotsrv_base}/.well-known/jwks.json` 拉公钥，缓存；按 `kid` 选 key；后台周期性刷新（默认 5 分钟）
- `access_middleware_rs256<IDTYPE>`：用 JWKS 验签 + 验 `exp` + （可选）验 `aud`
- 旧的 `access_middleware`（HS256）保留，兼容 cobotsrv 内部使用，标记 deprecated

### 1.3 cobotrix：切到 JWKS 验签

- `Cargo.toml`：保持 `auth = { workspace = true }`
- `config.rs`：去掉 `[auth].secret`/`exp_seconds`；改成 `[auth] { cobotsrv_base, jwks_refresh_secs, expected_audience }`
- `lib.rs::serve`：构造 `JwksClient`，等首次拉取成功（或失败时按策略：fail-fast or warn-and-retry）后再启动 HTTP server；中间件换成 `access_middleware_rs256`

### 1.4 cobotsrv 注册 cobotrix 为 OIDC client

cobotsrv 已有 `oidc_admin` 路由可创建 client。Phase 1 用启动期 seed（admin 手动建也行）：

- `conf/cobotsrv*.toml` 加可选段 `[[oidc.bootstrap_clients]] { client_id, client_secret, redirect_uris, scopes }`，启动时 upsert 到 `oidc_client` 表
- 默认带一项 `cobotrix`，redirect_uri = `http://localhost:5173/oauth/callback`（dev）

### 1.5 cobotrix 前端：Authorization Code + PKCE 流程

- 新增 `cobotweb/apps/cobotrix/src/auth/oidc.ts`
  - `redirectToLogin()`：生成 `code_verifier` + `code_challenge`，存 sessionStorage，跳 `cobotsrv/oidc/authorize`
  - `handleCallback(code)`：拿 verifier 调 `cobotsrv/oidc/token`，存 access_token / refresh_token / expires_at
  - `refreshAccessToken()`：在 expire 前自动 refresh
- 新增 `pages/callback.vue` 路由 `/oauth/callback`，处理 code 兑换
- 删除 `pages/login.vue` 自调 `/api/v1/login` 的逻辑，改成 "跳转到 cobotsrv 登录" 按钮
- `http.ts` 加 401 自动 refresh 一次后重试的拦截器

### 1.6 验收

- 浏览器访问 cobotrix → 自动跳 cobotsrv 登录 → 登录成功回跳 cobotrix `/oauth/callback` → 进 dashboard
- cobotrix 后端日志显示用 RS256 + 正确 kid 验签通过
- 把 cobotrix `[auth].cobotsrv_base` 指向错误地址 → 启动失败 / 中间件 503
- 用过期 token 调 `/devices` → 401 + 前端自动 refresh

---

## 阶段 2 — Application + Permission（自研）

**目的**：把权限模型从"全局 user × authority"扩到"per-application user × resource × action"，并提供集中的 PDP `/enforce` 给业务应用调用。

### 2.1 数据模型扩展（独立 permission 表，不动 authority）

设计修正：原计划是在 `authority` 表加列，落地时改为**新增独立 `permission` 表族**。理由：
- `authority` 表的语义是"HTTP route 级 RBAC"（uri+method），是 cobotsrv 自家路由的访问控制
- Phase 2 要的是"resource 级权限"（user × action × object），语义不同
- 强行复用一个表会让 uri+method 必须可空，且查询语义混乱
- 两套系统可以并存：cobotsrv 自家路由继续用 authority，跨应用 PDP 用 permission

实际表结构（已落地于 [cobotsrv/src/migration/m20260508_*](../cobotsrv/src/migration/)）：

```
application
  id PK
  app_key      UNIQUE   -- e.g. "cobotrix"
  name, description
  oidc_client_id FK→oauth_client (NULL = M2M-only / no browser identity)
  timestamps

permission
  id PK
  application_id FK→application  (required — 严格按应用隔离)
  name           UNIQUE within app  -- e.g. "device:read", "task:submit"
  description
  action         TEXT     -- e.g. "read", "submit"
  resource_type  TEXT?    -- e.g. "device", "task", NULL = action-only
  resource_pattern TEXT?  -- glob, e.g. "cobotlet-0:*", NULL = any
  timestamps

role_permission   (role_id, permission_id) PK
  纯 RBAC 绑定

user_permission   (user_id, permission_id, allow) PK
  ABAC 风格 explicit allow/deny override
```

`init_oidc_bootstrap_clients` 自动 upsert：每注册一个 OIDC client，同时建一个 `app_key = client_id` 的 application 行（即使 oauth_client 已存在也补建，方便升级）。若配置把已存在的 confidential client 标为 `public_client = true`，启动时自动升级（public_upgrade）并清空其 secret hash——因此升级部署**无需手动 SQL**。

> **public client 与 client_credentials 互斥**：public client（PKCE、无 secret）无法通过 secret 校验，`client_credentials` 授权对它必然失败。bootstrap 启动时若检测到 `public_client = true` 且 grant_types 含 `client_credentials` 会直接报错拒绝启动。需要后端机器间调用（如 cobotsrv 启动时同步权限）的服务应拆两个 client：
> - SPA 用 public client（授权码 + PKCE）
> - 后端用独立 confidential machine client（如 `cobotsrv-sync`，仅 `client_credentials`）
>
> `POST /api/v1/application/sync-permissions` 支持 `app_key` 字段：machine client 认证后把权限注册到 `app_key` 指定的 application（须已存在），实现"认证 client ≠ 权限归属 client"的解耦（cobotsrv 传 `app_key = "cobotsrv"`）。

### 2.2 access_token 携带 roles + permissions（粗粒度）

- 签 token 时根据 `aud`（target application）从 `user_authority`/`role_authority` 拉本应用相关的 `roles`/`permissions` 数组塞进 claims
- 业务应用 middleware 把这些放入 request extension，handler 用 `require_perm!("device:read")` 宏离线判断（O(1)）

### 2.3 PDP `/enforce`

- `POST /api/v1/enforce`，body：
  ```json
  { "application": "cobotrix", "subject": 42, "action": "submit", "object": "task:cobotlet-0:lift" }
  ```
- 返回：`{ "allow": true, "reason": "matched permission #17" }`
- 实现：查 user_authority + role_authority + matching authority 行，pattern 匹配（glob 或简单正则）
- 客户端缓存：`libs/auth` 加 `EnforceClient`，5s LRU 缓存

### 2.4 cobotrix 接入

- handler 内细粒度判断：
  ```rust
  async fn submit(claims: Claims<i32>, enforce: EnforceClient, ...) {
      enforce.check("submit", &format!("task:{alias}:{cap}")).await?;
      ...
  }
  ```
- 粗粒度路由层（permission claim 已在 token 里）：
  ```rust
  Router::new()
      .route("/devices", get(list).layer(require_perm!("device:read")))
  ```

### 2.5 验收

- 在 cobotsrv 给某 user 分配 `cobotrix:device:read` 权限，刷新 token 后 cobotrix `GET /devices` 通过；撤销后 403
- `POST /tasks` 走 `/enforce` 拉取 `task:{alias}:{cap}` 决策；改 authority 后 5s 内生效（受缓存 TTL）

---

## 阶段 3 — 管理 UI

cobotsrv 前端（`cobotweb/apps/cobotsrv`）加三个页面，照 Casdoor 删繁就简：

1. **Applications**：列表 / 新建（自动生成 client_id+secret，导出 JWKS URL 给应用）
2. **Permissions**：选 Application → 定义 Resource type / Action / Pattern，分配给 Role 或 User
3. **Authorized apps for me**：当前用户已授权过哪些 OIDC client（其实 cobotsrv 已经有 `/oidc/authorized-apps`，挂个页面即可）

不做 Casdoor 的：组织、成员邀请、登录页主题、第三方 IdP 接入、SMS/Email 验证 —— 这些超出当前规模需求。

---

## 跨阶段约定

- **Token 类型**：access_token JWT (RS256, exp 短，2h)；refresh_token opaque (DB 存)；id_token JWT (RS256, OIDC consumer 用)
- **kid 命名**：`cobotsrv-rsa-1`，rotate 时 `-2`，旧 kid 在 JWKS 里保留 24h 后下线
- **错误约定**：401 = token 无效/过期；403 = token 有效但权限不足；422 = `/enforce` 自身参数错
- **PDP 缓存**：所有调用方默认 5s LRU；管理后台改权限后给敏感操作的应用主动推 invalidate（可选，Phase 3 再做）
