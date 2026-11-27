# 外部身份社交登录（P3–P6）Tasks

> 状态：已完成（T1–T21，2026-11-27）

## 文件清单

| 操作 | 文件 | 职责 |
|---|---|---|
| 新建 | `yufu/src/service/identity/external/wechat_qr.rs` | 微信开放平台扫码适配器与 fake 测试 |
| 新建 | `yufu/src/service/identity/external/qq.rs` | QQ OAuth 适配器与 fake 测试 |
| 修改 | `yufu/src/service/identity/external/mod.rs` | 导出新适配器 |
| 修改 | `yufu/src/lib.rs` | registry 加载三个适配器；管理端热刷新 |
| 修改 | `yufu/src/service/identity/external/service.rs` | 上游失败审计；复用登录/绑定 |
| 新建 | `yufu/src/service/identity/external/admin.rs` | 管理端配置与统计服务 |
| 修改 | `yufu/src/domain/identity/external.rs` | 管理端视图/更新/统计类型 |
| 修改 | `yufu/src/service/identity/repository_external.rs` | Provider 管理读写、审计统计查询 |
| 修改 | `yufu/src/service/identity/repository.rs` | `IdentityTransaction` 扩展方法 |
| 修改 | `yufu/src/service/identity/repository_pg.rs` | 新 trait 方法转发 |
| 新建 | `yufu/src/route/external_admin.rs` | 管理端 Provider 路由 |
| 修改 | `yufu/src/route/mod.rs` | 注册管理端路由模块 |
| 修改 | `yufu/src/route/identity_browser.rs` | 挂载管理端路由 |
| 新建 | `web/src/pages/admin/api/external-providers.ts` | 管理端 API 客户端 |
| 新建 | `web/src/pages/admin/views/external-providers.vue` | 管理端 Provider 配置/统计页 |
| 修改 | `web/src/router/routes.ts` | 新增 `/admin/external-providers` |
| 修改 | `web/src/router/navigation.ts` | 新增导航项 |
| 新建 | `tests/yufu/routes/external_identity_wechat.rs` | 微信 fake 端到端测试 |
| 新建 | `tests/yufu/routes/external_identity_qq.rs` | QQ fake 端到端测试 |
| 新建 | `tests/yufu/routes/external_identity_admin.rs` | 管理端配置/Secret/统计测试 |
| 修改 | `tests/yufu/routes/mod.rs` | 注册新测试模块 |
| 新建 | `docs/operations/social-login.md` | 申请、配置、主密钥、升级说明 |
| 修改 | `docs/specs/external-identity-federation/spec.md` | 状态更新（完成后） |

## T1 微信适配器：begin

**文件：** `yufu/src/service/identity/external/wechat_qr.rs`
**依赖：** 无
**步骤：**
1. 定义 `WechatQrAdapter`，实现 `ExternalIdentityAdapter`，`key=WechatQr`。
2. `capabilities` 设为 `pkce=false, nonce=false, native=false, union_id=true`。
3. `begin` 读取 `authorize_endpoint`/`scope` 覆盖，默认 `https://open.weixin.qq.com/connect/qrconnect`、`snsapi_login`。
4. 拼接 `appid/redirect_uri/response_type=code/scope/state`，末尾保留 `#wechat_redirect`。
5. `secret_payload` 返回 `None`。

**验证：** 单元测试解析 URL，确认 `appid`、`redirect_uri`、`state`、`scope` 和 `#wechat_redirect`。

## T2 微信适配器：complete

**文件：** `yufu/src/service/identity/external/wechat_qr.rs`
**依赖：** T1
**步骤：**
1. GET token endpoint，参数 `appid/secret/code/grant_type=authorization_code`。
2. 解析 `access_token/openid/unionid/errcode/errmsg`；`errcode != 0` 返回 `Unauthenticated` 或 `DependencyUnavailable`。
3. GET userinfo，带 `access_token/openid/lang`；解析 `nickname/headimgurl/unionid`。
4. 头像 `http://` 归一化为 `https://`，其他非 https 丢弃。
5. 构造 `ExternalIdentity`，subject=`openid`，`union_id` 保留，`email=None`，调用 `validate()`。

**验证：** fake 微信 token/userinfo 返回后，得到 `provider=WechatQr`、subject、昵称、https 头像；errcode 非 0 时失败。

## T3 微信 fake 单测

**文件：** `yufu/src/service/identity/external/wechat_qr.rs`
**依赖：** T2
**步骤：**
1. 用 Axum 启动 loopback fake 微信服务。
2. 测试 begin URL、complete 成功、errcode 失败、头像协议归一化。
3. 使用 `ExternalHttpClient::new(..., true)`。

**验证：** `cargo test -p yufu --lib wechat_qr` 通过。

## T4 QQ 适配器：begin

