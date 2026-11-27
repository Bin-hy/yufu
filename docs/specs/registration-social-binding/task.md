# 邀请注册同步绑定与邀请记录 Tasks

> 输入：`spec.md`、`plan.md`
> 状态：已执行（2026-11-27）
> 执行原则：每项任务完成后运行验证命令；未通过不进入下一项。全量测试按项目约定使用 `--test-threads=1`。

## 文件清单

| 操作 | 文件 | 职责 |
|---|---|---|
| 修改 | `yufu/src/domain/identity/account.rs` | 邀请状态、列表查询、列表视图、分页结构 |
| 修改 | `yufu/src/service/identity/account_actions.rs` | 接受邀请原子创建会话、Provider 审计、重新签发前的类型调整 |
| 修改 | `yufu/src/service/identity/account.rs` | 注册 `account_invitations` 子模块 |
| 新建 | `yufu/src/service/identity/account_invitations.rs` | 邀请列表与重新签发服务 |
| 修改 | `yufu/src/service/identity/repository.rs` | `list_invitations` 仓储接口 |
| 修改 | `yufu/src/service/identity/repository_pg.rs` | 委托邀请列表查询 |
| 新建 | `yufu/src/service/identity/repository_invitation.rs` | 邀请列表 SQL 与游标 |
| 修改 | `yufu/src/service/identity/mod.rs` | 注册仓储模块 |
| 修改 | `yufu/src/route/identity_actions.rs` | 接受请求 Provider 校验与会话 Cookie |
| 修改 | `yufu/src/route/enterprise_admin.rs` | 邀请列表与重新签发路由 |
| 修改 | `yufu/src/route/identity_external.rs` | bind start flow_id、回调落点 |
| 修改 | `yufu/src/service/identity/external/service.rs` | bind 续接、流程校验、回调上下文 |
| 修改 | `web/src/pages/account/api/actions.ts` | 接受请求类型 |
| 修改 | `web/src/pages/account/api/identities.ts` | bind start flow_id |
| 修改 | `web/src/pages/account/views/action.vue` | Provider 卡片、自动登录与绑定 |
| 修改 | `web/src/pages/account/views/identities.vue` | 绑定成功/失败提示 |
| 修改 | `web/src/pages/admin/api/index.ts` | 邀请列表/重签发 API |
| 修改 | `web/src/pages/admin/api/type.ts` | 邀请记录类型 |
| 修改 | `web/src/pages/admin/views/accounts.vue` | 邀请记录面板与操作 |
| 修改 | `tests/yufu/routes/identity_actions.rs` | 自动会话、原子性、并发、权限 |
| 新建 | `tests/yufu/routes/identity_invitations.rs` | 邀请列表/撤销/重签发 |
| 修改 | `tests/yufu/routes/external_identity_github.rs` | 注册同步绑定与 OIDC 续接 |
| 修改 | `web/tests/unit/identity-ui.test.ts` | 注册 Provider 与自动绑定 |
| 修改 | `web/tests/unit/admin-console.test.ts` | 邀请列表、撤销、重签发 |
| 修改 | `web/tests/e2e/enterprise-sso-enterprise-onboarding.spec.ts` | `requires_login` 与列表验证 |
| 修改 | `docs/operations/social-login.md` | 注册同步绑定与邀请记录运维说明 |

## T1 定义邀请列表领域类型

**文件：** `yufu/src/domain/identity/account.rs`
**依赖：** 无
**步骤：**
1. 新增 `InvitationStatus`，包含 `Pending/Accepted/Revoked/Expired`，实现 `as_str` 和 `serde::Serialize`（snake_case）。
2. 新增 `InvitationListQuery`，包含 `after: Option<String>`、`limit: u32`、`status`、`purpose`、`person_kind`、`search`、`now_millis`。
3. 新增内部 `InvitationRow` 与公开 `InvitationView`，字段按 plan 定义；`InvitationRow` 保留 `created_at_millis`。
4. 新增 `InvitationPage`，包含 `items` 与 `next`。
5. 为 `InvitationStatus` 添加 `from_str` 或 `TryFrom<&str>`，只接受四个固定值。

