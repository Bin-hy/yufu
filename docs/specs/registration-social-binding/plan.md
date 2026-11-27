# 邀请注册同步绑定与邀请记录 Plan

> 输入：`docs/specs/registration-social-binding/spec.md`
> 状态：已实现，验证通过（2026-11-27）
> 约束：不新增数据库表或列；沿用现有邀请、外部身份、审计与权限模型。

## 架构概览

本次改动分为四条互相衔接的链路：

1. **邀请接受原子登录**：新建账户接受邀请时，在同一个身份事务中创建账户、消费邀请、创建中心会话，并在存在 OIDC 流程时把流程绑定到新会话。路由层负责下发会话 Cookie。
2. **注册页同步绑定**：注册页读取服务端 Provider catalog，提交时携带可选的 `bind_provider`；接受成功后前端用新会话立即发起对应 Provider 的绑定授权，授权回调继续原 OIDC 流程或进入身份页。
3. **绑定续接与失败落点**：外部绑定流继续复用 `identity_external_flow.oidc_flow_id`；绑定成功、拒绝或回调失败时，都能根据外部流上下文回到 OIDC 同意页或身份页，而不是统一落到登录页。
4. **邀请记录与重新签发**：管理端新增邀请列表读取和重新签发；列表按 `(created_at, id)` 游标分页，状态由服务端派生；重新签发只创建新记录，旧记录与旧链接保持不变。

不改变下游 OIDC 协议、账户中心绑定/解绑语义、已有账户接受邀请流程和密码体系。

## 核心数据结构

### `InvitationStatus`

放在 `domain/identity/account.rs`，用于服务端派生状态和过滤校验。

```rust
pub enum InvitationStatus {
    Pending,
    Accepted,
    Revoked,
    Expired,
}
```

序列化为 `pending | accepted | revoked | expired`。派生优先级固定为：

```text
accepted > revoked > expired > pending
```

即 `consumed_at IS NOT NULL` 优先，其次 `revoked_at IS NOT NULL`，再次 `expires_at <= now`，否则待用。

### `InvitationListQuery`

```rust
pub struct InvitationListQuery {
    pub after: Option<String>,          // "{created_at_millis}:{uuid}"
    pub limit: u32,                     // 1..=50
    pub status: Option<InvitationStatus>,
    pub purpose: Option<InvitationPurpose>,
    pub person_kind: Option<PersonKind>,
    pub search: Option<String>,
    pub now_millis: u64,
}
```

游标使用 `created_at_millis` 与 `id` 的组合，避免同一毫秒内多条记录在分页时重复或遗漏。公开 API 的时间字段仍返回秒级时间戳，与现有前端一致。

### `InvitationRow` / `InvitationView` / `InvitationPage`

- `InvitationRow`：仓储层内部结构，保留 `created_at` 毫秒值用于生成游标。
- `InvitationView`：路由响应结构，时间字段为秒，字段列表严格限定为列表所需元数据：

```text
id, purpose, person_kind, developer_grant,
target_user_id, target_username, target_nickname,
created_by, creator_username, creator_nickname,
created_at, expires_at, status,
accepted_user_id, accepted_username, accepted_nickname, accepted_at,
revoked_at
```

不包含 `secret_digest`、secret、Provider token、邮箱或其他无关资料。

- `InvitationPage { items: Vec<InvitationView>, next: Option<String> }`。

### `AcceptedInvitation` / `IssuedBrowserSession`

`AccountService::accept_invitation` 的返回值从 `AccountContinuation` 改为：

```rust
pub struct AcceptedInvitation {
    pub continuation: AccountContinuation,
    pub session: Option<IssuedBrowserSession>,
}

pub struct IssuedBrowserSession {
    pub secret: IssuedSecret,
    pub expires_at: u64,
}
```

- 新账户返回 `session = Some(...)`，`requires_login = false`。
- 已有账户返回 `session = None`，`requires_login = false`。
- `session.secret` 只用于路由层设置 HttpOnly Cookie，不进入 JSON。

### `AccountContinuation`