**文件：** `yufu/src/service/identity/external/qq.rs`
**依赖：** 无
**步骤：**
1. 定义 `QqAdapter`，`key=Qq`，能力 `pkce=false, nonce=false, native=false, union_id=true`。
2. `begin` 默认 `https://graph.qq.com/oauth2.0/authorize`，参数 `response_type=code/client_id/redirect_uri/state/scope=get_user_info`。
3. 支持 `authorize_endpoint/scope` 覆盖。

**验证：** 单测解析 URL，确认 client_id、redirect_uri、state、scope。

## T5 QQ 适配器：complete

**文件：** `yufu/src/service/identity/external/qq.rs`
**依赖：** T4
**步骤：**
1. token 请求 `fmt=json`；读取文本，兼容纯 JSON 与 `callback(...)` JSONP。
2. 解析 `access_token/error/error_description`；错误直接失败。
3. 请求 `me`，解析 `openid/client_id`；JSONP 同样兼容。
4. 请求 `get_user_info`，参数 `access_token/oauth_consumer_key/APPID/openid`；校验 `ret == 0`。
5. 取 `nickname`、`figureurl_qq_2` 或 `figureurl_qq_1`，归一化为 https；构造身份并校验。

**验证：** fake QQ 返回 JSON/JSONP 两种格式均成功；缺少 openid 或 ret!=0 时失败。

## T6 QQ fake 单测

**文件：** `yufu/src/service/identity/external/qq.rs`
**依赖：** T5
**步骤：**
1. Axum fake 服务分别返回 JSON 与 JSONP。
2. 测试 begin、complete、token 错误、userinfo ret 错误。
3. loopback HTTP 客户端。

**验证：** `cargo test -p yufu --lib qq` 通过。

## T7 注册表加载三个适配器

**文件：** `yufu/src/lib.rs`、`yufu/src/service/identity/external/mod.rs`
**依赖：** T1–T6
**步骤：**
1. 导出 `WechatQrAdapter`、`QqAdapter`。
2. `reload_external_registry` 的 adapters 列表加入 QQ、微信扫码。
3. 保持启用但缺适配器/Secret 时 fail-closed。

**验证：** 启动测试 fixture 不报错；公共 catalog 默认仍为空。

## T8 上游失败审计

**文件：** `yufu/src/service/identity/external/service.rs`
**依赖：** T2、T5
**步骤：**
1. 新增 best-effort `record_failure(provider, reason, request_id)`。
2. 在 `complete` 的 state 校验失败、上游调用失败路径写 `external.login` failure 审计；使用新预算，失败只记日志不覆盖主错误。
3. 审计摘要仅含 `provider`，不写 code/state/token。

**验证：** 单测/集成测试触发上游失败后，`identity_audit_event` 有 `external.login` failure 记录。

## T9 管理端数据类型

**文件：** `yufu/src/domain/identity/external.rs`
**依赖：** 无
**步骤：**
1. 定义 `ExternalProviderAdminView`、`UpdateExternalProvider`、`ExternalProviderStats`、`ProviderStat`。
2. 所有类型实现必要的 Serialize/Deserialize。
3. 校验 revision、client_id 长度、enabled 完整性。

**验证：** `cargo check -p yufu` 通过；单测覆盖类型校验。

## T10 管理端 Repository 读取

**文件：** `yufu/src/service/identity/repository_external.rs`、`repository.rs`、`repository_pg.rs`
**依赖：** T9
**步骤：**
1. Trait 增加 `external_provider_admin_rows()`。
2. SQL 查询 `identity_external_provider` 全量行，返回非敏感字段。
3. 映射为 `ExternalProviderAdminView`，`secret_configured` 由密文非空推导。

**验证：** 集成测试查询默认 5 行；Secret 字段不存在于返回结构。

## T11 管理端 Repository 写入/轮换

**文件：** 同上
**依赖：** T10
**步骤：**
1. Trait 增加 `update_external_provider(...)` 与 `rotate_external_provider_secret(...)`。
2. 更新带 `revision` 条件；0 行返回 `Conflict`。
3. 只更新 `client_id/enabled/config/secret_ciphertext/revision/updated_at/updated_by`。
4. 禁止把 Secret 写进审计摘要。

**验证：** 集成测试覆盖 revision 冲突、Secret 保留/替换、启用缺配置拒绝。

## T12 管理端统计查询

**文件：** 同上
**依赖：** T8
**步骤：**
1. Trait 增加 `external_login_stats(from, to)`。
2. SQL：`action='external.login'`，按 `change_summary->>'provider'`、`result`、`reason_code` 聚合。
3. 返回 `ExternalProviderStats`；无数据时返回空列表。
4. 时间窗口由服务端限定为 24h/7d。

**验证：** 构造 success/denied/failure 审计后，统计计数与分类正确。

## T13 管理端服务

**文件：** `yufu/src/service/identity/external/admin.rs`
**依赖：** T10–T12
**步骤：**
1. `ExternalProviderAdminService` 持 repository、clock、cipher。
2. `list`：读取当前 actor + `ExternalProviderRead` 权限。
3. `update`：锁 Provider 行/用户，校验 revision、启用完整性，加密 Secret，写审计。
4. `rotate_secret`：校验 revision，重新加密，写 `external_provider.secret` 审计。
5. `stats`：读取当前 actor + 权限，限制窗口，查询聚合。
6. 所有写操作使用 `authenticate_current_actor`，读操作使用 `read_current_actor`。

