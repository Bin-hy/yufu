# Go 资源服务器接入

适用于：已有登录体系、只需要在自己的 API 上校验 yufu 签发访问令牌的服务。

## 1. 先拿到三个值

| 值 | 来源 |
| --- | --- |
| `issuer` | `state.env` 的 `YUFU_ISSUER`，例如 `http://127.0.0.1:14000` |
| `client_id` | `state.env` 的 `YUFU_CLIENT_ID`（只用于初始化 SDK） |
| `audience` | `state.env` 的 `YUFU_AUDIENCE`，例如 `urn:app:development` |

`audience` 是应用环境的 access audience，**不是** client id。
令牌校验只依赖 issuer 与 JWKS；`client_id` 是 SDK 初始化要求，不参与签名校验。

## 2. 生成中间件

```bash
bash "$SKILL_DIR/scripts/scaffold-resource-server.sh" --dir ./internal/auth --pkg auth
```

脚本会做三件事：渲染 `yufu_auth.go` 与 `yufu_auth_test.go`、把 SDK 固定到
`assets/versions.env` 中的版本（可用 `YUFU_SDK_VERSION` 覆盖）、运行 `go test`。
目录必须位于已初始化的 Go module 内。

生成结果：

| 文件 | 内容 |
| --- | --- |
| `yufu_auth.go` | `Verifier`、`NewVerifier`、`Middleware`、`ClaimsFromContext` |
| `yufu_auth_test.go` | 有效令牌、无令牌、audience 不符、过期四种情况 |

## 3. 接入

```go
client, err := yufu.New(yufu.Config{
	Issuer:   issuer,
	ClientID: clientID,
})
if err != nil {
	return err
}
defer client.Close()

verifier, err := auth.NewVerifier(client, audience)
if err != nil {
	return err
}

mux.Handle("/api/", verifier.Middleware(apiHandler))
```

在 handler 里取 claims：

```go
claims, ok := auth.ClaimsFromContext(r.Context())
if !ok {
	http.Error(w, "unauthorized", http.StatusUnauthorized)
	return
}
// claims.Subject 是稳定主体标识；用 iss + sub 作为本地用户键，不要用 email。
```

## 4. 错误约定

| 情况 | 响应 |
| --- | --- |
| 没有 `Authorization` 头 | `401` + `WWW-Authenticate: Bearer error="missing_token"` |
| 签名 / issuer / audience / 过期任一不符 | `401` + `WWW-Authenticate: Bearer error="invalid_token"` |

不要在 401 响应里回显令牌或校验器的内部错误文本。

## 5. 什么时候需要 introspection

本地校验已经覆盖签名、issuer、audience、`typ` 与有效期，绝大多数请求不需要额外网络调用。
只有在需要「令牌是否已被主动撤销」的即时语义时才调用 `Introspect`，并且：

- 注册一个 Reader 凭据，凭据只放在服务端；
- 内省结果按 `sid` 与 `sub` 清理缓存；
- 不要用内省替代本地校验。

## 6. 与脚本的对应关系

`scaffold-resource-server.sh` 生成的文件就是本节描述的中间件；
手工修改后重跑脚本会覆盖同名文件，请把项目私有逻辑放在调用方，而不是改生成文件。