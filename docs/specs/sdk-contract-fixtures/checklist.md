# SDK 契约 Fixture 语料 Checklist

> 状态：待审批
> 每一项通过运行命令或观察行为验证，不依赖逐行读实现。

## 语料完整性

- [ ] `manifest.json` 可解析且所有引用文件存在（验证：`jq . sdk/contract-fixtures/manifest.json` 成功；加载器自校验通过）
- [ ] 14 个 claims 源全部为合法 JSON 且 id 唯一（验证：`jq` 逐文件解析；生成器启动时不报重复 id）
- [ ] 生成的 `.jwt` 数量与 claims 数量一致（验证：`find sdk/contract-fixtures/tokens -name '*.jwt' | wc -l` 为 14）
- [ ] JWKS 的 `kid` 与 claims header 的 `kid` 对应（验证：`jq -r '.keys[0].kid' jwks.json` 为 `key-1`）
- [ ] Discovery 的 `issuer` 与 manifest 一致，两个 backchannel 能力为 `true`（验证：`jq` 比较）
- [ ] 端点行为中的 `body_file` 全部存在（验证：`jq` 提取路径后逐一 `test -f`）
- [ ] 响应体中的 JWT 与 `tokens/` 中的对应文件逐字一致（验证：生成器再生成比对）

## 生成确定性

- [ ] 再生成不产生 diff（验证：`go test -count=1 ./...` 在 `tools/gen` 下通过；手动运行时 `git status --short sdk/contract-fixtures` 为空）
- [ ] 篡改任一 claim 后一致性测试失败（验证：临时改一个 `exp`，运行 `tools/gen` 测试应失败；恢复后通过）
- [ ] 两把 TEST-ONLY 密钥可被 OpenSSL 校验（验证：`openssl rsa -in <key> -check -noout`）

## 参考消费者

- [ ] `TestContractDiscoveryAndJWKS` 通过（验证：`go test -run TestContractDiscoveryAndJWKS -count=1`）
- [ ] 授权码流程通过：固定 State/Nonce、S256 关系成立、ID Token 的 `nonce` 匹配、访问令牌 `aud/token_use` 正确（验证：`go test -run TestContractAuthorizationCodeFlow -count=1`）
- [ ] refresh 流程通过（验证：`go test -run TestContractRefresh -count=1`）
- [ ] 端点错误通过：错 PKCE 返回 `invalid_grant`、不支持的 grant 返回 `unsupported_grant_type`、UserInfo 缺 Bearer 返回 401、Introspection 错凭据返回 401（验证：`go test -run TestContractEndpointErrors -count=1`）
- [ ] 负向访问令牌通过：1 条接受 + 6 条拒绝且原因类别匹配（验证：`go test -run TestContractNegativeTokens -count=1`）
- [ ] revoke 与 logout 通过：revoke 幂等、合法 logout 接受、4 类负向 logout 拒绝（验证：`go test -run TestContractRevokeAndLogout -count=1`）
- [ ] demo 的 backchannel replay 幂等（验证：对应测试两次 POST 均返回 200）
- [ ] 离线测试不再现场造数（验证：`grep -n "rsa.GenerateKey\|time.Now()" examples/sdk/go/sdk_test.go` 无匹配）

## 集成

- [ ] fixture server 监听地址与 manifest 的 issuer 完全一致，SDK 无需改写 issuer（验证：Discovery 校验通过；测试中未出现 host 替换）
- [ ] 所有 SDK client 注入固定时钟（验证：负向过期向量被拒绝、正向向量被接受同时成立）
- [ ] `YUFU_CONTRACT_FIXTURES` 可覆盖语料路径（验证：把语料复制到临时目录，带环境变量运行测试通过）
- [ ] 端口被占用时给出明确失败信息（验证：先监听 18765，再运行测试，错误信息包含端口）
- [ ] 新语言实现者仅凭 `README.md` 能确定加载顺序、固定 issuer、测试时钟与拒绝原因类别（验证：按 README 命令复跑一次）

## 编译与静态检查

- [ ] 生成器模块 `go vet ./...` 通过
- [ ] 示例模块 `go vet ./...` 通过
- [ ] 两个模块均未新增第三方依赖（验证：`go.mod` 只含标准库或原有依赖）
- [ ] 所有 JSON 可被 `jq` 解析（验证：`for f in sdk/contract-fixtures/**/*.json; do jq . "$f" >/dev/null; done`）

## 端到端场景

- [ ] 场景 1（正常消费）：干净工作区 → `cd examples/sdk/go && go test -count=1 ./...` 全绿 → `cd sdk/contract-fixtures/tools/gen && go run . -root ../..` → `git status` 无变化
- [ ] 场景 2（时钟确实生效）：把 manifest 的 `clock` 改到所有正向 token 过期之后 → 正向向量测试失败；恢复后全绿
- [ ] 场景 3（自校验生效）：删除一个被 `expectations.json` 引用的 token 文件 → 加载器明确报错指出缺失文件；恢复后全绿

## Spec 对齐

| 验收标准 | 对应检查 |
|---|---|
| AC1 | 语料完整性第 1、2、6 项 |
| AC2 | 参考消费者第 1 项 |
| AC3 | 参考消费者第 2 项 |
| AC4 | 参考消费者第 3 项 |
| AC5 | 参考消费者第 4 项 |
| AC6 | 参考消费者第 6 项 |
| AC7 | 参考消费者第 5 项 |
| AC8 | 生成确定性第 1 项 |
| AC9 | 参考消费者第 8 项、集成第 2 项 |
| AC10 | 集成第 5 项 |