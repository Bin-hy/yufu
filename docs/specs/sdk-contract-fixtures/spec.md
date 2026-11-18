# SDK 契约 Fixture 语料 Spec

> 状态：待审批
> 关联：`docs/sdk/sdk-strategy-2026-11-11.md` §4.5（`sdk/contract-fixtures/` 布局与共享契约规则）

## 背景

Runtime SDK 的协议契约目前只存在于 `examples/sdk/go/sdk_test.go` 的 fake issuer 中：

- RSA 密钥每个测试随机生成，JWT 由测试现场签名；
- issuer URL 来自 `httptest.Server` 的临时地址，claims 时间戳来自 `time.Now()`；
- 授权码流程的 `state`、`nonce`、`code_verifier` 都是运行时随机值；
- token 端点按 `grant_type`、`code_verifier` 分支，UserInfo/Introspection 按请求头与凭据分支。

这些数据无法被第二个语言复用。SDK 已按语言独立实现（Go 已在 `Bin-hy/yufu-sdk-go`，后续 Python/Java），如果没有共享语料，各语言对同一协议行为的判断只能靠人读代码对齐。

本期的目标是把这套 fake issuer 沉淀为**语言无关、纯静态、离线可验证**的契约语料，并让现有 Go 示例测试成为参考消费者，证明语料足以表达协议行为。

## 目标

1. 产出一份版本化、自描述的契约语料，任何语言只需 JSON 解析 + JWT 验签即可消费。
2. 语料覆盖现有 fake issuer 的全部场景，并补充安全基线中的负向 token 向量。
3. 现有 Go 示例测试改为消费语料，不再现场生成密钥、URL 或时间戳。
4. 为 Python/Java SDK 提供可直接复用的输入，本期不实现它们的 runner。

## 功能需求

- **F1 语料清单（manifest）**：提供一份机器可读的清单，声明语料版本、固定 issuer、固定测试时钟、整条授权码流程的固定常量（`state`、`nonce`、`code_verifier`、`code`、客户端凭据）、密钥引用、场景索引，以及所有被引用文件。清单必须可自校验：引用的文件缺失或字段非法时，消费者应能明确报错。
- **F2 Discovery 向量**：一份完整的 OIDC Discovery 文档，端点全部指向固定 issuer，声明 `response_types_supported`、`grant_types_supported`、`scopes_supported`、`code_challenge_methods_supported`，以及 `backchannel_logout_supported` / `backchannel_logout_session_supported`。
- **F3 JWKS 与密钥向量**：一份 JWKS，包含语料签名密钥的公钥；提交对应的 TEST-ONLY 私钥仅用于再生成。语料需明确标注私钥不可用于生产。
- **F4 Token 正向向量**：`authorization_code` 与 `refresh_token` 两种授权的成功响应，均为预签静态 JWT。访问令牌采用 `typ=at+jwt`，claims 至少包含 `iss/sub/aud/iat/exp/jti/scope/token_use/application/session_id`；ID Token 采用 `typ=JWT`，claims 至少包含 `iss/sub/aud/iat/exp/nonce/sid`。
- **F5 端点行为清单**：以“请求匹配条件 → 响应”的形式声明 token、UserInfo、Introspection、Revocation 四个端点的行为，至少覆盖：
  - 授权码兑换成功；
  - 刷新成功；
  - PKCE 校验失败（错误的 `code_verifier`）；
  - 不支持的 `grant_type`；
  - UserInfo 缺失 Bearer；
  - Introspection 凭据错误与成功；
  - Revocation 返回 200。
  匹配条件必须精确到方法、路径、表单字段或请求头；未列出的请求应得到明确的兜底响应。
- **F6 Logout Token 向量**：一个合法的 `logout+jwt`（含唯一 `events`、`jti`、`sid` 或 `sub`、生命周期 ≤ 120 秒），以及以下负向向量：缺失 `events`、包含 `nonce`、生命周期超过 120 秒、`typ` 不正确。
- **F7 安全基线负向 Token 向量**：在固定测试时钟下，提供以下负向访问令牌并声明期望拒绝原因类别：已过期、签名错误、未知 `kid`、`typ` 不正确、`iss` 不匹配、`aud` 不匹配。合法访问令牌必须能被接受。
- **F8 期望结果**：每个向量附带机器可读的期望（接受 / 拒绝 + 拒绝原因类别）。消费者断言语义结果，不依赖具体错误文案或 Go 错误类型。
- **F9 参考消费者**：现有 Go 示例测试改为从语料加载 Discovery、JWKS、token 响应与端点行为，使用语料的固定时钟完成 token 校验；负向向量以表驱动方式逐条断言。测试不再自行生成 RSA 密钥、不再用 `time.Now()` 构造 claims。
- **F10 消费说明**：提供 README，说明语料布局、加载顺序、固定 issuer 与测试时钟的使用方式、如何新增/修改向量、版本与兼容规则，以及 TEST-ONLY 密钥的警告。

## 非功能需求

- **N1 纯文本**：语料只包含 UTF-8 JSON 与 JWT 文本，不引入二进制资源。
- **N2 离线可验证**：消费语料不依赖公网、DNS 或容器；仅使用 loopback。
- **N3 可确定性再生**：相同私钥与相同 claims 必须产生逐字节一致的 JWT；提供再生成手段，重复执行不产生 diff。
- **N4 安全标注**：TEST-ONLY 私钥在文件名和 README 中显著标注，禁止用于任何非测试环境。
- **N5 兼容演进**：字段只增不删；破坏性变更必须提升语料版本，并在 README 记录变更。
- **N6 语言无关**：语料不包含任何语言的类型、函数或路径约定；消费方无需阅读 Go 代码。
- **N7 测试命令不变**：`examples/sdk/go` 的离线测试仍通过 `go test -count=1 ./...` 执行，且不访问外部网络。

## 不做的事

- 不实现 Python/Java 的 contract runner（等对应 SDK 立项后再做）。
- 不引入 OpenAPI 生成器，不产出生成式 SDK。
- 不修改 live integration 测试或其真实服务器流程。
- 不覆盖 Management API。
- 不包含 JWKS 轮转、Clock skew、refresh token 轮转、scope 缩小等扩展场景。
- 不把语料发布为独立仓库或独立发布物。

## 验收标准

- **AC1**：清单可自校验；所有被引用文件存在，JSON 可被 `jq` 解析。
- **AC2**：消费方仅凭 Discovery 与 JWKS 即可完成发现与验签准备。
- **AC3**：使用语料常量可完整走通授权码 + PKCE 流程，ID Token 的 `nonce`、访问令牌的 `aud/token_use` 断言通过。
- **AC4**：使用语料可完成一次 refresh 流程并得到新的访问令牌。
- **AC5**：token/UserInfo/Introspection/Revocation 的正向与错误场景返回与清单一致的状态码与 OAuth 错误码。
- **AC6**：合法 logout token 被接受；F6 的四类负向 logout token 均被拒绝。
- **AC7**：F7 的六类负向访问令牌均被拒绝且拒绝原因类别匹配；合法访问令牌被接受。
- **AC8**：重新执行语料再生成后 `git diff` 为空。
- **AC9**：Go 参考消费者在固定测试时钟下通过，且测试代码中不再出现现场生成 RSA 密钥或 `time.Now()` 构造 claims。
- **AC10**：README 能独立指导一个新语言的实现者加载并跑通全部向量。