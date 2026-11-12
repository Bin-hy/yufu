# 重启后登录 403 的续修

日期：2026-11-08。承接 [认证集成修复](login-security-integration.md)。

用户日志显示 `GET /api/v1/csrf` 返回 200，随后 `POST /api/v1/login` 在 0ms 内返回 403。挑战接口成功只说明已生成证明，不能证明浏览器保存并回传了 Cookie，也不代表写请求的 Origin 已获授权。旧访问日志没有具体校验原因，无法据此确定用户实际触发的分支。

## 隔离复现与修复

用户配置的非敏感字段为 `http.listen = "127.0.0.1:4000"`、`auth.origins = ["*"]`，没有覆盖 issuer 或 public 地址。先前真实浏览器只覆盖 IP 地址入口，漏测了 localhost。

隔离后端使用同一份有效 CSRF 证明：IP 来源能进入账号校验并为不存在的账号返回 401；将 Origin 改成同端口 localhost 则立即返回 403。现在仅在显式 `allow_insecure_loopback_cookies` 模式下允许 issuer 相同协议、相同端口的 loopback 地址别名。其他协议/端口仍需明确来源配置，远程来源在此模式下继续拒绝，`*` 不授权携带 Cookie 的写操作。默认生产 Cookie 策略和 Origin 限制保持。

WebKit 默认 Secure Cookie 策略的隔离真实浏览器测试同样复现了挑战 200 后登录 403：HTTP 页面没有保存 `__Host-yufu-csrf`。本机 HTTP 仍需显式启动参数；未启用时，独立 CLI 启动日志给出提示，前端也显示参数与 HTTPS 要求。

浏览器校验拒绝现在返回 403 JSON `{ "code": "csrf_…", "message": "browser authentication rejected" }` 和 `Cache-Control: no-store`。前端按错误码展示具体中文提示并保留输入，不自动重放写请求。服务日志记录公开原因码；不记录证明、Cookie、密码或签名细节。Origin 拒绝日志仅记录规范化请求与 issuer 来源。

| 错误码 | 含义 |
| --- | --- |
| `csrf_origin_missing` | 没有 Origin |
| `csrf_origin_invalid` | Origin 格式无效、重复或不是规范来源 |
| `csrf_origin_not_allowed` | 来源未授权 |
| `csrf_cookie_invalid` | Cookie 格式无效、重复或过大 |
| `csrf_cookie_missing` | 未携带所需 Cookie |
| `csrf_cookie_requires_https` | HTTP 使用 Secure 策略且未携带所需 Cookie |
| `csrf_proof_missing` | 未携带校验证明 |
| `csrf_proof_mismatch` | 请求证明和 Cookie 不一致 |
| `csrf_proof_invalid` | 证明验证失败，需要重新获取 |

时钟失健康或预算失败继续返回原有 503，账号认证错误继续返回 401。

## 验证

- 回归测试在修复前稳定复现 localhost 来源被拒和六种前端错误提示缺失；修复后前端 66 项、Rust 库 43 项测试通过。
- 隔离认证集成回归 8 项通过，包括来源别名、协议/端口/远程拒绝、缺失/无效 Cookie 或证明、时钟失健康及账号流程。
- 隔离撤销语义回归 9 项通过，覆盖 CSRF 拒绝无副作用、证明轮换绑定、退出提交后清 Cookie、OIDC 确认退出、客户端边界及管理会话撤销。
- 三种浏览器分别在 IP 与 localhost 入口完成真实注册、管理员登录、当前身份读取、长 JWT 恢复、刷新与退出，六组流程全部通过。WebKit 默认 Secure 策略的单独用例验证挑战 200、登录 403 和可操作的页面提示。
- 类型检查和生产构建、lint、Rust 库与根认证测试/独立 CLI 的 Clippy、Rust 格式和 diff 检查通过，三种 Rust 启动二进制构建通过。

最新实际 WebUI 入口为 `assets/index-CtkJUYqG.js`。未向用户业务服务发送登录或其他写请求，未更改真实配置或管理用户服务进程。业务账户和业务权限仍需本地实际验收；本轮没有重跑全部工作台或视觉截图用例。

## 本机启动

前端已构建，重启后端并强制刷新页面：

```sh
cargo run --bin yufu --features webui -- \
  --config conf/yufu.local/yufu.toml \
  --allow-insecure-loopback-cookies
```

启动日志应出现 `explicit loopback HTTP development cookies enabled`。浏览器可访问 `http://127.0.0.1:4000` 或 `http://localhost:4000`；同一次使用过程中保持地址一致，两个主机的 Cookie 不共享。若仍拒绝，Network 的登录响应 JSON 和日志 `reason=csrf_…` 会指出分支，无需粘贴密码、Token 或 Cookie。