**验证：** `cargo check -p yufu` 通过；`cargo test -p yufu --lib account` 中现有领域测试通过。

## T2 扩展接受邀请返回类型

**文件：** `yufu/src/service/identity/account_actions.rs`
**依赖：** T1（同文件类型）
**步骤：**
1. 新增 `IssuedBrowserSession { secret: IssuedSecret, expires_at: u64 }`。
2. 新增 `AcceptedInvitation { continuation: AccountContinuation, session: Option<IssuedBrowserSession> }`。
3. 给 `AccountContinuation` 增加 `bind_provider: Option<ExternalProviderKey>`。
4. 给 `InvitationAcceptance::NewAccount` 增加 `bind_provider: Option<ExternalProviderKey>`。
5. 更新所有 `AccountContinuation` 构造点，默认 `bind_provider: None`。

**验证：** `cargo check -p yufu`；编译错误清零。

## T3 生成接受邀请会话材料

**文件：** `yufu/src/service/identity/account_actions.rs`
**依赖：** T2
**步骤：**
1. 在 `accept_invitation` 中、进入工作单元前生成 `session_entropy: [u8; 32]`、`session_root_id: Uuid` 和 `authenticated_at`。
2. 从 `self.config.identity.sso_session_ttl_seconds` 计算 `session_expires_at`，溢出返回 `CapacityExceeded`。
3. 将上述值传入 `AcceptInvitation` 工作单元。
4. 已有账户分支不生成新会话，`session` 返回 `None`。
5. 新账户分支返回 `session: Some(IssuedBrowserSession { secret, expires_at })`。

**验证：** `cargo check -p yufu`；单元测试编译通过。

## T4 在邀请接受事务中创建中心会话

**文件：** `yufu/src/service/identity/account_actions.rs`
**依赖：** T3
**步骤：**
1. 将 `AcceptInvitation` 的 `type Output` 改为 `AcceptedInvitation`。
2. 创建账户并消费邀请后，构造 `BrowserSessionSnapshot`：
   - `id = session_root_id`
   - `principal = stable_principal(issuer, account.user_id)`
   - `secret_digest = session_secret.digest()`
   - `auth_epoch = account.auth_epoch`
   - `authenticated_at`、`expires_at` 使用传入值。
3. 调用 `IdentityWrite::CreateCenter(root)`。
4. 若动作流携带 OIDC 流程：
   - 加载授权流程；
   - 校验浏览器绑定、未完成、未绑定到其他会话；
   - 调用 `IdentityWrite::BindFlow { reauthenticated: true }`。
5. 写入 `account.invitation.accept` 审计；新账户额外写入 `session.login` 审计。
6. 审计摘要包含 `created`、`developer_grant`、`provider`（如有）。
7. 返回 `AcceptedInvitation`，`requires_login = false`。

**验证：** `cargo test -p yufu --lib account` 通过；`cargo check -p yufu`。

## T5 接受路由校验 Provider 并下发 Cookie

**文件：** `yufu/src/route/identity_actions.rs`
**依赖：** T4
**步骤：**
1. `AcceptRequest` 增加 `bind_provider: Option<String>`，保持 `deny_unknown_fields`。
2. 解析 `ExternalProviderKey`；拒绝 native Provider。
3. 通过 `state.external_registry()` 确认 Provider 已启用且配置完整；失败返回 `InvalidInput`。
4. 已有账户请求携带 `bind_provider` 时返回 `InvalidInput`。
5. 调用 `accept_invitation` 获取 `AcceptedInvitation`。
6. 清除 action Cookie；若存在会话，调用 `browser::authenticated_cookies`。
7. 合并所有 `Set-Cookie`，返回 continuation JSON（不包含会话 secret）。
8. 新账户 `requires_login=false`。