增加可选字段：

```rust
pub bind_provider: Option<ExternalProviderKey>,
```

仅新建账户且选择 Provider 时返回值；其他入口为 `None`。前端使用服务端确认的 Provider key 发起绑定。

### `InvitationAcceptance`

新建账户分支增加：

```rust
bind_provider: Option<ExternalProviderKey>,
```

仅用于服务端校验后的审计记录；实际绑定不在邀请接受事务内执行。

### `ExternalCompletion`

调整外部回调结果，使绑定也能携带 OIDC 续接：

```rust
pub enum ExternalCompletion {
    Login { cookie_secret, expires_at, oidc_flow_id },
    Bind { provider_key, oidc_flow_id: Option<Uuid> },
    Rejected {
        purpose: ExternalPurpose,
        code: &'static str,
        oidc_flow_id: Option<Uuid>,
    },
}
```

`Bind` 和绑定相关的 `Rejected` 都携带原 OIDC 流程标识；登录相关结果保持现有语义。

### 前端类型

`web/src/pages/admin/api/type.ts`：

```ts
type InvitationStatus = 'pending' | 'accepted' | 'revoked' | 'expired'
interface InvitationRecord {
  id: string
  purpose: 'onboarding' | 'qualification'
  person_kind: 'employee' | 'collaborator'
  developer_grant: boolean
  target_user_id: number | null
  target_username: string | null
  target_nickname: string | null
  created_by: number
  creator_username: string
  creator_nickname: string
  created_at: number
  expires_at: number
  status: InvitationStatus
  accepted_user_id: number | null
  accepted_username: string | null
  accepted_nickname: string | null
  accepted_at: number | null
  revoked_at: number | null
}
```

`web/src/pages/account/api/actions.ts` 的接受请求增加 `bind_provider?: string`；`identities.ts` 的 `startExternalBind` 增加可选 `flow_id`。

## 模块设计

### 身份接受服务

**文件：** `yufu/src/service/identity/account_actions.rs`

**职责：**

- 新建账户接受邀请时生成会话 secret 和 root id。
- 在 `AcceptInvitation` 工作单元中：
  1. 校验并消费邀请；
  2. 创建账户；
  3. 创建中心会话 `CreateCenter`；
  4. 若动作流携带 OIDC 流程，校验后执行 `BindFlow { reauthenticated: true }`；
  5. 写 `account.invitation.accept` 与 `session.login` 审计；
  6. 返回账户、OIDC 续接和会话材料。
- 已有账户分支不创建新会话，保持现有接受逻辑。

**关键约束：**

- 密码哈希仍在事务外完成。
- 会话创建、邀请消费、账户创建必须同一事务提交。
- 账户停用、邀请过期/撤销/已消费、OIDC 流程不匹配时整体失败。
- `bind_provider` 只写入审计摘要，不在此事务内发起第三方请求。

### 身份路由

**文件：** `yufu/src/route/identity_actions.rs`

**职责：**

- `AcceptRequest` 增加 `bind_provider: Option<String>`。
- 新建账户请求解析并校验 Provider：
  - 必须是浏览器型 Provider（非 native）；
  - 必须存在于当前 runtime registry 且已启用/配置完整；
  - 已有账户请求携带 Provider 时返回 `InvalidInput`。
- 接收 `AcceptedInvitation` 后：
  - 清除 action Cookie；
  - 使用 `browser::authenticated_cookies` 生成会话 Cookie 与 CSRF Cookie；
  - 合并多组 `Set-Cookie` 后返回 JSON continuation。

### 邀请管理服务

**文件：** `yufu/src/service/identity/account_invitations.rs`（新建）

**职责：**

- `list_invitations`：
  - 读取当前中心会话 actor；
  - 要求管理员且具有 `IdentityOperation::AccountRead`；
  - 校验分页/筛选；
  - 调用仓储查询；
  - 派生状态、生成下一页游标。
