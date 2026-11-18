# SDK 契约 Fixture 语料 Plan

> 状态：待审批
> 输入：`docs/specs/sdk-contract-fixtures/spec.md`（已批准）

## 架构概览

本方案由三个产物组成，语料本身是唯一事实来源：

1. **静态语料** `sdk/contract-fixtures/`：manifest、Discovery、JWKS、端点行为、claims 源、预签 token、响应体、期望结果。纯文本，无运行时状态。
2. **语料生成器** `sdk/contract-fixtures/tools/gen/`：一个无第三方依赖的 Go 程序，从 claims 源和提交的 TEST-ONLY 密钥生成 `tokens/`、`jwks.json`、`responses/`，并校验再生成结果逐字节一致。
3. **参考消费者** `examples/sdk/go/`：从语料加载并启动一个固定 issuer 的本地 fixture server，重写离线测试，直接调用 Go SDK 的协议方法消费向量。

消费者只依赖三件事：JSON 解析、JWT 验签、可注入的时钟。不需要签名能力，也不读取生成器代码。

## 目录组织

```
sdk/contract-fixtures/
├── README.md                     # 消费说明、版本规则、TEST-ONLY 警告
├── manifest.json                 # 版本、issuer、测试时钟、流程常量、文件索引
├── discovery.json                # GET /.well-known/openid-configuration
├── jwks.json                     # 生成：签名公钥
├── endpoints.json                # 端点行为：请求匹配 → 响应
├── expectations.json             # token 向量的接受/拒绝期望
├── claims/                       # 源：JWT 的 header + claims
│   ├── access-token.json
│   ├── access-token-refreshed.json
│   ├── id-token.json
│   ├── logout-token.json
│   └── invalid/
│       ├── access-expired.json
│       ├── access-wrong-signature.json
│       ├── access-unknown-kid.json
│       ├── access-wrong-typ.json
│       ├── access-wrong-issuer.json
│       ├── access-wrong-audience.json
│       ├── logout-missing-events.json
│       ├── logout-with-nonce.json
│       ├── logout-lifetime-too-long.json
│       └── logout-wrong-typ.json
├── tokens/                       # 生成：与 claims/ 同构的 .jwt 文件
│   └── invalid/...
├── responses.source.json         # 源：响应体中 token 字段的占位声明
├── responses/                    # 生成：可直接返回的 JSON 响应体
│   ├── token-authorization-code.json
│   ├── token-refresh.json
│   ├── userinfo.json
│   └── introspection.json
├── keys/
│   ├── TEST-ONLY-signing-key.pem     # RSA 2048，仅用于再生成
│   └── TEST-ONLY-rotated-key.pem     # 仅用于错签名向量
└── tools/gen/
    ├── go.mod
    ├── main.go
    └── generate_test.go          # 再生成到临时目录并逐字节比对
```

`manifest.json` 中的路径全部相对语料根目录。

## 核心数据结构

### manifest.json

```json
{
  "version": "1",
  "issuer": "http://127.0.0.1:18765",
  "clock": 1793491200,
  "flow": {
    "state": "state-1",
    "nonce": "nonce-1",
    "code": "code-1",
    "code_verifier": "fixture-verifier-0123456789abcdefghijklmnopqrstuvwxyz",
    "wrong_code_verifier": "fixture-verifier-WRONG-0123456789abcdefghijklmnopqrstu",
    "client": {"client_id": "web", "client_secret": "secret"},
    "reader": {"client_id": "reader", "client_secret": "reader-secret"},
    "redirect_uri": "https://app.example/callback",
    "post_logout_redirect_uri": "https://app.example/",
    "audience": "urn:app:test",
    "scopes": ["openid", "profile", "email"]
  },
  "files": {
    "discovery": "discovery.json",
    "jwks": "jwks.json",
    "endpoints": "endpoints.json",
    "expectations": "expectations.json",
    "keys": {
      "signing": "keys/TEST-ONLY-signing-key.pem",
      "rotated": "keys/TEST-ONLY-rotated-key.pem"
    }
  }
}
```

