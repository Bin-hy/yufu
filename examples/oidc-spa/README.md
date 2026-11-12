# oidc-spa 参考客户端

一个最小的公共 SPA + 独立保护 API，演示授权码 + PKCE、标签页隔离会话、refresh token 续期、bearer 校验、Reader 内省、受保护写入和退出联动。

## 前置

1. 在中心开发者控制台注册一个公共 SPA 客户端（不配置 client secret）。
2. 回调地址填写 `http://127.0.0.1:5173/callback`（本地联调）或你的 HTTPS 回调。
3. 退出后回调填写 `http://127.0.0.1:5173/` 或对应 HTTPS 地址。
4. 为应用创建 Reader，记录 Reader client id 与 secret；Reader 只给 API 进程使用。
5. 确认当前账户已获应用分配；生产环境还需配置已审批。

## 配置

```bash
cp .env.example .env
```

浏览器可见参数（构建时注入，必须以 `VITE_` 开头）：

- `VITE_ISSUER`、`VITE_CLIENT_ID`、`VITE_REDIRECT_URI`、`VITE_POST_LOGOUT_REDIRECT_URI`、`VITE_SCOPE`、`VITE_API_BASE_URL`。

API 进程参数（只在服务端读取）：

- `API_AUDIENCE`：应用环境注册的 access audience，例如 `urn:app:production`。
- `SPA_CLIENT_ID`：用于拒绝为其他客户端签发的 token。
- `READER_CLIENT_ID`、`READER_CLIENT_SECRET`：内省凭据。
- `ALLOWED_ORIGIN`：SPA 的精确 origin。
- `ALLOW_INSECURE_LOOPBACK`：仅本地 HTTP 联调时为 `true`。

不要把 `client_secret` 或 Reader 凭据写成 `VITE_*`；`src/config.ts` 会拒绝这类变量。

## 运行

```bash
pnpm install
pnpm --filter @yufu/oidc-spa build
pnpm --filter @yufu/oidc-spa start:api
pnpm --filter @yufu/oidc-spa dev
```

打开 `http://127.0.0.1:5173/`，登录后点击受保护操作。API 默认监听 `http://127.0.0.1:3001`。

## 可观察行为

- 登录使用 `sessionStorage`，不同标签页互不共享；不启用 iframe 静默续期。
- 刷新只走 refresh token，本标签页内 single-flight。
- `POST /api/operation` 先校验 RS256、issuer、`aud=API_AUDIENCE`、`typ=at+jwt`、有效期，再内省确认 `active`、`client_id`、`sub`。
- 非注册 `Origin` 在读取 token 之前返回 403。
- `POST /oidc/backchannel-logout` 校验 logout token，按 `sid`/`sub` 清缓存；重复 `jti` 幂等。

## 安全边界

- 公共客户端没有 secret；浏览器包只包含公共参数。
- 中心 access token 的 `aud` 是 access audience，不是 client id。
- 在线检查默认正向缓存 2 秒、硬上限 5 秒；层间传递保留原始验证时刻，过期或内省失败即拒绝。
- 本示例不实现多租户、联邦登录或强认证。

## 测试

```bash
pnpm --filter @yufu/oidc-spa check
pnpm --filter @yufu/oidc-spa test
pnpm --filter @yufu/oidc-spa build
```

当前单测覆盖公共配置、OIDC 用户流、bearer 校验、Reader 内省、origin 门、后通道通知与缓存时限。构建产物已扫描，不含 Reader secret 或 Reader client id。真实跨站 TLS 与三浏览器验收（T95—T98）尚未完成，不能据此宣称浏览器级交付。