- `reissue_invitation`：
  - 锁定原邀请与目标账户；
  - 要求 `IdentityOperation::InvitationIssue`；
  - 读取原记录，复制用途、目标、人员类型、开发者资格；
  - 重新校验目标账户存在、启用且人员类型一致；
  - 使用当前 `invitation_ttl_seconds` 创建新记录；
  - 写 `account.invitation.reissue` 审计，摘要包含原邀请标识、目标账户和到期时间；
  - 返回一次性 secret。
- 不修改原记录，不撤销其他待用链接。

**模块注册：** 在 `yufu/src/service/identity/account.rs` 增加 `#[path = "account_invitations.rs"] mod invitations; pub use invitations::*;`。

### 邀请仓储

**文件：** `yufu/src/service/identity/repository_invitation.rs`（新建）

**职责：**

- 实现 `PgSecurityTransaction::identity_list_invitations(InvitationListQuery)`。
- 查询结构：

```sql
SELECT
  i.id, i.purpose, i.target_user_id, i.person_kind, i.developer_grant,
  i.created_by, i.created_at, i.expires_at, i.consumed_at,
  i.accepted_user_id, i.revoked_at,
  tu.username AS target_username, tu.nickname AS target_nickname,
  cu.username AS creator_username, cu.nickname AS creator_nickname,
  au.username AS accepted_username, au.nickname AS accepted_nickname,
  CASE
    WHEN i.consumed_at IS NOT NULL THEN 'accepted'
    WHEN i.revoked_at IS NOT NULL THEN 'revoked'
    WHEN i.expires_at <= $now THEN 'expired'
    ELSE 'pending'
  END AS status
FROM identity_invitation i
LEFT JOIN "user" tu ON tu.id = i.target_user_id
LEFT JOIN "user" cu ON cu.id = i.created_by
LEFT JOIN "user" au ON au.id = i.accepted_user_id
WHERE ...
ORDER BY i.created_at DESC, i.id DESC
LIMIT $limit_plus_one
```

- 过滤：
  - 状态：对上面的 `CASE` 结果过滤；
  - 用途、人员类型：精确匹配；
  - 搜索：目标账户、创建人、接受人的用户名/昵称，以及完整邀请 UUID；
  - 游标：`(i.created_at, i.id) < ($after_created, $after_id)`。
- 搜索沿用现有 `POSITION(lower($q) IN lower(...))` 方式，避免 LIKE 通配符注入。
- 不选择 `secret_digest`。

**接口注册：** 在 `IdentityTransaction` 增加 `list_invitations`，在 `repository_pg.rs` 委托到新实现，并在 `service/identity/mod.rs` 注册模块。

### 邀请管理路由

**文件：** `yufu/src/route/enterprise_admin.rs`

新增：

- `GET /api/v1/admin/invitations`
  - Query：`after`, `limit`, `status`, `purpose`, `person_kind`, `q`；
  - 读取 SSO Cookie；
  - 调用 `AccountService::list_invitations`；
  - 返回 `InvitationPage`，响应头使用受保护缓存策略。
- `POST /api/v1/admin/invitations/{id}/reissue`
  - 校验 CSRF/Origin；
  - 调用 `AccountService::reissue_invitation`；
  - 返回与签发一致的 `{ id, secret, expires_at }`；
  - 一次性 secret 只在本次响应出现。

现有 `POST /admin/invitations` 与 `POST /admin/invitations/{id}/revoke` 保持不变。

### 外部绑定服务

**文件：** `yufu/src/service/identity/external/service.rs`

**修改：**

- `start_bind` 增加 `flow_id: Option<Uuid>`，透传给 `start`。
- `StartExternalFlow` 在 `purpose == Bind` 且 `oidc_flow_id.is_some()` 时：
  - 加载授权流程；
  - 校验浏览器绑定；
  - 校验流程未完成、未绑定到其他中心会话；
  - 记录 `identity_external_flow.oidc_flow_id`。