- `clock` 是固定测试时钟（2026-11-01T00:00:00Z）。所有 token 的 `iat`/`exp` 相对它取值。
- `issuer` 同时是 fixture server 的监听地址。端口 18765 避开集成脚本已使用的 14000/18080。

### claims 源

```json
{
  "id": "access-token",
  "key": "signing",
  "header": {"alg": "RS256", "kid": "key-1", "typ": "at+jwt"},
  "claims": {
    "iss": "http://127.0.0.1:18765",
    "sub": "42",
    "aud": "urn:app:test",
    "iat": 1793491140,
    "exp": 1793491500,
    "jti": "jti-1",
    "scope": "openid profile",
    "token_use": "user_session",
    "application": "urn:app:test",
    "session_id": "session-1"
  }
}
```

- `key: "rotated"` 配合 `kid: "key-1"` 产生错签名向量；`key: "signing"` 配合不存在的 `kid` 产生未知 kid 向量。
- 负向向量只改必要的字段：过期改 `exp`，错 typ/iss/aud 改对应 claim，logout 负向分别缺 `events`、加 `nonce`、拉长生命周期、改 `typ`。

### endpoints.json

按端点分组，组内 case 有序，首个匹配者生效：

```json
{
  "endpoints": [
    {
      "method": "POST",
      "path": "/oidc/token",
      "cases": [
        {
          "id": "authorization-code-success",
          "form_equals": {
            "grant_type": "authorization_code",
            "code": "code-1",
            "code_verifier": "fixture-verifier-0123456789abcdefghijklmnopqrstuvwxyz"
          },
          "response": {"status": 200, "body_file": "responses/token-authorization-code.json"}
        },
        {
          "id": "authorization-code-wrong-pkce",
          "form_equals": {"grant_type": "authorization_code"},
          "response": {"status": 400, "json": {"error": "invalid_grant"}}
        },
        {
          "id": "refresh-success",
          "form_equals": {"grant_type": "refresh_token", "refresh_token": "refresh-1"},
          "response": {"status": 200, "body_file": "responses/token-refresh.json"}
        }
      ],
      "default": {"status": 400, "json": {"error": "unsupported_grant_type"}}
    },
    {
      "method": "GET",
      "path": "/oidc/userinfo",
      "cases": [
        {
          "id": "userinfo-success",
          "headers_match": {"Authorization": "^Bearer [^ ]+$"},
          "response": {"status": 200, "body_file": "responses/userinfo.json"}
        }
      ],
      "default": {"status": 401, "json": {"error": "invalid_token"}}
    },
    {
      "method": "POST",
      "path": "/oidc/introspect",
      "cases": [
        {
          "id": "introspection-success",
          "form_present": ["token"],
          "form_equals": {"client_id": "reader", "client_secret": "reader-secret"},
          "response": {"status": 200, "body_file": "responses/introspection.json"}
        }
      ],
      "default": {"status": 401, "json": {"error": "invalid_client"}}
    },
    {
      "method": "POST",
      "path": "/oidc/revoke",
      "cases": [],
      "default": {"status": 200}
    }
  ]
}
```

匹配器全部为 AND 语义：

| 匹配器 | 含义 |
|---|---|
| `form_equals` | 表单字段必须全部存在且值相等 |
| `form_present` | 表单字段必须存在，值不限 |
| `headers_match` | 请求头必须匹配对应正则 |

响应二选一：`json` 内联或 `body_file` 引用。未匹配任何 case 时使用端点 `default`；路径未声明返回 404。

### expectations.json

