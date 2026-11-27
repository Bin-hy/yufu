# 外部社交登录运维与接入说明

> 适用版本：外部身份 P3–P6
> 范围：GitHub、QQ、微信开放平台扫码登录
> 不包含：Yufu 官方微信小程序、`wx.login`、`/mini/*`、handoff/resume、公众号网页 OAuth、本机微信登录检测

## 1. 能力概览

Yufu 仍是唯一 OIDC Provider。GitHub/QQ/微信只用于认证 Yufu 用户；认证成功后仍走原有 `identity_sso_session` 与 OIDC 授权流程。下游应用不需要接入任何社交平台，也不感知用户使用了哪种登录方式。

三种登录方式都遵循：

- 一次性 state，绑定发起登录的浏览器；
- 固定回调地址；
- 服务端换取身份，Secret 不出服务端；
- 未绑定默认拒绝；`registration=open` 时可创建无密码外部账号；
- 同一 Provider 同一外部身份只能绑定一个 Yufu 用户；
- 最后一个可用登录凭据不可解绑；
- 登录、绑定、解绑、配置变更全部写审计。

## 2. 回调地址

将 `<issuer>` 替换为部署的公开地址。本地开发默认 issuer 为 `http://127.0.0.1:4000`。

| Provider | 回调地址 |
|---|---|
| GitHub | `https://<issuer>/identity/external/github/callback` |
| QQ | `https://<issuer>/identity/external/qq/callback` |
| 微信扫码 | `https://<issuer>/identity/external/wechat_qr/callback` |

本地示例：

```text
http://127.0.0.1:4000/identity/external/github/callback
http://127.0.0.1:4000/identity/external/qq/callback
http://127.0.0.1:4000/identity/external/wechat_qr/callback
```

回调地址必须与平台后台登记值完全一致（协议、域名、端口、路径）。

## 3. 平台申请条件

### GitHub

- 任意 GitHub 账号可创建 OAuth App。
- 需要 Homepage URL 与 Authorization callback URL。
- Scope：`read:user user:email`。
- 稳定身份：`/user` 返回的数值 `id`；不使用邮箱或登录名作为身份键。
- 本地 `http://127.0.0.1:4000` 可用于开发；生产使用 HTTPS。

### QQ

- QQ 互联开发者注册（公司/个人，需实名）。
- 创建“网站应用”，网站地址与回调地址需 ICP 备案且与平台填写一致。
- 审核通常 1–3 个工作日。
- 稳定身份：`openid`；`unionid` 仅在平台返回时保存，不用于自动合并。
- 需按腾讯规范放置 QQ 登录按钮。

### 微信开放平台扫码

- 仅支持**微信开放平台网站应用**，使用 `snsapi_login` 扫码登录。
- 个人主体不可申请；需要企业/组织主体与开放平台开发者认证。
- 需要配置授权回调域名。
- 浏览器无法检测本机微信是否已登录；用户必须用手机微信扫码。
- 微信内置浏览器/公众号网页 OAuth 不在本阶段范围。

## 4. 配置方式

### 4.1 主密钥（必需）

Provider Secret 使用 AES-256-GCM 加密存储。主密钥只从环境变量读取：

```bash
mkdir -p ~/.config/yufu
openssl rand -base64 32 | tr -d '\n' > ~/.config/yufu/external.key
chmod 600 ~/.config/yufu/external.key
export YUFU_EXTERNAL_SECRET_KEY="$(cat ~/.config/yufu/external.key)"
```

注意：

- 主密钥必须持久化并复用；丢失后已保存的 Provider Secret 无法解密，服务会拒绝启动。
- 主密钥不得写入数据库、镜像、仓库或日志。
- 更换主密钥需要重新录入所有 Provider Secret。

### 4.2 管理控制台

管理员进入 **企业管理 → 第三方登录**：

- 填写 GitHub Client ID/Secret、QQ AppID/AppKey、微信 AppID/AppSecret；
- 启用/禁用 Provider；
- Secret 只写不回显；留空表示保持原值；
- 使用“轮换 Secret”单独更新 Secret；
- 保存带 revision；冲突时刷新后重试；
- 启用要求 Client ID/AppID 与 Secret 完整；
- 修改后无需重启，登录页立即生效。

权限：`ExternalProviderRead` / `ExternalProviderConfigure`。默认超级管理员拥有；其他角色需显式授权。

### 4.3 环境变量引导（可选）

首次启动时可用环境变量引导 GitHub 配置；仅在对应列为空时写入，不覆盖管理端已保存的配置：

```bash
export YUFU_EXTERNAL_GITHUB_CLIENT_ID='<client id>'
read -s YUFU_EXTERNAL_GITHUB_CLIENT_SECRET
export YUFU_EXTERNAL_GITHUB_CLIENT_SECRET
export YUFU_EXTERNAL_GITHUB_ENABLED=true
```

QQ/微信请通过管理控制台配置。

### 4.4 未绑定策略

```toml
[external_identity]
enabled = true
registration = "disabled"   # disabled | open
```

- `disabled`（默认）：未绑定身份不能登录，也不创建账号；提示先使用 Yufu 账号登录并绑定。
- `open`：未绑定身份首次登录自动创建无密码外部账号；该账号不能密码登录，不授予管理员/开发者权限。
- `invite_only` 为后续占位，当前按 `disabled` 处理。
- 禁止按邮箱/昵称自动合并账号。