- `CompleteExternal::bind`：
  - 在创建身份映射前校验 `flow.oidc_flow_id`：
    - 浏览器绑定一致；
    - 流程未完成；
    - 若已绑定中心会话，必须是当前 actor 的 root；
  - 成功或幂等绑定时返回 `ExternalCompletion::Bind { provider_key, oidc_flow_id }`；
  - 冲突/拒绝时返回带 `oidc_flow_id` 的 `Rejected`。
- 新增只读 `peek_flow_context(provider_key, state, browser)`：
  - 按 state 摘要加载外部流；
  - 校验 provider 与浏览器绑定；
  - 返回 purpose 与 `oidc_flow_id`，不消费流程。
  - 供回调错误分支决定跳转目标。

### 外部绑定路由

**文件：** `yufu/src/route/identity_external.rs`

**修改：**

- `bind_start` 接受可选 JSON：

```rust
#[derive(Deserialize)]
struct BindStartRequest { flow_id: Option<Uuid> }
```

使用可选 body，兼容旧前端无 body 的调用。

- `bind_start` 将 `flow_id` 传给服务。
- `callback`：
  - 成功 `Bind { oidc_flow_id: Some(id) }` → `/oidc/consent?flow_id={id}`；
  - 成功 `Bind { oidc_flow_id: None }` → `/account/identities?bound={provider}`；
  - 绑定 `Rejected` 带流程 → `/oidc/consent?flow_id={id}`；
  - 绑定 `Rejected` 无流程 → `/account/identities?error={code}`；
  - 登录 `Rejected` 保持 `/login?error={code}`；
  - 回调 `query.error` 或处理错误时：
    - 若存在 state 且 `peek_flow_context` 得到 Bind：
      - 有流程 → `/oidc/consent?flow_id={id}`；
      - 无流程 → `/account/identities?error={code}`；
    - 若得到 Login → `/login?error={code}`；
    - 无法读取上下文时，有中心 Cookie 则去 `/account/identities?error=external_authorization_denied`，否则回登录页。
- 所有跳转目标都是服务端硬编码路径，不接受客户端 URL。

### 注册页

**文件：** `web/src/pages/account/views/action.vue`

**职责：**

- 仅在 `kind === 'invitation' && !existing` 时读取 `getExternalProviders`。
- 只展示 `mode === 'oauth'` 的 Provider。
- 表单下方增加单选 Provider 卡片：
  - 默认不选，等同“仅创建账户”；
  - 再次点击可取消选择；
  - 使用品牌图标、名称、选中态、`aria-pressed`；
  - 读取失败时隐藏区域，不阻断注册。
- 提交时：
  - 请求体增加 `bind_provider`；
  - 成功后调用 `user.ensureSession()` 加载自动建立的中心会话；
  - 若返回 `bind_provider`：
    - 调用 `startExternalBind(provider, oidc_flow_id ?? undefined)`；
    - 成功跳转第三方授权；
    - 失败时提示“账户已创建，但绑定未开始”，然后按流程跳转 OIDC 或身份页。
  - 若无 Provider：
    - 有 OIDC 流程 → 同意页；
    - 否则 → 账户中心。
- `requires_login` 不再触发 `user.logout()`；该字段只作为兼容输出，新账户应为 false。

### 身份页

**文件：** `web/src/pages/account/views/identities.vue`

**职责：**

- 读取查询参数：
  - `bound={provider}`：成功后 toast；
  - `error={code}`：显示绑定失败提示；
  - `bind={provider}`：在提示中带上 Provider 名称，并保留重试入口。
- 加载失败仍保留页面和重试按钮。
- 不改变绑定/解绑确认逻辑。

### 管理端邀请列表

**文件：** `web/src/pages/admin/views/accounts.vue`

**职责：**

- 在一次性链接面板之后、账户列表之前增加“邀请记录”面板。
- 使用 `useCursorList<InvitationRecord, string>`：
  - 默认按创建时间倒序；
  - 每页 25；
  - 支持状态筛选、用途筛选、人员类型筛选、搜索；
  - 搜索做 240ms 防抖；
  - 筛选变化从第一页开始。
