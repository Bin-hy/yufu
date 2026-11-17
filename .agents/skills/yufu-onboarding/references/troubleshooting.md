# 故障定位

按阶段定位，不猜。每条都给出可执行的校验命令。

## 部署

| 症状 | 常见原因 | 校验 |
| --- | --- | --- |
| `preflight blocked: port 14000 is already in use` | 旧进程或其他服务占用 | `lsof -nP -iTCP:14000 -sTCP:LISTEN` |
| `Postgres did not become ready` | 容器没起来或端口冲突 | `docker logs postgres_yufu` |
| `postgres_yufu is bound to host port ...` | 旧容器端口与 `YUFU_PG_PORT` 不一致 | 停掉旧容器，或设 `YUFU_PG_PORT=<旧端口>` 后重跑 |
| `download failed` | 网络受限或发布包不可达 | 设 `YUFU_BIN` 指向本地二进制；或用代理重跑 |
| `checksum mismatch` | 归档被替换或版本文件过期 | 删除 `.yufu-onboarding/bin/` 后重试；核对 `assets/versions.env` |
| `Migration file of version ... is missing` | 数据库已被更新版本迁移，固定的旧版二进制无法打开 | 用独立的 `YUFU_DB_NAME` 重建库，或改用与迁移匹配的版本 |
| `unsupported platform` 或 `unsupported YUFU_TARGET` | 平台不在支持列表 | 使用 `aarch64-apple-darwin`、`x86_64-apple-darwin`、`x86_64-unknown-linux-gnu` 之一 |
| yufu 启动即退出 | 缺少私钥、配置错误、端口占用 | `tail -50 .yufu-onboarding/yufu.log` |
| `health ready but security clock is not synchronized` | 出站 UDP/123 被阻断 | `nc -zu time.cloudflare.com 123`；换内网时间源 |
| discovery issuer 不一致 | 配置里的 issuer 与访问地址不同 | 重新执行 `deploy-local.sh`，不要手工改配置 |

## 注册

| 症状 | 常见原因 | 校验 |
| --- | --- | --- |
| `login failed with HTTP 401` | 管理员账号不匹配 | 核对 `state.env` 的 `YUFU_ADMIN_USER` |
| `registration returned HTTP 400 invalid_input` | `server_web` 填了 `browser_origins`；或 redirect 非法 | 用 `--mode api` 对比，或核对 `references/register-app.md` 的参数约束 |
| `registration returned HTTP 409` | `app_key` 已存在但幂等键不同 | 复用原 `--app-key` |
| `client ... was registered without a new secret` | 重放不返回 secret | 控制台重置 secret，或换 `--app-key` 重注册 |

## 配对

| 症状 | 常见原因 | 校验 |
| --- | --- | --- |
| `redirect_uri mismatch: registered ..., current ...` | 环境变量覆盖了注册值 | 对齐 `YUFU_REDIRECT_URI` 与 `state.env` |
| `authorization endpoint returned HTTP 4xx` | client 不存在或 redirect 不在白名单 | `bash "$SKILL_DIR/scripts/verify-app.sh"`，响应体会给出错误码 |
| `authorization endpoint rejected the request` | scope 未授权或应用未分配访问 | 管理台确认该用户有访问分配 |
| `end-session endpoint returned HTTP 400` | 登出地址未同时注册进 `redirect_uris`（旧版脚本注册的 client） | 用新 `--app-key` 重注册；`verify-app.sh` 会点名该字段 |

## 接入

| 症状 | 常见原因 | 校验 |
| --- | --- | --- |
| 登录后回调 400 `unknown or replayed state` | state 未持久化或跨实例 | 会话存储改为共享存储 |
| 回调 401 `nonce does not match` | 校验时用了新的 nonce | 用发起授权时保存的 nonce |
| API 恒 401 `audience does not match` | 校验用了 client id 而不是 access audience | 核对 `YUFU_AUDIENCE` |
| API 偶发 401 | JWKS 轮换与缓存 | SDK 会按 `kid` 重新拉取；确认 `JWKSTTL` 不是 0 |
| `go: module lookup disabled` | 离线且模块缓存缺失 | 先在联网环境 `go mod download` |

## 通用

- 状态文件：`.yufu-onboarding/state.env`（权限 600，不要提交）。
- 服务日志：`.yufu-onboarding/yufu.log`。
- 任何一步失败，先跑 `bash "$SKILL_DIR/scripts/verify-issuer.sh"` 排除 issuer 层问题。