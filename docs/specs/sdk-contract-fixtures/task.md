# SDK 契约 Fixture 语料 Tasks

> 状态：待审批
> 输入：`docs/specs/sdk-contract-fixtures/spec.md`、`plan.md`（均已批准）

## 文件清单

| 操作 | 文件 | 职责 |
|---|---|---|
| 新建 | `sdk/contract-fixtures/manifest.json` | 版本、issuer、时钟、流程常量、文件索引 |
| 新建 | `sdk/contract-fixtures/discovery.json` | Discovery 向量 |
| 新建 | `sdk/contract-fixtures/endpoints.json` | 端点行为清单 |
| 新建 | `sdk/contract-fixtures/expectations.json` | token 接受/拒绝期望 |
| 新建 | `sdk/contract-fixtures/responses.source.json` | 响应体 token 占位声明 |
| 新建 | `sdk/contract-fixtures/claims/access-token.json` | 访问令牌 claims 源 |
| 新建 | `sdk/contract-fixtures/claims/access-token-refreshed.json` | 刷新后的访问令牌 claims 源 |
| 新建 | `sdk/contract-fixtures/claims/id-token.json` | ID Token claims 源 |
| 新建 | `sdk/contract-fixtures/claims/logout-token.json` | 合法 logout token claims 源 |
| 新建 | `sdk/contract-fixtures/claims/invalid/access-expired.json` | 过期访问令牌 |
| 新建 | `sdk/contract-fixtures/claims/invalid/access-wrong-signature.json` | 错签名访问令牌 |
| 新建 | `sdk/contract-fixtures/claims/invalid/access-unknown-kid.json` | 未知 kid 访问令牌 |
| 新建 | `sdk/contract-fixtures/claims/invalid/access-wrong-typ.json` | 错 typ 访问令牌 |
| 新建 | `sdk/contract-fixtures/claims/invalid/access-wrong-issuer.json` | 错 iss 访问令牌 |
| 新建 | `sdk/contract-fixtures/claims/invalid/access-wrong-audience.json` | 错 aud 访问令牌 |
| 新建 | `sdk/contract-fixtures/claims/invalid/logout-missing-events.json` | 缺 events 的 logout token |
| 新建 | `sdk/contract-fixtures/claims/invalid/logout-with-nonce.json` | 含 nonce 的 logout token |
| 新建 | `sdk/contract-fixtures/claims/invalid/logout-lifetime-too-long.json` | 生命周期 121 秒的 logout token |
| 新建 | `sdk/contract-fixtures/claims/invalid/logout-wrong-typ.json` | 错 typ 的 logout token |
| 新建 | `sdk/contract-fixtures/keys/TEST-ONLY-signing-key.pem` | 签名私钥（仅测试） |
| 新建 | `sdk/contract-fixtures/keys/TEST-ONLY-rotated-key.pem` | 错签名用私钥（仅测试） |
| 新建 | `sdk/contract-fixtures/tools/gen/go.mod` | 生成器模块 |
| 新建 | `sdk/contract-fixtures/tools/gen/main.go` | 生成器 |
| 新建 | `sdk/contract-fixtures/tools/gen/generate_test.go` | 再生成字节比对 |
| 生成 | `sdk/contract-fixtures/jwks.json` | 签名公钥 JWKS |
| 生成 | `sdk/contract-fixtures/tokens/**/*.jwt` | 预签 token |
| 生成 | `sdk/contract-fixtures/responses/*.json` | 响应体 |
| 新建 | `sdk/contract-fixtures/README.md` | 消费说明与版本规则 |
| 新建 | `examples/sdk/go/fixtures_test.go` | 加载器、fixture server、契约测试 |
| 修改 | `examples/sdk/go/sdk_test.go` | 删除现场造数，保留 demo replay 测试 |
| 修改 | `examples/sdk/go/README.md` | 说明语料位置与环境变量 |

## T1: 建立目录与 manifest

**文件：** `sdk/contract-fixtures/manifest.json`
**依赖：** 无
**步骤：**
1. 创建 `sdk/contract-fixtures/{claims/invalid,tokens,tokens/invalid,responses,keys,tools/gen}`。
2. 写入 manifest：`version "1"`、`issuer "http://127.0.0.1:18765"`、`clock 1793491200`、`flow` 常量（`state-1`、`nonce-1`、`code-1`、`fixture-verifier-0123456789abcdefghijklmnopqrstuvwxyz`、错误 verifier、`web/secret`、`reader/reader-secret`、redirect、post logout、`urn:app:test`、scopes）。
3. 写入 `files` 索引：discovery、jwks、endpoints、expectations、keys。

**验证：** `jq . sdk/contract-fixtures/manifest.json` 输出合法；`jq -r .issuer` 为固定 issuer。

## T2: Discovery 向量

**文件：** `sdk/contract-fixtures/discovery.json`
**依赖：** T1
**步骤：**
1. 写入 `issuer` 与全部端点 URL，均指向 manifest 的 issuer。
2. 声明 `response_types_supported ["code"]`、`grant_types_supported ["authorization_code","refresh_token"]`、`scopes_supported`、`code_challenge_methods_supported ["S256"]`、`id_token_signing_alg_values_supported ["RS256"]`、`token_endpoint_auth_methods_supported ["client_secret_post","none"]`、`subject_types_supported ["public"]`，以及两个 backchannel 能力为 `true`。