**验证：** `cargo test --test mod identity_actions -- --test-threads=1`；先允许旧断言失败，下一步更新。

## T6 更新接受邀请后端测试

**文件：** `tests/yufu/routes/identity_actions.rs`
**依赖：** T5
**步骤：**
1. 把新账户接受后的 `requires_login` 期望从 `true` 改为 `false`。
2. 断言响应包含中心 SSO Cookie 和 CSRF Cookie，且 action Cookie 被清除。
3. 使用响应 Cookie 请求 `/api/v1/account/session`，断言 200 且 `user_id` 匹配。
4. 增加“非法 `bind_provider` 不创建账户、不消费邀请”的用例。
5. 增加“同一邀请并发接受只成功一次”的用例。
6. 保留已有账户接受路径断言不变。

**验证：** `cargo test --test mod identity_actions -- --test-threads=1` 通过。

## T7 增加邀请列表仓储接口

**文件：** `yufu/src/service/identity/repository.rs`、`yufu/src/service/identity/repository_pg.rs`、`yufu/src/service/identity/mod.rs`
**依赖：** T1
**步骤：**
1. 在 `IdentityTransaction` 增加 `async fn list_invitations(&mut self, query: InvitationListQuery) -> Result<Vec<InvitationRow>, IdentityError>`。
2. 在 `repository_pg.rs` 实现委托到 `self.identity_list_invitations(query).await`。
3. 在 `service/identity/mod.rs` 注册 `mod repository_invitation;`。

**验证：** `cargo check -p yufu` 通过。

## T8 实现邀请列表 SQL

**文件：** `yufu/src/service/identity/repository_invitation.rs`（新建）
**依赖：** T7
**步骤：**
1. 解析游标 `"{millis}:{uuid}"`；非法返回 `InvalidInput`。
2. 查询 `identity_invitation` 并左连接目标、创建人、接受人 `user`。
3. 用 `CASE` 派生状态，优先级 accepted > revoked > expired > pending。
4. 应用状态、用途、人员类型、搜索和游标过滤。
5. 搜索匹配目标/创建人/接受人的用户名、昵称和完整邀请 UUID。
6. 按 `created_at DESC, id DESC` 排序，`LIMIT limit + 1`。
7. 不查询 `secret_digest`。

**验证：** `cargo check -p yufu`；后续服务测试覆盖。

## T9 实现邀请列表服务

**文件：** `yufu/src/service/identity/account_invitations.rs`（新建）、`yufu/src/service/identity/account.rs`
**依赖：** T8
**步骤：**
1. 在 `account.rs` 注册 `#[path = "account_invitations.rs"] mod invitations; pub use invitations::*;`。
2. 实现 `AccountService::list_invitations(secret, query, budget)`。
3. 校验 `limit` 为 1..=50；校验状态字符串。
4. 读取当前 actor，要求管理员且具有 `IdentityOperation::AccountRead`。
5. 调用仓储查询，最多取 `limit+1` 条。
6. 将 `InvitationRow` 转为 `InvitationView`，时间字段除以 1000。
7. 根据最后一条可见记录生成 `next` 游标；超过限制时截断。
8. 回滚只读事务。

**验证：** `cargo check -p yufu`；`cargo test -p yufu --lib account`。

## T10 实现重新签发工作单元

**文件：** `yufu/src/service/identity/account_invitations.rs`
**依赖：** T9
**步骤：**
1. 实现 `AccountService::reissue_invitation(evidence, source_id, request_id, budget)`。
2. 在工作单元中锁定原邀请及目标账户。
3. 校验当前 actor，要求 `IdentityOperation::InvitationIssue`。
4. 加载原邀请；允许任意状态。
5. 若目标账户存在：校验启用状态与人员类型一致；失败返回 `InvalidInput`。
6. 生成新 secret、新 id 和当前 TTL；复制用途、目标、人员类型、开发者资格；`created_by` 为当前管理员。
7. 写 `IssueInvitation` 和 `account.invitation.reissue` 审计；摘要使用 `invitation_id` 表示原记录、`target_user_id`、`expires_at`。
8. 不修改原记录。
9. 返回 `IssuedAccountAction`。