**验证：** 单测/集成测试覆盖权限拒绝、revision 冲突、Secret 不回显、审计。

## T14 管理端路由

**文件：** `yufu/src/route/external_admin.rs`、`route/mod.rs`、`route/identity_browser.rs`
**依赖：** T13
**步骤：**
1. 注册 4 个管理端路由。
2. 读接口读取 SSO Cookie；写接口先 `verify_write`。
3. 写成功后调用 `state.reload_external_registry()`；刷新失败返回 503 但保留已提交配置。
4. 错误返回 `request_id`。

**验证：** HTTP 集成测试：未登录 401、无权限 403、成功 200、revision 冲突 409、Secret 不回显。

## T15 前端管理端 API

**文件：** `web/src/pages/admin/api/external-providers.ts`
**依赖：** T14
**步骤：**
1. 定义 Provider 视图、更新请求、统计类型。
2. 实现 list/update/rotate/stats 方法，使用 `identityRequest`。
3. Secret 只作为请求字段，不进入本地缓存。

**验证：** `pnpm --dir web check` 通过。

## T16 前端管理端页面

**文件：** `web/src/pages/admin/views/external-providers.vue`、`routes.ts`、`navigation.ts`
**依赖：** T15
**步骤：**
1. 新增 `/admin/external-providers`，权限 `administrator`。
2. 每行展示 Provider、Client ID/AppID、Secret 已配置、启用开关、revision、更新时间。
3. Secret 输入只写；空表示保留；轮换操作二次确认。
4. 保存带 `expected_revision`；冲突提示刷新。
5. 统计区展示 24h/7d 成功/拒绝/失败和主要分类。
6. 导航加入“第三方登录”。

**验证：** `pnpm --dir web check/lint/test/build` 通过；手工页面可加载。

## T17 微信 fake 端到端测试

**文件：** `tests/yufu/routes/external_identity_wechat.rs`、`routes/mod.rs`
**依赖：** T7、T3
**步骤：**
1. 复用 GitHub 测试结构，fake 微信 token/userinfo。
2. 测试 start → callback 成功、未绑定拒绝、绑定/解绑。
3. 断言 `identity_external_identity.provider_key='wechat_qr'` 和审计。

**验证：** `cargo test --test mod external_identity_wechat -- --test-threads=1` 通过。

## T18 QQ fake 端到端测试

**文件：** `tests/yufu/routes/external_identity_qq.rs`、`routes/mod.rs`
**依赖：** T7、T6
**步骤：**
1. fake QQ 返回 JSONP 格式。
2. 测试 start → callback 成功、token/userinfo 错误失败、绑定/解绑。
3. 断言 provider_key=`qq`。

**验证：** `cargo test --test mod external_identity_qq -- --test-threads=1` 通过。

## T19 管理端集成测试

**文件：** `tests/yufu/routes/external_identity_admin.rs`、`routes/mod.rs`
**依赖：** T14
**步骤：**
1. 登录管理员，读取 Provider 列表。
2. 更新 client_id/启用，断言登录页 catalog 变化。
3. 写入 Secret，断言响应/日志不含明文，`secret_configured=true`。
4. revision 冲突返回 409。
5. 构造审计后查询 24h 统计。
6. 禁用 Provider 后 start 失败关闭。

**验证：** `cargo test --test mod external_identity_admin -- --test-threads=1` 通过。

## T20 文档

**文件：** `docs/operations/social-login.md`
**依赖：** T1–T19
**步骤：**
1. 三个 Provider 的申请条件、回调地址、配置项。
2. 主密钥生成/持久化/轮换/丢失处理。
3. 未绑定策略、自动建号、解绑限制。
4. 升级/回滚、默认关闭、资质门禁。
5. 明确不包含小程序/公众号/handoff/resume。

**验证：** 文档中的命令和 URL 与实际代码一致；无 Secret 示例。

## T21 全量验证

**依赖：** T1–T20
**步骤：**
1. `cargo fmt --check`。
2. `cargo clippy --workspace --all-targets -- -D warnings`。
3. `cargo test -p yufu --lib`。
4. 串行运行外部身份相关集成测试。
5. `pnpm --dir web check && pnpm --dir web test && pnpm --dir web lint && pnpm --dir web build`。
6. 更新 spec 状态与交付说明。

**验证：** 全部命令通过；记录实际输出。

## 执行顺序

```text
T1 → T2 → T3 ─┐
T4 → T5 → T6 ─┼→ T7 → T8 ─┐
              │            ├→ T13 → T14 → T15 → T16
T9 → T10 → T11 → T12 ──────┘
T17/T18/T19 依赖 T7/T14
T20 依赖实现完成
T21 最后
```