- 桌面表格列：
  - 用途/人员类型/开发者资格；
  - 目标账户；
  - 状态；
  - 到期时间；
  - 创建人/创建时间；
  - 接受人/接受时间或撤销时间；
  - 操作。
- 移动端使用卡片布局，状态和操作不被裁切。
- 待用记录显示“撤销”；所有记录显示“重新签发”。
- 撤销：
  - 二次确认；
  - 成功后刷新列表和一次性结果面板；
  - 失败保留列表并显示错误。
- 重新签发：
  - 二次确认；
  - 成功后把新 secret 放入现有一次性链接面板；
  - 刷新列表；
  - 不隐藏或修改原记录。
- 签发成功后也要刷新邀请列表。
- 新增入口按钮/空状态不改变现有账户操作。

### 管理端 API

**文件：** `web/src/pages/admin/api/index.ts`

新增：

```ts
listInvitations(after?, signal?, filter?) => Page<InvitationRecord, string>
reissueInvitation(id) => IssuedAction
```

参数为空时不发送空字符串；状态/用途/人员类型使用 `all` 时不传。

## 模块交互

### 新建账户 + 同步绑定

```text
注册页
  ├─ GET /identity/external/providers
  ├─ POST /identity/invitations/accept { ..., bind_provider? }
  │     └─ 事务：创建账户 + 消费邀请 + 创建中心会话 + 可选 BindFlow + 审计
  │     └─ 响应：Set-Cookie + { user_id, oidc_flow_id, requires_login:false, bind_provider? }
  ├─ GET /account/session（ensureSession）
  ├─ 若 bind_provider：
  │     POST /identity/external/{provider}/bind/start { flow_id? }
  │       └─ 创建 external flow（含 oidc_flow_id）
  │       └─ 302 第三方授权
  │     └─ 回调：
  │           success + flow → /oidc/consent?flow_id=...
  │           success 无 flow → /account/identities?bound=...
  │           denied/error + flow → /oidc/consent?flow_id=...
  │           denied/error 无 flow → /account/identities?error=...
  └─ 若无 bind_provider：
        flow → /oidc/consent
        无 flow → /account
```

### 邀请记录

```text
管理页
  ├─ GET /admin/invitations?status=&purpose=&person_kind=&q=&after=&limit=
  │     └─ AccountRead 授权 → 服务端派生状态 → 游标分页
  ├─ POST /admin/invitations/{id}/revoke
  │     └─ InvitationRevoke → 更新 revoked_at → 刷新列表
  └─ POST /admin/invitations/{id}/reissue
        └─ InvitationIssue → 新记录 + 新 secret → 一次性面板 + 刷新列表
```

## 文件组织

```text
yufu/src/
├── domain/identity/account.rs                    # InvitationStatus / 列表查询 / 视图 / 分页
├── service/identity/
│   ├── account.rs                                # 注册 account_invitations 子模块
│   ├── account_actions.rs                        # 接受邀请原子创建会话；审计
│   ├── account_invitations.rs                    # 列表与重新签发服务
│   ├── repository.rs                             # IdentityTransaction::list_invitations
│   ├── repository_pg.rs                          # 委托邀请查询
│   ├── repository_invitation.rs                  # 邀请列表 SQL
│   └── external/service.rs                       # bind 续接、flow 校验、peek 上下文
├── route/
│   ├── identity_actions.rs                       # 接受请求 Provider 校验 + 会话 Cookie
│   ├── enterprise_admin.rs                       # GET 列表 + POST 重新签发
│   └── identity_external.rs                      # bind start flow_id + 回调落点
└── service/identity/mod.rs                       # 注册 repository_invitation

web/src/
├── pages/account/api/actions.ts                  # accept 请求类型
├── pages/account/api/identities.ts               # startExternalBind flow_id
├── pages/account/views/action.vue                # Provider 卡片 + 自动绑定
├── pages/account/views/identities.vue            # bound/error 提示
├── pages/admin/api/index.ts                      # 邀请列表/重签发 API
├── pages/admin/api/type.ts                       # InvitationRecord
└── pages/admin/views/accounts.vue                # 邀请记录面板

tests/yufu/routes/
├── identity_actions.rs                           # 自动会话 + 原子性 + 并发
├── external_identity_github.rs                   # 注册同步绑定 + OIDC 续接
└── identity_invitations.rs                       # 列表/筛选/撤销/重签发（新建）

web/tests/unit/
├── identity-ui.test.ts                           # 注册 Provider 选择与自动绑定
└── admin-console.test.ts                         # 邀请列表、撤销、重签发

web/tests/e2e/enterprise-sso-enterprise-onboarding.spec.ts  # requires_login 断言更新
```

