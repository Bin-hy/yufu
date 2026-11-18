# yufu Runtime SDK 契约 Fixture 语料

> **TEST-ONLY。** `keys/` 下的私钥只用于再生成测试向量，禁止用于任何非测试环境。

这份语料描述 yufu Runtime SDK 的 HTTP/OIDC 契约。它是语言无关的纯文本，任何 SDK 只需 JSON 解析、JWT 验签和可注入的时钟即可消费。

## 目录

```
manifest.json          语料版本、固定 issuer、测试时钟、流程常量、文件索引
discovery.json         GET /.well-known/openid-configuration
jwks.json              GET /.well-known/jwks.json（生成）
endpoints.json         token / userinfo / introspect / revoke 的请求 → 响应规则
expectations.json      token 向量的接受 / 拒绝期望
claims/                JWT 的 header + claims 源
tokens/                预签 JWT（生成）
responses.source.json  响应体的 token 占位声明
responses/             可直接返回的响应体（生成）
keys/                  TEST-ONLY RSA 私钥
tools/gen/             生成器与再生成一致性测试
```

`tokens/`、`responses/`、`jwks.json` 都是产物；不要手工修改。

## 固定约定

- **issuer**：`http://127.0.0.1:18765`。静态 JWT 的 `iss` 已固定，因此消费者必须在该地址提供服务，不能改用随机端口。
- **测试时钟**：`manifest.clock`（`1793491200`，2026-11-01T00:00:00Z）。所有 `iat`/`exp` 相对它取值。消费者必须支持注入时钟；用真实时间验证会得到错误结果。
- **并发**：同一主机不要并行运行多个语言的契约测试，它们会争用 18765。

## 加载顺序

1. 读取 `manifest.json`，并校验其中引用的文件存在。
2. 读取 `discovery.json` 与 `jwks.json`，按 issuer 提供发现与验签。
3. 读取 `endpoints.json`，实现四个端点的行为。
4. 读取 `expectations.json`，逐条断言 token 接受 / 拒绝。
5. token 文件按 `expectations.json` 中的相对路径读取。

消费骨架（语言无关）：

```
bind(manifest.issuer)
serve GET /.well-known/openid-configuration -> discovery.json
serve GET /.well-known/jwks.json            -> jwks.json
serve endpoints.json rules
client = SDK(issuer=manifest.issuer,
             client_id=flow.client.client_id,
             client_secret=flow.client.client_secret,
             now=manifest.clock)
```

## 端点行为

`endpoints.json` 按端点分组，组内规则有序，首个匹配者生效；全部匹配器为 AND 语义：

| 匹配器 | 含义 |
|---|---|
| `form_equals` | 表单字段必须存在且值相等 |
| `form_present` | 表单字段必须存在，值不限 |
| `headers_match` | 请求头必须匹配正则 |

响应使用 `json` 内联或 `body_file` 引用。没有规则匹配时返回端点 `default`；未声明的路径返回 404。

`/oidc/revoke` 永远返回 200（幂等）。

## Token 期望

`expectations.json` 分三组：`access_token`、`id_token`、`logout_token`。每条包含：

- `expect: "accept" | "reject"`；
- 拒绝时的 `reason`，取值固定为：`expired`、`bad_signature`、`unknown_kid`、`typ_mismatch`、`issuer_mismatch`、`audience_mismatch`、`missing_events`、`nonce_present`、`lifetime_too_long`。

消费者负责把自身错误映射到这些类别；语料不约束错误文案。

## 修改向量

- 改 token 内容：编辑 `claims/**/*.json`，然后重新生成。
- 改端点行为：编辑 `endpoints.json` 或 `responses.source.json`。
- 改流程常量：编辑 `manifest.json` 的 `flow`。
- 新增 token：在 `claims/` 下新建文件，增加唯一 `id`，并在 `responses.source.json` 或 `expectations.json` 中引用。
- 任何字段只增不删；破坏性变更必须提升 `manifest.version` 并在本文件记录。

## 再生成

```bash
cd sdk/contract-fixtures/tools/gen
go run . -root ../..
go test -count=1 ./...
```

生成器只使用标准库，输出确定性：相同输入产生逐字节一致的 JWT、JWKS 与响应体。`go test` 会把产物生成到临时目录并与仓库内容比对，防止 claims 与 token 漂移。

不要手工创建或替换 `keys/`；密钥一旦变更，所有 token 都需要重新生成。