```json
{
  "access_token": [
    {"id": "access-token", "token": "tokens/access-token.jwt",
     "audience": "urn:app:test", "expect": "accept"},
    {"id": "access-expired", "token": "tokens/invalid/access-expired.jwt",
     "audience": "urn:app:test", "expect": "reject", "reason": "expired"}
  ],
  "id_token": [
    {"id": "id-token", "token": "tokens/id-token.jwt",
     "nonce": "nonce-1", "expect": "accept"}
  ],
  "logout_token": [
    {"id": "logout-token", "token": "tokens/logout-token.jwt", "expect": "accept"},
    {"id": "logout-missing-events", "token": "tokens/invalid/logout-missing-events.jwt",
     "expect": "reject", "reason": "missing_events"}
  ]
}
```

拒绝原因类别是规范值：`expired`、`bad_signature`、`unknown_kid`、`typ_mismatch`、`issuer_mismatch`、`audience_mismatch`、`missing_events`、`nonce_present`、`lifetime_too_long`。消费者负责把自身错误映射到这些类别，语料不约束错误文案。

## 生成器设计

- 单文件 Go 程序，仅用标准库（`crypto/rsa`、`crypto/sha256`、`encoding/json`、`encoding/pem`），独立 `go.mod`。
- 读取 `manifest.json`、`claims/**/*.json`、`responses.source.json`、`keys/`：
  1. 解析并校验 claims 结构；
  2. 用对应私钥做 RS256（PKCS#1 v1.5）签名，写 `tokens/`；
  3. 从签名公钥生成 `jwks.json`；
  4. 解析 `responses.source.json` 中的 token 占位，替换为已签 JWT，写 `responses/`。
- `responses.source.json` 只声明 token 字段，例如：
  ```json
  {
    "token-authorization-code": {
      "access_token": {"token": "access-token"},
      "id_token": {"token": "id-token"},
      "refresh_token": "refresh-1",
      "token_type": "Bearer",
      "expires_in": 300,
      "scope": "openid profile"
    }
  }
  ```
- 生成确定性：Go 的 `encoding/json` 对 map 键排序，RSA PKCS#1 v1.5 签名确定，因此相同输入必然逐字节一致。输出统一 2 空格缩进、结尾换行。
- `generate_test.go`：生成到 `t.TempDir()`，与仓库内 `tokens/`、`responses/`、`jwks.json` 做递归字节比较；不一致即失败。该测试同时覆盖 AC8。
- 密钥只生成一次并提交；生成器不负责创建或覆盖密钥，缺失时直接报错。

## 参考消费者设计

新增 `examples/sdk/go/fixtures_test.go`，包含三部分：

1. **加载器**：语料根目录取 `YUFU_CONTRACT_FIXTURES`，缺省为 `../../../sdk/contract-fixtures`。加载 manifest 并校验所有引用文件存在。
2. **fixture server**：解析 manifest 的 `issuer` 得到 `host:port`，用该地址监听；注册 Discovery、JWKS 和 `endpoints.json` 中的端点。收到请求后按 case 顺序匹配，执行 default 或返回 404。
3. **测试**：
   - `TestContractDiscoveryAndJWKS`：SDK 完成发现并成功验签。
   - `TestContractAuthorizationCodeFlow`：直接调用 SDK——用固定 `State/Nonce` 构造授权 URL，断言 `SHA256(verifier)=challenge`；用 manifest 的 `code` 与 `code_verifier` 调用 `ExchangeCode`；校验 ID Token 的 `nonce`、访问令牌的 `aud/token_use`。
     PKCE verifier 由 SDK 随机生成，因此不把它用于 token 交换；交换使用语料的规范 verifier。这是 HTTP 契约与 SDK 内部生成逻辑的边界。
   - `TestContractRefresh`：用语料 refresh token 刷新并校验返回的新访问令牌。
   - `TestContractEndpointErrors`：表驱动覆盖错 PKCE、不支持的 grant、UserInfo 缺 Bearer、Introspection 错凭据。
   - `TestContractNegativeTokens`：表驱动消费 `expectations.json`，按类别断言拒绝原因；Go 侧维护“SDK 错误描述 → 规范类别”的映射表。
   - `TestContractRevokeAndLogout`：Revoke 幂等成功；合法 logout token 通过，四类负向 logout token 被拒绝。
   - 保留 demo 的 backchannel replay 幂等测试：用语料 logout token 两次 POST demo handler，仍应返回 200。