**验证：** `cargo check -p yufu`；后续测试覆盖。

## T11 增加邀请管理路由

**文件：** `yufu/src/route/enterprise_admin.rs`
**依赖：** T9、T10
**步骤：**
1. 增加 `GET /admin/invitations`，Query 包含 `after`、`limit`、`status`、`purpose`、`person_kind`、`q`。
2. 读取 SSO Cookie，调用 `list_invitations`，返回 `Json(InvitationPage)`。
3. 增加 `POST /admin/invitations/{id}/reissue`，校验写证据，调用 `reissue_invitation`。
4. 重新签发响应与签发一致：`{ id, secret, expires_at }`。
5. 保持现有签发/撤销路由不变。

**验证：** `cargo check -p yufu`；`cargo test --test mod identity_actions -- --test-threads=1` 通过。

## T12 增加邀请列表与重新签发后端测试

**文件：** `tests/yufu/routes/identity_invitations.rs`（新建）、`tests/yufu/routes/mod.rs`
**依赖：** T11
**步骤：**
1. 注册新测试模块。
2. 测试未登录和普通用户访问列表被拒绝；管理员成功。
3. 构造待用、已接受、已撤销、已过期记录，断言状态优先级。
4. 测试状态、用途、人员类型、搜索过滤。
5. 测试分页游标：每页 25，不重复不遗漏。
6. 测试撤销待用成功、撤销非待用失败、审计存在。
7. 测试任意状态重新签发：新 id/secret、原记录不变、旧待用链接仍可用。
8. 测试目标账户停用时重新签发失败且无新记录。
9. 测试同一目标连续重新签发产生多条待用记录。
10. 断言列表响应与审计不含 secret 或 digest。

**验证：** `cargo test --test mod identity_invitations -- --test-threads=1` 通过。

## T13 扩展外部绑定开始接口

**文件：** `yufu/src/service/identity/external/service.rs`、`yufu/src/route/identity_external.rs`
**依赖：** 无
**步骤：**
1. `start_bind` 增加 `flow_id: Option<Uuid>` 参数并透传。
2. 路由 `bind_start` 接受可选 JSON `{ flow_id }`，兼容无 body。
3. 前端 `startExternalBind(key, flow_id?)` 同步修改（T24）。
4. `StartExternalFlow` 在 Bind 且带 flow 时校验授权流程浏览器绑定、未完成、未绑定其他会话。

**验证：** `cargo check -p yufu`；`cargo test --test mod external_identity_github -- --test-threads=1` 现有绑定用例通过。

## T14 让绑定结果携带 OIDC 续接

**文件：** `yufu/src/service/identity/external/service.rs`
**依赖：** T13
**步骤：**
1. `ExternalCompletion::Bind` 增加 `oidc_flow_id: Option<Uuid>`。
2. `ExternalCompletion::Rejected` 增加 `oidc_flow_id: Option<Uuid>`。
3. `CompleteExternal::bind` 在创建身份前校验 flow：
   - 浏览器绑定一致；
   - 流程未完成；
   - 若已绑定中心会话，必须是当前 actor 的 root。
4. 成功与幂等绑定返回 flow id；冲突/拒绝返回带 flow id 的 Rejected。
5. 登录分支构造 Rejected 时填 `None`。

**验证：** `cargo check -p yufu`。

## T15 增加外部回调上下文读取

