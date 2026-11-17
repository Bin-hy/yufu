# 应用注册

注册通过控制台管理接口完成，不需要浏览器操作。前提是本地部署写入的引导管理员账号。

## 两种模式

| 模式 | `client_kind` | 是否有 secret | 适用 |
| --- | --- | --- | --- |
| `web` | `server_web` | 有 | 服务端持有会话的 Web 后端 |
| `api` | `browser_spa` | 无 | 只校验 Bearer 的资源服务器 |

`server_web` 不允许配置 `browser_origins`；`browser_spa` 的 `browser_origins` 必须与
全部 redirect URI 的 origin 完全一致。脚本已按模式处理（登出地址同源时自动去重），
不要手工混填。

## 命令

```bash
bash "$SKILL_DIR/scripts/register-app.sh" --mode web \
  --redirect-uri http://127.0.0.1:8080/callback \
  --post-logout-uri http://127.0.0.1:8080/ \
  --audience urn:app:development \
  --scopes "openid profile email"
```

```bash
bash "$SKILL_DIR/scripts/register-app.sh" --mode api --audience urn:app:development
```

参数默认值见脚本；`--app-key` 默认由目标目录路径派生，同一项目重跑稳定。

## 调用链

1. `GET /api/v1/identity/csrf`，携带 `Origin`，拿到 `csrf_token` 并保存 cookie。
2. `POST /api/v1/identity/login`，携带 `Origin`、`x-csrf-token` 与用户名密码。
3. `POST /api/v1/admin/apps`，携带 `Origin`、`x-csrf-token`、`idempotency-key`。
4. 从响应读取 development 环境的 `client_id`；机密客户端同时读取一次性 `client_secret`。
5. 写入 `state.env` 与 `client_secret`（权限 600）。

## 幂等

- 幂等键就是 `app_key`。同一项目重跑会命中同一条操作回执。
- 令牌重放只返回应用与 client id，**不再返回 secret**。
- `state.env` 已有 `YUFU_CLIENT_ID` 时，脚本直接进入配对校验，不会重复创建应用。
- secret 文件丢失时脚本会明确失败：在控制台重置 secret，或换一个新的 `--app-key` 重新注册。

## 参数语义

| 参数 | 约束 |
| --- | --- |
| `redirect_uri` | 与授权请求逐字符一致，包括 scheme、host、port、path；不支持通配 |
| `post_logout_redirect_uri` | 同上，用于 RP 发起退出；本机版本要求它同时出现在 `redirect_uris` 中，脚本会自动加入 |
| `access_audience` | 资源服务器校验时使用的 audience，不是 client id |
| `identity_scopes` | 空格分隔；`openid` 必填，常用 `openid profile email` |
| `consent_policy` | 本项目统一 `user_consent` |
| `logout_capability` | 统一 `online`；`backchannel_and_online` 必须同时给回调地址 |

## 失败对照

| HTTP | 含义 | 处理 |
| --- | --- | --- |
| `401` | 登录失败或 cookie 未生效 | 检查管理员账号与 `allow_insecure_loopback_cookies` |
| `403` | 账号没有管理权限 | 用部署时的引导管理员 |
| `400 invalid_input` | 参数组合不合法 | 按上文约束逐项核对；`server_web` 最常见原因是误填 `browser_origins` |
| `409` | `app_key` 已存在且幂等键不同 | 复用原 `--app-key`，或换新 key |