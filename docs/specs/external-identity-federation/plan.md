# 外部身份社交登录（P3–P6）Plan

> 输入：已确认的 [spec.md](./spec.md)
> 状态：已批准并执行（2026-11-27）
> 基线：P1/P2 已实现；本 plan 只描述 P3–P6 的增量设计
> 范围：微信开放平台扫码、QQ OAuth、管理端配置/统计、安全验收与文档

## 架构概览

现有外部身份域已经具备统一入口：`ExternalIdentityAdapter`、`ExternalIdentityService`、`identity_external_*` 表、`CreateCenter + BindFlow` 会话复用、SecretCipher、限流和审计。P3–P6 不重写这套结构，只增加两类 Provider 适配器、一个管理端配置面和一个统计视图。

- **微信扫码适配器**：实现开放平台网站应用 `qrconnect` / `snsapi_login`。`begin` 返回微信官方扫码页 URL；`complete` 用 `code` 换 `access_token`，再取 `openid/unionid/nickname/headimgurl`。不支持 PKCE；依赖 state、固定回调和 AppSecret。
- **QQ 适配器**：实现 QQ 互联 OAuth 2.0。`begin` 返回 `graph.qq.com/oauth2.0/authorize`；`complete` 处理 token、`me`、`get_user_info`，兼容 JSON 与 JSONP 包装。以 `openid` 为稳定标识。
- **管理端配置服务**：读写 `identity_external_provider`；Secret 只写不回显；启用/禁用；revision 冲突；写审计；写成功后热刷新 Provider 注册表。
- **统计服务**：从 `identity_audit_event` 聚合 `external.login` 的成功/拒绝/失败，按 Provider 与时间窗口分组；补齐上游失败审计。
- **前端**：登录页已由公共 catalog 驱动，新增 Provider 自动出现；新增管理端 Provider 配置页与统计卡片；账户中心第三方身份页已支持所有 Provider，无需结构改动。
- **测试**：为微信/QQ 增加 fake Provider 端到端测试；为管理端增加配置、Secret 脱敏、启停、revision、统计测试。

## 核心数据结构

### ExternalProviderKey（已有）

现有枚举包含 `GitHub/Qq/WechatMini/WechatWeb/WechatQr`。本阶段只启用：

- `Qq`：QQ 互联
- `WechatQr`：微信开放平台网站应用扫码

`WechatMini/WechatWeb` 保持禁用/未实现，避免范围蔓延。

### ExternalProviderConfig（已有）

```text
ExternalProviderConfig {
  key: ExternalProviderKey
  client_id: String
  secret: Arc<SecretToken>        // 只在内存中使用
  config: serde_json::Value       // 非敏感：endpoint 覆盖、scope、lang
}
```

新增 Provider 的 `config` 约定：

- 微信：`authorize_endpoint`、`token_endpoint`、`userinfo_endpoint`、`scope`（默认 `snsapi_login`）、`lang`（默认 `zh_CN`）
- QQ：`authorize_endpoint`、`token_endpoint`、`me_endpoint`、`user_endpoint`、`scope`（默认 `get_user_info`）

### 管理端视图

```text
ExternalProviderAdminView {
  provider_key: String
  family: String
  enabled: bool
  client_id: String
  secret_configured: bool
  revision: u64
  updated_at: u64
  updated_by: Option<i32>
  config_complete: bool
}

UpdateExternalProvider {
  provider_key: ExternalProviderKey
  expected_revision: u64
  enabled: bool
  client_id: String
  secret: Option<String>          // 只写；None 表示保持原 Secret
}

ExternalProviderStats {
  from: u64
  to: u64
  providers: Vec<ProviderStat>
}
ProviderStat {
  provider_key: String
  success: u64
  denied: u64
  failure: u64
  reasons: Vec<(String, u64)>
}
```

### 审计事件

复用 `identity_audit_event`：

- `external.login`：成功/拒绝/失败
- `external.login.start`
- `external.bind` / `external.unbind`
- `external_provider.update` / `external_provider.secret`

`change_summary` 只允许白名单键：`provider`、`idempotent`、`existing`、`revision`、`enabled`、`changed_fields`、`user_id`、`affected_count`。Secret、code、token、state 永不进入摘要。

## 模块设计

### 微信扫码适配器

**职责：** 构建官方扫码 URL；用 code 换取微信身份；把微信返回归一化为 `ExternalIdentity`。

**对外接口：** 实现现有 `ExternalIdentityAdapter`：

