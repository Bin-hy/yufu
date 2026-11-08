# OIDC subject 字符串类型修复 Checklist

依据已批准的 spec.md、plan.md 和 task.md。已完成实现与相关验证，实际结果见文末。未通过的文档示例测试保留未勾选状态。

## 实现与协议行为

- [x] AC1：TokenManager 的 HMAC ID Token 通过签名校验，解码后的 `sub` 为 JSON 字符串 `"123"`。（验证：auth 对应单元测试）
- [x] AC1：IdTokenSigner 的 HMAC ID Token 通过签名校验，解码后的 `sub` 为 JSON 字符串 `"123"`。（验证：auth 对应单元测试）
- [x] AC1：IdTokenSigner 的 RSA ID Token 使用测试密钥签发并通过签名校验，解码后的 `sub` 为 JSON 字符串 `"123"`。（验证：auth 对应单元测试）
- [x] AC3：已知整数用户 ID 的协议输出为原 ID 的十进制字符串，无前缀或身份映射改变。（验证：单元测试及 OIDC 集成测试中对已知 ID 的断言）
- [x] N3：协议载荷与两个 ID Token 签发入口均要求字符串，整数不能直接作为 subject 传入。（验证：审阅类型和入口签名，并编译项目内调用方）

## 集成与端到端

- [x] AC2：现有用户授权并完成授权码兑换后，ID Token 内的 `sub` 是预期字符串。（验证：扩展现有 OIDC HTTP 集成测试）
- [x] AC2：使用本次兑换得到的 Access Token 查询 UserInfo，返回字符串 `sub`，与 ID Token 严格相等。（验证：同一集成测试）
- [x] AC2：使用现有 Refresh Token 刷新后，新 ID Token 和新 Access Token 对应的 UserInfo 均返回相同字符串 subject，且与刷新前一致。（验证：同一集成测试）
- [x] AC5、N2：刷新令牌轮换、撤销及撤销后拒绝刷新行为仍然通过。（验证：保留并运行现有 OIDC 集成场景）
- [x] AC4：前端加载 `sub: "123"` 的 UserInfo 后，业务 store 中的本地用户 ID 为数字 123。（验证：auth store 回归测试）

## 编译与测试

- [ ] `cargo test -p auth` 全部通过：45 个单元测试通过（包含 AC1 的 RSA、HMAC 测试），但 8 个原有文档示例测试编译失败，1 个忽略。
- [x] AC5：`cargo check -p cobotusr` 通过，所有受影响 ID Token 签发调用方编译成功。
- [x] AC2、AC5：`cargo test --test mod test_oidc_authorization_code_pkce_exchange -- --nocapture` 通过；记录 PostgreSQL 测试容器是否可用。
- [x] AC4：在 `cobotimx` 目录运行 `npm test` 通过，包含新增 store 测试。
- [x] AC4：在 `cobotimx` 目录运行 `npx vue-tsc -b` 通过，UserInfo 的字符串类型与使用方一致。
- [x] `cargo fmt --check` 通过；若存在其他工作区变更导致的既有问题，单独记录，并核实本次涉及文件格式正确。

## 兼容性与范围

- [x] N1：本次差异不包含数据库迁移，数据库及内部用户 ID 类型保持不变。（验证：审阅最终差异）
- [x] N2：本次差异未改变 Access Token、令牌内省及内部旧登录刷新令牌的协议类型或处理逻辑。（验证：审阅最终差异及相关回归结果）
- [x] N4：交付说明写明旧 ID Token 需刷新或重新登录获取新类型；依赖数字 subject 的外部客户端需要调整。
- [x] 工作区中其他未提交变更保留，最终说明明确本次修复文件及实际未通过或未执行的检查。（验证：审阅最终差异和验证记录）

## 验证记录

- 缺陷复现：修复前运行 `cargo test -p auth id_token`，3 个新增测试均在签名验证成功后失败，实际 `Number(123)`，预期 `String("123")`。
- 修复后 `cargo test -p auth`：45 个单元测试全部通过。文档示例测试 8 个失败、1 个忽略；错误涉及原有示例引用私有的 `auth::TokenManager` / `auth::Claims`、缺少示例依赖、过时接口和运行时 feature。此次未扩大范围修改这些示例。
- `cargo check -p cobotusr`：通过。
- `cargo test --test mod test_oidc_authorization_code_pkce_exchange -- --nocapture`：1 个测试通过（70 个过滤），覆盖 confidential/public 授权码兑换、UserInfo、刷新后 subject，以及原有轮换、撤销与错误路径。
- 集成测试第一次在沙箱内无法连接 Docker socket，确认是权限限制后，经工具审批在沙箱外重跑通过，实际使用本机 Docker 的临时 PostgreSQL 容器。
- `npm test`（cobotimx）：2 个测试文件、11 个测试通过。新增 store 测试通过 `completeSignIn()` 触发实际的后台 UserInfo 加载（`loadUserInfo` 是私有函数），验证 `"123"` 转换为数字 123。
- `npx vue-tsc -b`（cobotimx）：通过；还原此次检查产生的 tsconfig.tsbuildinfo 缓存差异。
- `cargo fmt --check`、`git diff --check`：通过。
- 最终差异审阅：仅修复 ID Token、UserInfo、前端协议类型及相关测试；数据库结构、Access Token、内省与旧登录刷新令牌未作修改。其他既有工作区修改保留。
- 兼容性：已签发的数字 subject ID Token 保持原样；刷新或重新登录后获得字符串 subject。依赖数字 subject 的外部客户端需要同步调整。
- 未运行 workspace 全套测试、全量 clippy 或审计；此次验证范围为已批准任务清单中的相关测试、编译与格式检查。