## 技术决策

| 决策点 | 选择 | 理由 |
|---|---|---|
| 自动会话放在哪里 | 放进 `AcceptInvitation` 同一工作单元 | 满足账户、邀请消费、会话原子性；避免“账户已建但登录失败” |
| 会话如何下发 | 服务返回 `IssuedBrowserSession`，路由设置 HttpOnly Cookie | secret 不进入 JSON；复用密码登录的 Cookie/CSRF 语义 |
| Provider 选择如何传给服务端 | 接受请求携带可选 `bind_provider`，服务端校验并回显 | 服务端不信任前端；非法 Provider 不创建账户 |
| 实际绑定是否放进接受事务 | 不放入 | 第三方网络不可与数据库事务绑定；绑定失败不能回滚已创建账户 |
| 绑定续接如何保存 | 复用 `identity_external_flow.oidc_flow_id` | 表和字段已存在；无需迁移 |
| 绑定成功如何续接 OIDC | 回调返回 `oidc_flow_id`，前端回到同意页 | 复用现有授权流程绑定与同意逻辑 |
| 绑定失败落点 | 根据外部流上下文区分 Bind/Login 与是否有流程 | 已登录用户不应被送去登录页；OIDC 流程不能丢失 |
| OIDC 流程何时绑定新会话 | 邀请接受事务内 `BindFlow(reauthenticated=true)` | 注册密码是一次全新认证；同时保证 prompt=login 语义 |
| 列表权限 | 管理员 + `AccountRead` | 列表是账户目录的只读视图；AC18 已明确账户读取权限 |
| 是否新增权限/表 | 否 | 现有权限、邀请表和外部流字段足够；减少迁移与授权风险 |
| 列表状态 | 服务端按时间与消费/撤销时间派生 | 避免客户端时钟偏差；状态优先级固定 |
| 列表分页 | `(created_at, id)` 复合游标倒序 | 新记录持续插入时仍稳定，不重复不遗漏 |
| 重新签发 | 新记录，旧记录与旧链接保留 | 用户明确选择多链接并行；避免误伤已发出的链接 |
| 重新签发权限 | `InvitationIssue` | 与签发同权；撤销继续使用 `InvitationRevoke` |
| 列表搜索 | 服务端 `POSITION(lower(...))` | 复用现有搜索风格，避免 LIKE 通配符注入 |
| 时间字段 | API 返回秒，内部游标用毫秒 | 与前端现有类型一致，同时保证分页精度 |
| 前端 Provider 展示 | 只显示 `mode === 'oauth'` | 本次仅支持浏览器扫码/OAuth；native 不在范围 |
| 绑定失败提示 | 主页 toast + 身份页查询参数 | 跨页面携带结果，避免静默失败 |
| 测试 Provider | 复用本地 fake GitHub/QQ/微信 | 不依赖真实第三方资质与网络 |

## 验证策略

- 后端：`cargo test --workspace`（按项目约定单线程）；新增测试覆盖自动会话、原子回滚、并发接受、Provider 校验、邀请列表/筛选/分页、撤销、重新签发、OIDC 续接和回调失败。
- 前端：`npm test`（Vitest）；新增注册 Provider 选择和邀请列表交互测试。
- 端到端：更新开户 E2E 的 `requires_login` 断言；用管理端 API 验证签发后列表可见、重新签发后旧链接仍可用。
- 安全检查：检查列表响应、审计摘要和日志无 secret/digest/token；外部回调无开放重定向。