- `capabilities`: `pkce=false, nonce=false, native=false, union_id=true`
- `begin`: 生成 `qrconnect` URL，参数 `appid/redirect_uri/response_type=code/scope=snsapi_login/state`，末尾 `#wechat_redirect`
- `complete`: 先换 token，再取 userinfo；`errcode != 0` 必须失败；`openid` 作为 subject；`unionid` 可选；头像 `http://` 归一化为 `https://` 或丢弃

**依赖：** `ExternalHttpClient`、`ExternalProviderConfig`、`ExternalIdentity` 校验。

### QQ 适配器

**职责：** 构建 QQ 授权 URL；换取 token；解析 `me` 和 `get_user_info`；归一化身份。

**对外接口：**

- `capabilities`: `pkce=false, nonce=false, native=false, union_id=true`
- `begin`: `graph.qq.com/oauth2.0/authorize`，参数 `response_type=code/client_id/redirect_uri/state/scope=get_user_info`
- `complete`: token → me → userinfo；兼容 JSON 与 `callback(...)` JSONP；`ret != 0` 或缺少 `openid` 必须失败

**依赖：** 同微信；额外需要 JSONP 提取工具（仅服务端）。

### Provider 注册表

**职责：** 启动和管理端变更后加载启用且完整的 Provider；只把可用 Provider 暴露到公共 catalog。

**变更：** `AppState::reload_external_registry` 的 adapter 列表从仅 GitHub 扩展为 GitHub、QQ、微信扫码；管理端写成功后调用同一方法热刷新。任一已启用 Provider 缺少适配器/Secret 时启动或刷新失败（fail closed）。

### 管理端配置服务

**职责：** 认证管理员、校验权限、读写 Provider 配置、处理 revision、加密 Secret、写审计、触发热刷新。

**对外接口：**

```text
list(secret, budget) -> Vec<ExternalProviderAdminView>
update(secret, UpdateExternalProvider, request_id, budget) -> ExternalProviderAdminView
rotate_secret(secret, provider_key, secret, expected_revision, request_id, budget)
stats(secret, from, to, budget) -> ExternalProviderStats
```

**依赖：** `IdentityRepository` 事务、`SecretCipher`、`IdentityOperation::ExternalProviderRead/Configure`、`AppState` 热刷新。

**规则：**

- 读取不返回 Secret；`secret_configured` 由密文是否为空推导。
- 更新时 `secret=None` 保留原密文；`secret=Some` 重新加密。
- 启用时校验 `client_id` 非空、密文可解密、适配器存在。
- revision 不匹配返回 `Conflict`。
- 写审计 `external_provider.update/secret`，摘要仅含 provider、enabled、changed_fields、revision。
- 写成功后热刷新；刷新失败返回错误，但数据库已提交（fail closed：登录页可能暂时不显示该 Provider）。

### 统计服务

**职责：** 从审计聚合登录结果。

**查询：**

- `action = 'external.login'`，`happened_at BETWEEN from AND to`
- Provider 从 `change_summary->>'provider'` 取
- `result` 分 success/denied/failure
- `reason_code` 分组为失败分类

**变更：** `ExternalIdentityService` 在上游调用失败、state 无效等“尚未进入完成事务”的路径写 `external.login` failure 审计；已在完成事务内的拒绝继续写 denied。

### 路由

```text
GET  /api/v1/admin/external-providers
PUT  /api/v1/admin/external-providers/{provider}
POST /api/v1/admin/external-providers/{provider}/secret
GET  /api/v1/admin/external-providers/stats?window=24h|7d
```

- 全部走 SSO Cookie；写操作额外 `verify_write`（CSRF + Origin）。
- 管理端读/写分别要求 `ExternalProviderRead/Configure`。
- 写成功后调用 `AppState::reload_external_registry`。

现有公开路由不变：

```text
GET  /api/v1/identity/external/providers
POST /api/v1/identity/external/{provider}/start
POST /api/v1/identity/external/{provider}/bind/start
GET  /identity/external/{provider}/callback
GET  /api/v1/account/identities
POST /api/v1/account/identities/{id}/unbind
```

### 前端

- 新增 `/admin/external-providers` 页面与导航项。
- 每行展示 Provider、Client ID/AppID、Secret 已配置状态、启用开关、revision、更新时间。
- Secret 输入为只写；空表示保持不变；提供“轮换 Secret”操作。
- 保存带 `expected_revision`，冲突提示刷新。
- 统计区展示 24h/7d 成功/拒绝/失败数和主要失败分类。
- 登录页和账户中心不需要结构性改动；catalog 增加 Provider 后自动出现。