**文件：** `yufu/src/service/identity/external/service.rs`
**依赖：** T14
**步骤：**
1. 新增只读 `peek_flow_context(provider_key, state, browser)`。
2. 按 state 摘要加载外部流；校验 provider 与浏览器绑定。
3. 返回 `ExternalCallbackContext { purpose, oidc_flow_id }`，不消费流程。
4. 流不存在或校验失败返回 `None`/`NotFound`，供路由降级。

**验证：** `cargo check -p yufu`。

## T16 调整外部回调跳转

**文件：** `yufu/src/route/identity_external.rs`
**依赖：** T14、T15
**步骤：**
1. 成功 Bind 带 flow → `/oidc/consent?flow_id={id}`。
2. 成功 Bind 无 flow → `/account/identities?bound={provider}`。
3. Bind Rejected 带 flow → `/oidc/consent?flow_id={id}`。
4. Bind Rejected 无 flow → `/account/identities?error={code}`。
5. Login Rejected 保持 `/login?error={code}`。
6. `query.error` 分支和通用错误分支：
   - 尝试从 state 读取上下文；
   - Bind+flow → 同意页；
   - Bind 无 flow → 身份页错误；
   - Login → 登录页错误；
   - 无上下文时有中心 Cookie 去身份页，否则登录页。
7. 确保所有跳转都是硬编码路径。

**验证：** `cargo check -p yufu`；`cargo test --test mod external_identity_github -- --test-threads=1`。

## T17 增加注册同步绑定后端测试

**文件：** `tests/yufu/routes/external_identity_github.rs`
**依赖：** T5、T16
**步骤：**
1. 创建邀请并兑换 action；接受时携带 `bind_provider: "github"`。
2. 断言响应建立会话且 `requires_login=false`。
3. 使用会话调用 `bind/start`，携带原 OIDC `flow_id`。
4. 完成 fake GitHub 回调，断言身份映射创建、跳转到同意页。
5. 测试绑定取消/失败：账户和会话保留，身份映射不存在，流程仍可继续。
6. 测试禁用 Provider 后接受请求被拒绝且账户不创建。
7. 测试伪造/不匹配 flow 被拒绝。

**验证：** `cargo test --test mod external_identity_github -- --test-threads=1` 通过。

## T18 补充自动会话原子性测试

**文件：** `tests/yufu/routes/identity_actions.rs`
**依赖：** T4
**步骤：**
1. 使用数据库触发器让 `identity_sso_session` 插入或 `session.login` 审计失败。
2. 接受邀请，断言响应失败。
3. 断言账户不存在、邀请仍待用、无会话。
4. 移除触发器后重试成功。
5. 测试已有账户接受邀请不创建额外中心会话。

**验证：** `cargo test --test mod identity_actions -- --test-threads=1` 通过。

## T19 更新前端接受 API 类型

**文件：** `web/src/pages/account/api/actions.ts`、`web/src/pages/account/api/type.ts`、`web/src/pages/account/api/identities.ts`
**依赖：** T5、T13
**步骤：**
1. `acceptInvitation` 请求类型增加 `bind_provider?: string`。
2. `PasswordChangeResult` 增加可选 `bind_provider?: string`。
3. `startExternalBind` 增加可选 `flow_id` 并发送 JSON body。
4. 保持现有调用方兼容。

**验证：** `cd web && pnpm check` 通过。

## T20 注册页加载并展示 Provider

**文件：** `web/src/pages/account/views/action.vue`
**依赖：** T19
**步骤：**
1. 引入 `getExternalProviders`、`ExternalProvider` 类型、品牌图标。
2. 仅当 `data.kind === 'invitation' && !existing` 时读取 Provider。
3. 过滤 `mode === 'oauth'`。
4. 读取失败时清空并隐藏区域，不显示注册错误。
5. 在密码字段下方渲染单选卡片；默认不选；再次点击取消。
6. 添加 `aria-pressed`、键盘焦点和可读标签。

**验证：** `cd web && pnpm check`；`pnpm test -- identity-ui`。

## T21 提交注册时携带 Provider 并自动绑定

