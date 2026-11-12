# OIDC subject 字符串类型修复 Plan

## 输入与范围

依据已获用户批准的 `spec.md`，修复协议类型和对应测试，不调整鉴权策略、数据库结构或用户标识映射。

## 架构概览

`auth` 库负责声明 ID Token 协议载荷并签名；`yufu` 在业务 ID 进入协议层时完成整数到十进制字符串的转换；`cobotimx` 按字符串接收 UserInfo，在业务 store 中保留现有数字转换。

## 核心数据结构与接口

- `IdTokenClaims` 去除 `IDTYPE` 泛型和相关约束，`sub` 固定为 `String`，其余字段保持原定义。
- `TokenManager::generate_id_token` 去除 ID 泛型，参数 `subject: &str`，构造载荷时使用 `subject.to_owned()`；其余参数及 `JwtResult<String>` 返回值保持不变。
- `IdTokenSigner::sign_id_token` 同样使用 `subject: &str`；RSA 和 HMAC 分支共享同一字符串载荷。
- `UserInfoResp.sub: String`；响应构造使用 `user_vo.id.to_string()`。
- TypeScript `UserInfo.sub: string`；store 继续通过 `Number(user.sub)` 获取内部数字 ID。

## 模块交互

1. 授权码兑换读取 `entry.user_id`，通过 `to_string()` 传入 ID Token 签发接口。
2. 刷新流程读取 `rec.user_id`，执行同样转换并签发新的 ID Token。
3. UserInfo 仍使用 Access Token 的字符串 subject 解析和查询用户，再将用户 ID 按相同规则输出。
4. 前端按字符串协议类型加载用户信息，进入现有数字 ID 业务逻辑。

## 文件组织与需求归属

| 文件 | 改动 | 需求 |
|---|---|---|
| `libs/auth/src/claims.rs` | 固定 ID Token subject 类型 | F1、N3 |
| `libs/auth/src/token_manager.rs` | 收紧两个签发入口，补充签名验证单元测试 | F1、N3 |
| `yufu/src/service/oidc.rs` | 授权码与刷新调用方显式转换 | F1、F3 |
| `yufu/src/route/oidc.rs` | UserInfo 类型与响应转换 | F2、F3 |
| `tests/yufu/routes/oidc.rs` | 扩展现有授权码、UserInfo、刷新集成场景 | F1、F2、F3、N2 |
| `cobotimx/src/auth/oidc.ts` | 收紧 UserInfo 接口 | F4 |
| `cobotimx/src/stores/auth.test.ts` | 使用现有测试约定，验证字符串 subject 加载为原数字用户 ID | F4 |

## 测试设计

- auth 单元测试分别覆盖 TokenManager HMAC、IdTokenSigner HMAC、IdTokenSigner RSA。使用测试专用密钥并校验签名，将载荷解码为 JSON，断言 `sub` 等于字符串 `"123"`，避免将 `as_str()` 检查遗漏为仅检查整个 token 类型。
- RSA 使用测试生成的密钥，不读取部署私钥。复用已有依赖和测试辅助能力，必要时仅添加测试依赖。
- 扩展已有 OIDC 集成测试：授权码兑换得到 ID Token 后，以对应 Access Token 查询 UserInfo；刷新后再次查询，断言四处 subject 均为字符串、与已知用户 ID 一致；保留刷新轮换和撤销验证。集成测试解析 JWT 载荷用于断言，签名验证由 auth 单元测试覆盖。
- 前端使用字符串 subject 的 UserInfo 响应驱动真实 store 加载方法，断言本地用户 ID 为数字 123。
- 执行 auth 测试、受影响 crate 编译、OIDC 集成测试、前端测试和类型检查、格式检查。集成测试沿用项目 PostgreSQL 测试容器；若环境缺少依赖，记录实际阻塞，不将未执行的测试标记为通过。

## 技术决策

| 决策 | 理由 |
|---|---|
| 协议结构和入口均使用字符串 | 从编译期阻止数字 subject 直接进入 ID Token |
| 转换留在业务边界 | 保持数据库和内部 ID 兼容，统一十进制表示 |
| 不使用仅序列化时转换的泛型方案 | 保持 Rust 类型声明与协议真实类型一致 |
| 不引入新标识或数据库迁移 | 仅修正类型，避免改变现有用户身份对应关系 |
| 旧 ID Token 通过刷新或重新登录更新 | 已签名令牌不能原地修改；现有刷新令牌继续可用 |

## 兼容性与工作区约束

这是共享 auth 库签发 API 的源码级变更，更新项目内所有调用点并编译验证。外部调用者需要传入字符串。部署后新签发令牌与 UserInfo 使用字符串，旧 ID Token 数字载荷保持原样，依赖旧类型的外部客户端需调整并重新获取令牌。实施时仅修改上述相关文件，保留工作区中其他未提交变更。