## 5. 登录与绑定流程

1. 用户访问下游应用，应用发起标准 OIDC Authorization Code + PKCE。
2. Yufu 登录页展示已启用的 GitHub/QQ/微信入口。
3. 用户选择 Provider，Yufu 创建一次性外部登录事务并跳转平台。
4. 平台回调后，Yufu 校验 state、浏览器绑定和有效期，服务端换取外部身份。
5. 已绑定：建立正常 Yufu 会话，继续原 OIDC 流程。
6. 未绑定：按 `registration` 策略拒绝或创建无密码账号。
7. 下游应用最终获得原有 ID Token/Access Token/Refresh Token。

绑定：

1. 用户先用 Yufu 账号登录。
2. 进入 **账户中心 → 第三方身份**。
3. 选择 Provider 并完成授权。
4. 同一外部身份已属于其他用户时拒绝；同一用户重复绑定按幂等成功。

解绑：

- 解绑不影响密码或其他 Provider，也不强制撤销现有会话。
- 若用户没有密码且没有其他外部身份，解绑被拒绝。

## 6. 审计与统计

审计动作：

- `external.login`（success/denied/failure）
- `external.login.start`
- `external.bind` / `external.unbind`
- `external_provider.update` / `external_provider.secret`

管理端 **第三方登录 → 登录统计** 支持最近 24 小时/7 天，按 Provider 展示成功、拒绝、失败及主要失败分类。统计不包含 Secret、授权码、Token 或完整外部标识。

## 7. 安全约束

- state 一次性、绑定浏览器、过期拒绝。
- GitHub 使用 PKCE S256；QQ/微信不支持 PKCE，依赖固定回调、服务端 Secret、一次性 code 和 state。
- 回调地址由服务端固定，不接受请求传入。
- Secret 加密存储；API/日志/审计不返回明文。
- 外部 HTTP 不占用核心写事务；有超时和并发上限。
- 未启用 Provider 失败关闭；单 Provider 故障不影响密码登录和其他 Provider。
- 主密钥缺失且存在密文时服务拒绝启动。

## 8. 升级与回滚

- 新迁移为增量式：新增外部身份表与默认禁用的 Provider，不修改 OIDC/用户/会话既有表。
- 升级后未配置 Provider 时，登录行为不变。
- 建议升级前备份数据库与主密钥。
- 回滚代码不会删除既有用户、会话、审计或 OIDC 数据；新增表可保留。
- 生产升级前先在数据库快照上演练迁移。

## 9. 测试与资质状态

自动化测试使用 fake Provider，不访问真实平台：

```bash
cargo test --test mod external_identity -- --test-threads=1
```

覆盖 GitHub/微信/QQ 登录、未绑定拒绝、绑定/解绑、Secret 脱敏、revision、统计、上游失败。

真实联调状态：

- GitHub：可用真实 OAuth App 联调。
- QQ：需要 QQ 互联应用与 ICP 备案域名；审核通过前保持禁用。
- 微信扫码：需要微信开放平台网站应用与企业主体认证；资质就绪前保持禁用。

资质未就绪不影响代码与 fake 验收，但上线前必须完成平台审核并把回调地址登记为实际 `issuer`。

## 10. 明确不包含

- Yufu 官方微信小程序、`wx.login`、`code2Session` 服务端接口、`/mini/*`
- 小程序 web-view 与 OIDC 的 handoff/resume
- 公众号网页 OAuth / 微信内置浏览器授权
- 检测本机微信客户端是否登录
- 按邮箱/昵称自动合并账号
- 同一 Provider 多身份
- 管理员代绑/强制解绑外部身份
- 版本 tag / GitHub Release

## 11. 官方平台核对记录（2026-11-27）

| 平台 | 官方结论 | Yufu 实现 |
|---|---|---|
| GitHub | OAuth 2.0，不是 OIDC；支持 PKCE S256；支持 `prompt=select_account`；REST API 需要 User-Agent；回调可能带 RFC 9207 `iss` | 使用 PKCE；可选 `prompt=select_account`；设置 User-Agent；兼容 `iss`；10 秒出网超时；HTTP/1.1 |
| 微信开放平台 | 网站应用使用 `qrconnect` + `snsapi_login`；微信 3.9.11 Windows / 4.0 Mac 起支持“网站应用快速登录”，已登录微信会优先提示当前客户端；`fast_login` 仅用于内嵌 JS | 跳转官方 `qrconnect` 页面；不伪造客户端检测；不在 URL 中添加未文档化参数 |
| QQ 互联 | token 为 GET；`me` 可加 `unionid=1`；`get_user_info` 使用 `access_token/oauth_consumer_key/openid`；授权可带 `display=pc` | 使用 GET token；`me` 请求 unionid；授权默认 `display=pc`；兼容 JSON/JSONP |

GitHub 已授权过的 OAuth App 不会重复显示 scope 同意页；这是 GitHub 的授权记忆行为。需要重新看到授权页时，应在 GitHub 的 Authorized OAuth Apps 中撤销授权。`prompt=select_account` 只强制账号选择器，不强制重新同意 scope。

GitHub 的 `iss` 参数、User-Agent 要求、PKCE 支持和授权记忆行为均已加入本地回归测试或运行日志。微信“快速登录”由微信客户端在官方扫码页侧触发，Yufu 无法主动检测本机微信状态；Yufu 只负责打开官方 `qrconnect` 页面。