## 模块交互

### 微信/QQ 登录

```text
浏览器 → 登录页点击 Provider
→ POST /identity/external/{key}/start（CSRF+Origin）
→ 服务端创建 external_flow(state, browser binding)
→ 303 Provider 官方授权页
→ 用户扫码/授权 → GET /identity/external/{key}/callback?code&state
→ 服务端校验 state/浏览器绑定/有效期
→ 服务端换 token + 取身份
→ 事务：消费 flow、查/建绑定、CreateCenter、BindFlow
→ Set-Cookie + 303 /oidc/consent 或 /account
```

### 管理端配置

```text
管理员 → PUT /admin/external-providers/{key}（CSRF+Origin）
→ 读取当前 actor + 权限
→ 校验 revision/配置完整性
→ 加密 Secret（如有）
→ 更新行 + 审计
→ 热刷新 registry
→ 返回脱敏视图
```

### 统计

```text
管理员 → GET /admin/external-providers/stats?window=24h
→ 权限校验
→ 审计表按 provider/result/reason 聚合
→ 返回计数（无 Secret/身份明文）
```

## 文件组织

```text
yufu/src/
├── domain/identity/external.rs                  # 现有类型；增加管理端视图/统计类型
├── service/identity/external/
│   ├── adapter.rs                               # 现有 trait
│   ├── github.rs                                # 已有
│   ├── wechat_qr.rs                             # 新增：微信扫码
│   ├── qq.rs                                    # 新增：QQ
│   ├── registry.rs                              # 增加两个 adapter 加载
│   ├── service.rs                               # 增加失败审计；复用登录/绑定
│   └── admin.rs                                 # 新增：管理端配置/统计
├── service/identity/repository_external.rs      # 增加管理端读写/统计查询
├── service/identity/repository.rs               # trait 扩展
├── route/identity_external.rs                   # 现有公开路由
├── route/external_admin.rs                      # 新增管理端路由
├── lib.rs                                       # registry 热刷新可见性
└── ...
web/src/
├── pages/admin/views/external-providers.vue     # 新增
├── pages/admin/api/external-providers.ts        # 新增
├── router/routes.ts                             # 新增路由
└── router/navigation.ts                         # 新增导航
docs/
├── operations/social-login.md                   # 新增：申请/配置/主密钥/升级
└── specs/external-identity-federation/          # 本套 spec
```

## 技术决策

| 决策点 | 选择 | 理由 |
|---|---|---|
| 微信 Web 形态 | 开放平台扫码，跳转官方扫码页 | 不引入微信 JS/CSP 风险；符合“桌面 Web 为主”；个人主体不可用的问题在资质门禁中说明 |
| 微信/QQ PKCE | 不启用 | 平台不支持；依赖 state、固定回调、服务端 Secret、一次性 code |
| 小程序能力 | 不做 | 用户明确 Yufu 不开发小程序；下游小程序通过标准 OIDC 接入 |
| 未绑定策略 | `disabled` 默认，`open` 可选 | 安全默认；保留消费级自动建号能力 |
| Secret 存储 | 复用 AEAD + 环境主密钥 | 已有 P1 能力；管理端只写不回显 |
| 管理端热生效 | 写成功后 reload registry | 避免重启；失败关闭 |
| 统计来源 | 现有审计表 | 不新增敏感采集；与登录审计口径一致 |
| 失败审计 | 新增 best-effort failure 事件 | 统计能区分上游失败；不阻塞主流程 |
| 同 Provider 多身份 | 保持一身份 | 与 P2 一致，降低 UI/解绑复杂度 |
| 发布 | 不打 tag | 用户选择 P6 只交付代码/测试/文档 |

## Spec 覆盖映射

| 需求 | 实现位置 |
|---|---|
| F1 微信扫码 | `wechat_qr.rs`、现有 start/callback、registry |
| F2 微信绑定/解绑 | 现有 `ExternalIdentityService` + 账户中心页 |
| F3 QQ | `qq.rs`、现有 start/callback、registry |
| F4 未绑定策略 | 现有 `RegistrationPolicy` + `service.rs` |
| F5 管理端配置 | `admin.rs`、`external_admin.rs`、`external-providers.vue` |
| F6 统计 | `admin.rs` stats + 失败审计 |
| F7 可用性隔离 | `registry.rs`、provider 级限流/超时、错误映射 |