所有 SDK client 都注入 `Now: func() time.Time { return time.Unix(manifest.Clock, 0) }`。

现有 `sdk_test.go` 中的 `testSigner`、`fakeIssuer`、`setAuthorization`、`logoutToken` 全部删除。

## 模块交互

```
claims/*.json ──┐
keys/*.pem ─────┼─> tools/gen ─> tokens/*.jwt ─> responses/*.json
responses.source.json ─┘                │
                                        └─> jwks.json
manifest.json ──> fixtures_test.go 加载器 ──> fixture server
                                                 │
SDK client (Now=clock) ── HTTP ──────────────────┘
        │
        └─> expectations.json ──> 正向/负向断言
```

## 技术决策

| 决策点 | 选择 | 理由 |
|---|---|---|
| 语料位置 | `sdk/contract-fixtures/` | 策略文档 §4.5 已预留；保持单一共享目录 |
| Token 策略 | 预签静态 JWT + TEST-ONLY 密钥 | 消费者只需解析与验签；语料可 diff、可审计 |
| 时间语义 | 固定测试时钟 | logout token 的 120 秒生命周期无法用远期 exp 表达；时钟注入让全部向量统一 |
| issuer | 固定 `http://127.0.0.1:18765` | 静态 JWT 的 `iss` 必须确定；任意语言都能直接消费，无需 transport 改写 |
| 端点行为 | 声明式有序 case | 语言无关；避免每个语言重写 fake server 的路由逻辑 |
| PKCE 交换 | 使用语料的规范 verifier | SDK 不允许固定 verifier；授权 URL 的生成逻辑单独断言，不混入 HTTP 契约 |
| 响应体 | 生成器内联 token | 避免同一 JWT 在多处重复维护 |
| 再生成 | 生成到临时目录做字节比对 | 防止 claims 与 tokens 漂移；不污染工作区 |
| 错误断言 | 规范原因类别 | 不绑定各语言错误文案 |

## Spec 覆盖

| 需求 | 实现位置 |
|---|---|
| F1 | `manifest.json` + 加载器自校验 |
| F2 | `discovery.json` |
| F3 | `keys/` + 生成器产出的 `jwks.json` |
| F4 | `claims/access-token.json`、`id-token.json`、`access-token-refreshed.json` 及响应体 |
| F5 | `endpoints.json` + `responses/` |
| F6 | `claims/logout-token.json` 与 4 个负向 claims |
| F7 | 6 个负向 access claims + `expectations.json` |
| F8 | `expectations.json` 的 `expect` / `reason` |
| F9 | `examples/sdk/go/fixtures_test.go` |
| F10 | `sdk/contract-fixtures/README.md` |

## 风险与缓解

| 风险 | 缓解 |
|---|---|
| 18765 端口被占用 | fixture server 启动失败时明确报错；README 说明同一主机不要并行运行多语言契约测试 |
| 消费者不支持注入时钟 | README 将时钟注入列为硬性要求；Go SDK 已支持 `Config.Now` |
| 负向错误映射随 SDK 文案变化 | 映射表集中在参考消费者测试中；语料只约束原因类别 |
| 生成器与手工文件不一致 | `generate_test.go` 对生成物做字节比对；手工文件（manifest/discovery/endpoints）由加载器自校验 |
| 语料被误用于生产 | 私钥文件名带 `TEST-ONLY`；README 首段警告；issuer 仅为 loopback |

## 不做的事

- 不新增 Python/Java runner。
- 不改动 `integration_test.go` 与真实服务器流程。
- 不改动 Go SDK 仓库的公开 API。
- 不引入第三方测试依赖或 OpenAPI 生成器。
- 不添加 CI 工作流；再生成校验通过本地/后续 CI 步骤接入。