**验证：** `jq -r .issuer` 与 manifest 一致；`jq '.backchannel_logout_supported and .backchannel_logout_session_supported'` 为 `true`。

## T3: 正向 claims 源

**文件：** 4 个 `claims/*.json`
**依赖：** T1
**步骤：**
1. `access-token.json`：`kid key-1`、`typ at+jwt`、`iss`、`sub 42`、`aud urn:app:test`、`iat 1793491140`、`exp 1793491500`、`jti jti-1`、`scope openid profile`、`token_use user_session`、`application urn:app:test`、`session_id session-1`。
2. `access-token-refreshed.json`：同上，`jti jti-2`。
3. `id-token.json`：`typ JWT`、`aud web`、`nonce nonce-1`、`sid sid-1`，其余同上。
4. `logout-token.json`：`typ logout+jwt`、`aud web`、`iat 1793491140`、`exp 1793491260`、`jti jti-3`、`sid sid-1`、`sub 42`、`events` 仅含 `http://schemas.openid.net/event/backchannel-logout: {}`。

**验证：** `jq` 解析全部文件；logout 的 `exp - iat` 等于 120。

## T4: 负向 claims 源

**文件：** 10 个 `claims/invalid/*.json`
**依赖：** T3
**步骤：**
1. 6 个 access 负向：过期（`iat 1793484000`、`exp 1793487600`）、错签名（`key rotated` 但 `kid key-1`）、未知 kid（`key signing`、`kid key-2`）、错 typ（`JWT`）、错 iss（`https://evil.example`）、错 aud（`urn:app:other`）。
2. 4 个 logout 负向：缺 `events`、加 `nonce nonce-1`、`exp 1793491261`（121 秒）、`typ JWT`。
3. 除被修改字段外，其余 claims 保持与正向向量一致。

**验证：** `ls claims/invalid/*.json | wc -l` 为 10；`jq` 逐一解析通过。

## T5: 生成 TEST-ONLY 密钥

**文件：** `keys/TEST-ONLY-signing-key.pem`、`keys/TEST-ONLY-rotated-key.pem`
**依赖：** T1
**步骤：**
1. 用 `openssl genrsa -traditional -out <path> 2048` 生成两把 RSA 2048 私钥；若 `-traditional` 不可用，允许 PKCS#8 输出。
2. 确认生成器同时支持 PKCS#1 与 PKCS#8。

**验证：** `openssl rsa -in <path> -check -noout` 对两把密钥均通过。

## T6: 响应体源

**文件：** `sdk/contract-fixtures/responses.source.json`
**依赖：** T3
**步骤：**
1. 声明 `token-authorization-code`：`access_token {"token":"access-token"}`、`id_token {"token":"id-token"}`、`refresh_token "refresh-1"`、`token_type "Bearer"`、`expires_in 300`、`scope "openid profile"`。
2. 声明 `token-refresh`：`access_token {"token":"access-token-refreshed"}`、`refresh_token "refresh-2"`、`token_type "Bearer"`、`expires_in 300`。
3. 声明 `userinfo` 与 `introspection` 的静态字段（`introspection` 的 `exp` 为 `1793491500`）。

**验证：** `jq` 解析通过；所有 `{"token":...}` 引用的 id 在 `claims/` 中存在。

## T7: 端点行为清单

**文件：** `sdk/contract-fixtures/endpoints.json`
**依赖：** T6
**步骤：**
1. `/oidc/token`：case 顺序为成功（精确 `code_verifier`）→ 其他 `authorization_code` 返回 400 `invalid_grant` → refresh 成功；default 400 `unsupported_grant_type`。
2. `/oidc/userinfo`：`Authorization` 匹配 `^Bearer [^ ]+$` 返回 200；default 401 `invalid_token`。
3. `/oidc/introspect`：`token` 存在且 reader 凭据精确匹配返回 200；default 401 `invalid_client`。
4. `/oidc/revoke`：无 case，default 200 空响应。

**验证：** `jq` 解析通过；`body_file` 引用的名称都在 `responses.source.json` 中。

## T8: 期望结果

**文件：** `sdk/contract-fixtures/expectations.json`
**依赖：** T3、T4
**步骤：**
1. `access_token`：1 条 accept + 6 条 reject，原因依次为 `expired`、`bad_signature`、`unknown_kid`、`typ_mismatch`、`issuer_mismatch`、`audience_mismatch`。
2. `id_token`：1 条 accept，`nonce "nonce-1"`。
3. `logout_token`：1 条 accept + 4 条 reject，原因为 `missing_events`、`nonce_present`、`lifetime_too_long`、`typ_mismatch`。
4. 所有 `token` 路径指向 `tokens/` 下的文件。

**验证：** `jq` 解析通过；`jq '[.access_token[],.id_token[],.logout_token[]] | length'` 为 13。

