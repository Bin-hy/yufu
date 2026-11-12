# OIDC subject 字符串类型修复 Tasks

依据已批准的 spec.md 和 plan.md。每项只修改本次修复相关内容，保留工作区其他未提交变更。

## 文件清单

| 操作 | 文件 | 职责 |
|---|---|---|
| 修改 | `libs/auth/src/claims.rs` | ID Token subject 协议类型 |
| 修改 | `libs/auth/src/token_manager.rs` | 签发入口和单元测试 |
| 按需修改 | `libs/auth/Cargo.toml` | 仅测试密钥生成需要且现有依赖无法满足时增加测试依赖 |
| 修改 | `yufu/src/service/oidc.rs` | 授权码与刷新调用方转换 |
| 修改 | `yufu/src/route/oidc.rs` | UserInfo 字符串输出 |
| 修改 | `tests/yufu/routes/oidc.rs` | 完整授权与刷新回归测试 |
| 修改 | `cobotimx/src/auth/oidc.ts` | UserInfo 接口类型 |
| 新建 | `cobotimx/src/stores/auth.test.ts` | 字符串 subject 的 store 回归测试 |

## T1：补充 ID Token 签发回归测试

**文件：** `libs/auth/src/token_manager.rs`，按需 `libs/auth/Cargo.toml`
**依赖：** 无
**步骤：**
1. 在已有测试模块中增加 TokenManager HMAC、IdTokenSigner HMAC、IdTokenSigner RSA 三个测试。
2. 修复前传入整数 123，校验签名后以 JSON 载荷断言 `sub` 等于字符串 `"123"`，验证测试能够复现现有缺陷。
3. RSA 使用测试生成的密钥，不读取部署密钥；验证算法、issuer 和 audience。

**验证：** `cargo test -p auth id_token`，记录修复前数字与字符串不相等的失败证据；若环境在编译阶段阻塞，记录阻塞而非宣称已复现。

## T2：收紧协议结构与签发接口

**文件：** `libs/auth/src/claims.rs`、`libs/auth/src/token_manager.rs`、`yufu/src/service/oidc.rs`
**依赖：** T1
**步骤：**
1. 去除 `IdTokenClaims` 的 ID 泛型，将 `sub` 固定为 `String`。
2. 两个 ID Token 签发入口取消 ID 泛型，改用 `subject: &str`，通过 `to_owned()` 构造载荷。
3. 授权码兑换与刷新调用处去除泛型参数，传入整数 ID 的 `to_string()` 结果。
4. T1 测试改为通过新字符串接口传入 subject，保留载荷类型及签名断言。
5. 更新受影响注释；不修改 Access Token 和内部刷新令牌类型。

**验证：** `cargo test -p auth`、`cargo check -p yufu`，预期签发测试通过且调用方编译通过。

## T3：修复 UserInfo 输出

**文件：** `yufu/src/route/oidc.rs`
**依赖：** T2
**步骤：**
1. 将 `UserInfoResp.sub` 改为 `String`。
2. 在响应构造处使用 `user_vo.id.to_string()`，保持其他字段行为。

**验证：** `cargo check -p yufu`；通过 T4 的 HTTP 测试确认响应实际输出字符串。

## T4：扩展授权码与刷新集成场景

**文件：** `tests/yufu/routes/oidc.rs`
**依赖：** T2、T3
**步骤：**
1. 在现有测试辅助方法和场景中复用已知登录用户 ID，按十进制字符串计算预期 subject。
2. 授权码兑换后解码 ID Token 载荷，断言 `sub` 为预期字符串。
3. 使用对应 Access Token 请求 `/api/v1/oidc/userinfo`，断言 subject 与 ID Token 完全一致。
4. 刷新后对新 ID Token、新 Access Token 的 UserInfo 执行同样断言，并与刷新前比较。
5. 保留已有刷新令牌轮换、撤销和错误路径验证。

**验证：** `cargo test --test mod test_oidc_authorization_code_pkce_exchange -- --nocapture`，预期授权码、UserInfo、刷新及撤销流程全部通过。PostgreSQL 测试容器不可用时，明确报告阻塞。

## T5：收紧前端类型并验证业务转换

**文件：** `cobotimx/src/auth/oidc.ts`、`cobotimx/src/stores/auth.test.ts`
**依赖：** T3
**步骤：**
1. 参照已有 Vitest 配置，在独立 Pinia 实例中测试真实 auth store。
2. 模拟 UserInfo 返回 `sub: "123"`，调用 `loadUserInfo(true)`，断言 `userId` 为数字 123。
3. 将 `UserInfo.sub` 改为 `string`，保持 store 现有数字转换逻辑。

**验证：** 在 `cobotimx` 目录运行 `npm test`、`npx vue-tsc -b`，预期测试和类型检查通过。

## T6：完成检查并记录证据

**文件：** 本次修改文件、`docs/specs/oidc-sub-string/checklist.md`
**依赖：** T2、T3、T4、T5
**步骤：**
1. 执行尚未完成的相关测试和格式检查 `cargo fmt --check`；只格式化本次涉及的代码。
2. 审阅本次差异，确认没有数据库迁移、用户身份映射改变或无关代码修改。
3. 在 checklist.md 记录实际命令、结果和未通过原因；不将未执行项标记为通过。
4. 汇报修复行为、验证结果以及旧 ID Token 需刷新或重新登录的兼容性说明。

**验证：** 本次变更差异符合已批准范围；检查记录可对应 spec.md 的 AC1–AC5，且每项状态都有实际证据。

## 执行顺序

T1 → T2 → T3 → T4 → T5 → T6。

任务执行前仍需完成并批准验收清单。不会自动提交工作区其他变更。

## 执行结果

T1–T6 已完成。实际集成测试 target 经 Cargo 元数据确认是 `mod`，已修正命令。前端通过公开的 `completeSignIn()` 触发私有的 `loadUserInfo` 并等待结果。验证结果和文档示例测试失败详情见 checklist.md。