**文件：** `web/src/pages/account/views/action.vue`
**依赖：** T20
**步骤：**
1. 提交请求增加 `bind_provider`。
2. 成功后调用 `user.ensureSession()`，失败时显示错误但不伪造成功。
3. 读取响应中的 `bind_provider` 和 `oidc_flow_id`。
4. 若有 Provider：调用 `startExternalBind(provider, flow_id)` 并跳转授权地址。
5. 绑定启动失败：
   - 提示“账户已创建，但绑定未开始”；
   - 有 flow 去同意页，否则去身份页并带错误参数。
6. 无 Provider：有 flow 去同意页，否则去账户中心。
7. 移除 `requires_login` 触发 logout 的逻辑。

**验证：** `cd web && pnpm check`；`pnpm test -- identity-ui`。

## T22 更新注册页测试与 mock

**文件：** `web/tests/unit/identity-ui.test.ts`
**依赖：** T21
**步骤：**
1. 为 `@/pages/account/api/identities` 增加 mock：`getExternalProviders`、`startExternalBind`。
2. 默认 Provider catalog 返回空数组，避免未 mock 请求。
3. 增加测试：有 Provider 时显示卡片，native 不显示。
4. 增加测试：选择 Provider 后接受请求包含 `bind_provider`，并调用 `startExternalBind`。
5. 增加测试：Provider 读取失败仍可仅创建账户。
6. 更新现有接受邀请测试的 `requires_login` 预期。

**验证：** `cd web && pnpm test -- identity-ui` 通过。

## T23 身份页展示绑定结果

**文件：** `web/src/pages/account/views/identities.vue`
**依赖：** T16
**步骤：**
1. 引入 `useRoute`。
2. 加载完成后读取 `bound`、`error`、`bind` 查询参数。
3. 成功显示 toast；失败显示可读错误横幅并包含 Provider 名称。
4. 保留重试/重新绑定入口。
5. 不改变绑定/解绑确认和最后凭据逻辑。

**验证：** `cd web && pnpm check`。

## T24 更新外部绑定前端 API

**文件：** `web/src/pages/account/api/identities.ts`
**依赖：** T13
**步骤：**
1. `startExternalBind(key, flowId?)` 发送 `{ flow_id }`。
2. 现有身份页调用不传 flow，行为不变。
3. 类型保持可选，避免旧调用报错。

**验证：** `cd web && pnpm check`。

## T25 增加管理端邀请列表类型与 API

**文件：** `web/src/pages/admin/api/type.ts`、`web/src/pages/admin/api/index.ts`
**依赖：** T11
**步骤：**
1. 新增 `InvitationRecord` 与 `InvitationStatus` 类型。
2. 新增 `listInvitations(after?, signal?, filter?)`。
3. 过滤参数为 `status`、`purpose`、`person_kind`、`q`；`all` 时不传。
4. 新增 `reissueInvitation(id)`。
5. 复用现有 `IssuedAction`。

**验证：** `cd web && pnpm check`。

## T26 管理页增加邀请记录面板

**文件：** `web/src/pages/admin/views/accounts.vue`
**依赖：** T25
**步骤：**
1. 在一次性链接面板后增加“邀请记录”面板。
2. 使用 `useCursorList<InvitationRecord, string>`，每页 25。
3. 增加状态筛选 chips、用途/人员类型选择、搜索框；搜索 240ms 防抖。
4. 增加桌面表格与移动卡片，展示 plan 中定义的字段。
5. 状态使用文本 + badge，不只靠颜色。
6. 空状态区分“无记录”和“无匹配”。
7. 加载更多按钮。

**验证：** `cd web && pnpm check`。

## T27 实现撤销与重新签发交互

