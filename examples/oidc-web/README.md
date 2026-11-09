# oidc-web 参考客户端

一个最小的机密 Web 客户端，演示标准授权码 + PKCE 登录、服务端会话、UserInfo、在线内省、受保护写入、本地退出、RP 发起退出和后通道退出。

## 前置

1. 在中心开发者控制台注册一个 `development` 或 `production` 应用。
2. 回调地址填写 `http://127.0.0.1:3000/callback`（本地联调）或你的 HTTPS 回调。
3. 退出后回调填写 `http://127.0.0.1:3000/` 或对应 HTTPS 地址。
4. 为应用创建 Reader，记录 Reader client id 与一次性显示的 secret。
5. 确认当前账户已获应用分配；生产环境还需配置已审批。

## 配置

```bash
cp .env.example .env
```

填写 `ISSUER`、`CLIENT_ID`、`CLIENT_SECRET`、`REDIRECT_URI`、`POST_LOGOUT_REDIRECT_URI`、`SESSION_SECRET`、`READER_CLIENT_ID`、`READER_CLIENT_SECRET`。

- `SESSION_SECRET` 至少 32 字符，仅用于本地会话 Cookie 的 CSRF 绑定。
- `ALLOW_INSECURE_LOOPBACK=true` 只允许 `127.0.0.1`/`localhost` 的 HTTP 联调；生产必须为 `false`。
- `CLIENT_SECRET` 与 `READER_CLIENT_SECRET` 只存在于服务端进程，不要写入任何前端变量。

## 运行

```bash
pnpm install
pnpm --filter @cobotrix/oidc-web build
pnpm --filter @cobotrix/oidc-web start
```

打开 `http://127.0.0.1:3000/`，点击登录，完成中心认证与同意后回到应用。

## 可观察行为

- `GET /`：已登录显示主体与两个表单；未登录显示登录入口。
- `GET /api/me`：只返回本地会话中的业务资料，不返回 token。
- `POST /api/operation`：校验同源与 CSRF 后，在线内省 access token，再执行示例写入。
- `POST /logout`：只清本应用会话，并尽力撤销本应用授权。
- `GET /logout/central`：跳转中心 RP 发起退出。
- `POST /oidc/backchannel-logout`：接收 `logout_token`，按 `sid`/`sub` 清本地会话与授权缓存；重复 `jti` 返回 `duplicate`。

## 安全边界

- 浏览器 Cookie 为 HttpOnly；token 不进入浏览器。
- 回调严格校验 state、nonce、issuer、audience 与 ID Token 签名。
- 受保护写入的在线检查默认正向缓存 2 秒、硬上限 5 秒；慢内省不延长新鲜度，过期或内省失败即拒绝。
- 本示例不实现多租户、联邦登录或强认证。

## 测试

```bash
pnpm --filter @cobotrix/oidc-web check
pnpm --filter @cobotrix/oidc-web test
pnpm --filter @cobotrix/oidc-web build
```

当前单测覆盖配置、授权请求/回调校验、会话、退出、CSRF、内省、缓存时限与后通道通知。真实跨站 TLS 与三浏览器验收（T95—T98）尚未完成，不能据此宣称浏览器级交付。