## T9: 实现生成器

**文件：** `sdk/contract-fixtures/tools/gen/go.mod`、`main.go`
**依赖：** T1–T6
**步骤：**
1. `go.mod`：module `yufu-contract-fixture-gen`，仅标准库。
2. `main.go`：
   - `-root` 参数，缺省 `../..`；
   - 读取 manifest、claims 目录、`responses.source.json`、两把私钥；
   - 校验 claims 结构与 id 唯一性；
   - 按 `key` 选择私钥，构造 header/claims，RS256 签名，写 `tokens/`；
   - 从 signing key 生成 `jwks.json`（`kid key-1`）；
   - 替换响应源中的 token 占位，写 `responses/`；
   - 统一 2 空格缩进与结尾换行。

**验证：** `cd sdk/contract-fixtures/tools/gen && go vet ./...` 通过。

## T10: 生成并检查产物

**文件：** 生成 `jwks.json`、`tokens/**`、`responses/*.json`
**依赖：** T9
**步骤：**
1. 运行 `go run . -root ../..`。
2. 抽取任一 token 的 header/claims 做 base64url 解码检查。
3. 检查 `responses/token-authorization-code.json` 中的 JWT 与 `tokens/access-token.jwt` 一致。

**验证：** 生成文件数量与 claims 数量一致（14 个 jwt）；`jwks.json` 的 `keys[0].kid` 为 `key-1`；响应体中的 JWT 可解码。

## T11: 再生成一致性测试

**文件：** `sdk/contract-fixtures/tools/gen/generate_test.go`
**依赖：** T10
**步骤：**
1. 生成到 `t.TempDir()`。
2. 递归比对 `tokens/`、`responses/`、`jwks.json` 与仓库内文件，逐字节比较。
3. 差异时输出首个不一致文件的路径。

**验证：** `go test -count=1 ./...` 在 `tools/gen` 下通过。

## T12: 消费说明

**文件：** `sdk/contract-fixtures/README.md`
**依赖：** T10
**步骤：**
1. 首段写 TEST-ONLY 警告。
2. 说明布局、加载顺序、固定 issuer 18765、固定时钟、时钟注入是硬性要求。
3. 说明匹配器语义、拒绝原因类别、如何新增向量、再生成命令。
4. 说明同一主机不要并行运行多语言契约测试；语料版本只增不删。

**验证：** 文档中引用的路径全部存在；命令可复制执行。

## T13: 实现加载器与 fixture server

**文件：** `examples/sdk/go/fixtures_test.go`
**依赖：** T10
**步骤：**
1. 定义 manifest、endpoint、expectation 的 Go 结构。
2. 实现语料根解析：`YUFU_CONTRACT_FIXTURES` 或 `../../../sdk/contract-fixtures`；校验引用文件存在。
3. 解析 issuer 的 host:port，用 `net.Listen` 固定监听；启动 `http.Server`，测试结束关闭。
4. 实现 Discovery/JWKS 文件服务与端点 case 匹配（`form_equals`、`form_present`、`headers_match`）；未匹配走 default，未知路径 404。
5. 提供固定时钟 helper。

**验证：** `cd examples/sdk/go && go vet ./...` 通过；`go test -run TestContractDiscoveryAndJWKS -count=1` 通过。

## T14: 重写契约测试

**文件：** `examples/sdk/go/fixtures_test.go`、`examples/sdk/go/sdk_test.go`
**依赖：** T13
**步骤：**
1. `fixtures_test.go` 增加：
   - 授权码流程（固定 State/Nonce；断言 S256；用规范 verifier 交换；校验 ID Token nonce 与 access token aud/token_use）；
   - refresh；
   - 端点错误（错 PKCE 用 SDK，其余用原始 HTTP）；
   - 负向 token 表驱动 + 原因类别映射；
   - revoke 与 logout 正负向。
2. `sdk_test.go` 删除 `testSigner`、`fakeIssuer`、`setAuthorization`、`logoutToken`，保留 demo backchannel replay 测试并改用 fixtures。

**验证：** `go test -count=1 ./...` 通过；`grep -n "rsa.GenerateKey\|time.Now()" examples/sdk/go/sdk_test.go` 无匹配。

## T15: 文档与最终校验

**文件：** `examples/sdk/go/README.md`
**依赖：** T14
**步骤：**
1. 在离线测试一节补充语料位置、`YUFU_CONTRACT_FIXTURES`、固定端口与时钟要求。
2. 执行最终校验：
   - `jq . sdk/contract-fixtures/*.json`；
   - `cd sdk/contract-fixtures/tools/gen && go test -count=1 ./...`；
   - `cd examples/sdk/go && go test -count=1 ./...`；
   - 再运行生成器后 `git status --short sdk/contract-fixtures` 无变化。

**验证：** 四条命令全部成功，生成物无 diff。

## 执行顺序

```
T1 → T2
T1 → T3 → T4
T1 → T5
T3 → T6 → T7
T3,T4 → T8
T5,T6 → T9 → T10 → T11
T10 → T12
T10 → T13 → T14 → T15
```