**文件：** `web/src/pages/admin/views/accounts.vue`
**依赖：** T26
**步骤：**
1. 待用记录显示“撤销”，所有记录显示“重新签发”。
2. 撤销使用确认弹窗；成功后刷新列表。
3. 重新签发使用确认弹窗；成功后设置一次性结果面板并刷新列表。
4. 签发邀请成功后刷新列表。
5. 请求进行中禁用对应行按钮，避免重复触发。
6. 失败显示现有 `FailureFeedback`/错误提示，不隐藏列表。

**验证：** `cd web && pnpm check`；后续测试。

## T28 更新管理端前端测试

**文件：** `web/tests/unit/admin-console.test.ts`
**依赖：** T27
**步骤：**
1. 在 mock 中增加 `listInvitations`、`reissueInvitation`。
2. 默认返回空列表。
3. 增加测试：列表显示状态/接受人；状态筛选传给服务端。
4. 增加测试：撤销待用记录调用 `revokeAction` 并刷新。
5. 增加测试：重新签发调用 `reissueInvitation`，一次性面板显示新 secret，原列表刷新。
6. 确保现有账户测试不受新增面板影响。

**验证：** `cd web && pnpm test -- admin-console` 通过。

## T29 补充邀请列表并发与权限测试

**文件：** `tests/yufu/routes/identity_invitations.rs`
**依赖：** T12
**步骤：**
1. 并发撤销与接受同一邀请：最多一方成功，另一方返回冲突/已消费。
2. 无 `AccountRead` 的管理员访问列表返回 403。
3. 无 `InvitationIssue` 重新签发返回 403。
4. 无 `InvitationRevoke` 撤销返回 403。
5. 断言拒绝的请求不改变邀请状态。

**验证：** `cargo test --test mod identity_invitations -- --test-threads=1` 通过。

## T30 更新 E2E 开户流程

**文件：** `web/tests/e2e/enterprise-sso-enterprise-onboarding.spec.ts`
**依赖：** T6、T11
**步骤：**
1. 接受邀请后断言 `requires_login=false`。
2. 直接使用浏览器上下文访问账户中心，确认无需再次登录。
3. 增加管理端邀请列表 API 验证：签发后列表包含该记录。
4. 增加重新签发验证：新 secret 可兑换，旧 secret 仍可兑换。
5. 不引入真实第三方平台依赖。

**验证：** `cd web && COBOT_SSO_PW_ARGS='-g onboarding' pnpm test:e2e` 通过。

## T31 更新运维文档

**文件：** `docs/operations/social-login.md`
**依赖：** T5、T16、T27
**步骤：**
1. 说明新建账户邀请现在自动登录。
2. 说明注册页 Provider 展示条件与绑定失败后的账户保留行为。
3. 说明邀请记录列表、状态、撤销和重新签发多链接并行。
4. 明确旧链接不可恢复，只能重新签发。
5. 明确列表不返回 secret/digest。

**验证：** 文档内容与接口行为逐条对照；无“旧链接可恢复”或“重新签发自动撤销旧链接”的表述。

## T32 全量验证

**文件：** 无
**依赖：** T1–T31
**步骤：**
1. `cargo fmt --all --check`
2. `cargo clippy -p yufu --all-targets --all-features -- -D warnings`
3. `cargo test --workspace --all-targets --all-features -- --test-threads=1`
4. `cargo test --workspace --doc --all-features`
5. `cd web && pnpm check`
6. `cd web && pnpm lint`
7. `cd web && pnpm test`
8. 记录实际通过/失败数量；失败项修复后重跑。

**验证：** 上述命令全部成功；任何未执行或环境阻塞项在验收报告中明确标注。

## 执行顺序

```text
T1 → T2 → T3 → T4 → T5 → T6
                         ↘
T7 → T8 → T9 → T10 → T11 → T12
                         ↘
T13 → T14 → T15 → T16 → T17 → T18
                         ↘
T19 → T20 → T21 → T22 → T23 → T24
                         ↘
T25 → T26 → T27 → T28 → T29
                         ↘
T30 → T31 → T32
```
