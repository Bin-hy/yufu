# 网关安全整改执行证据

## 环境与 T01

- 分支：codex/gateway-oidc-authorization；保留原有两个 assets/.gitkeep 删除及审查文档。
- rustc 1.97.1；cargo 1.97.1；pnpm 10.33.0；protoc 35.1。
- Docker Desktop 4.68.0，Engine 29.3.1；授权访问 Docker socket 后 client/server 检查退出码0。
- `cargo metadata --locked --format-version 1`：授权写 Cargo 缓存后退出码0，补齐锁定的 WinDivert 子模块及缺失依赖；未升级 Cargo.lock。
- `pnpm install --offline --frozen-lockfile`：退出码0，6个workspace项目依赖已就绪。
- 基础测试 `cargo test -p auth --lib --locked`：退出码0，1 passed，0 failed，0 ignored。此为原有基础库基线，不代表整改测试或全项目通过。

## T02 测试入口

- `cargo test --test mod --no-run --locked`：退出码0，集成测试入口编译成功。
- `cargo test --test mod security_t02 --locked -- --nocapture`：首次编译暴露 SeaORM 2 的原始 Statement 接口差异，已改用 execute_raw/query_one_raw；重跑退出码0，2 passed、0 failed、0 ignored。
- 已实际验证两个独立 PostgreSQL 容器的数据隔离，以及模拟设备的实际请求计数。
- 补齐可注入时钟夹具后再次执行相同命令：退出码0，3 passed、0 failed、0 ignored、48 filtered out。新增用例用同一夹具验证安全预算与授权新鲜期，包含时间回拨及同步状态失败。T02 已完成。

## T03 身份契约

- 已先新增身份命名空间、非法身份反序列化、秘密值输出三类测试；首轮退出码101，原因是尚未实现的类型未定义，确认测试未空跑。
- 完成 PrincipalKind、PrincipalRef、TokenUse、IdentityBinding、SecretToken 和安全错误分类；身份构造/反序列化校验保持一致，issuer保留精确字符串，不依赖URL规范化合并身份。
- `cargo test -p auth --lib security_t03 --locked`：退出码0，3 passed、0 failed、0 ignored、1 filtered out。
- `cargo clippy -p auth --lib --tests --locked -- -D warnings`：退出码0。
- `cargo fmt --all -- --check`、`git diff --check`：退出码0。
- T03 已完成。共享契约当前尚未替换业务路由，不能据此宣称任何现有鉴权漏洞已修复。

## T04–T06 资源、预算与授权有效期

- 先新增T04–T06行为测试，再运行 `cargo test -p auth --lib security_t0 --locked`：首轮退出码101，缺少待实现的类型，日志保存在本轮临时测试日志中；实现后退出码0，14 passed、0 failed、0 ignored、1 filtered out。
- T04：ResourceRef 按资源种类要求完整字段，反序列化执行相同校验；规范标识使用resource:v1结构化编码，网关/设备中的分隔符不能混淆目标。授权范围必须显式携带应用和资源选择器，空授权集合是deny-all。
- T05：新增可信时钟接口和SecurityBudget。排队及子调用共享原始单调截止期；外部剩余时间只能缩短预算；实际异步等待超时测试使用暂停的Tokio时钟，无需等待真实2秒。
- T06：源头新鲜期最多5秒，与凭据、父凭据及已继承期限取最小值；预留1秒时钟误差，无过期宽限。本地缓存绑定后还受固定单调截止期约束，时间回拨不恢复已过期结论。
- `cargo test -p auth --lib --locked`：退出码0，15 passed、0 failed、0 ignored、0 filtered out，包含原有基础库测试。
- `cargo clippy -p auth --lib --tests --locked -- -D warnings`：退出码0。
- `cargo fmt --all -- --check`、`git diff --check`：退出码0。
- T04、T05、T06 已完成。当前只完成共享安全契约；服务路由尚未接入，跨实例10秒撤权和完整业务链2秒预算仍未验收。

## 验收状态

checklist.md 全部条目保持未验证。只在取得对应实际证据后勾选，不用本文件中的环境检查替代端到端验收。

## T07 严格 JWT 验证

- 先新增负向测试再实现：首轮 `cargo test -p auth --lib security_t07 --locked` 退出码101，原因是待实现的验证接口及类型缺失。
- 新增 SecurityAccessClaims、CredentialExpectation、CredentialProfile、CredentialRef 与不可由客户端反序列化构造的 VerifiedCredential。验签结果保留签名 key ID，但不等同于凭据登记状态或业务授权。
- 严格验证 RS256、at+jwt、已配置可信 kid、精确 issuer/单受众/application、主体与用途组合、会话/设备绑定、到期/签发时间/nbf、必需字段及时间溢出。拒绝令牌自带的密钥URL/密钥/证书以及过大的令牌。
- 新访问凭据类型不为缺少用途/范围/绑定的旧令牌补默认权限；旧签发链的迁移仍在后续任务中。
- `cargo test -p auth --lib security_t07 --locked`：退出码0，5 passed、0 failed、0 ignored；包含合法用户、机器、设备与MCP类型的正向路径。

## T08 认证上下文及 Bearer 提取

- 先新增上下文/路由负向测试：首轮 `cargo test -p auth --lib security_t08 --locked` 退出码101，原因是待实现状态接口和上下文缺失。
- 新 StrictAccessState 必须注入 CurrentCredentialChecker，签名通过后核对可信登记快照及其新鲜度；签名claims与登记claims不同、已吊销、过期或依赖超时均不产生 AuthenticatedContext。
- 上下文秘密证据使用脱敏类型，无客户端反序列化入口；中间件先移除旧上下文，只从唯一Authorization头提取Bearer。
- 两个现有RS256中间件也已改为共用Bearer提取，不再接受query access_token回退；旧HS256通用回调接口不在本次替换范围。浏览器流连接的受限票据迁移需后续完成，不能用此变化宣称全部旧入口已迁移。
- `cargo test -p auth --lib security_t08 --locked`：退出码0，4 passed、0 failed、0 ignored；验证query及预置上下文不绕过认证、状态匹配/新鲜度、2秒超时，以及现有入口Bearer正常路径。
- 状态检查当前由隔离测试适配器验证接口契约；生产仓储及各服务的严格状态接入尚未完成，仍需T18、T24及相应服务组装任务。
- 最终回归：`cargo test -p auth --lib --locked` 退出码0，24 passed、0 failed、0 ignored；`cargo test --test mod --locked -- --test-threads=4` 退出码0，51 passed、0 failed、0 ignored。
- `cargo clippy -p auth --lib --tests --locked -- -D warnings`、`cargo fmt --all -- --check` 和 `git diff --check` 均退出码0。
- T07、T08 组件任务已完成；完整workspace所有feature、实际生产状态仓储及端到端安全清单仍未验收。

## T09 签名消息契约（2026-11-08）

- 先新增签名/篡改测试再实现；首轮 `cargo test -p auth --lib security_t09 --locked` 退出码101，待实现类型未定义。实现时补启用UUID的serde feature，未变更Cargo.lock。
- 新增独立的操作许可与网关事件JWT类型，仅支持RS256和可信配置中的kid。访问凭据、许可、结果不能互换；不接受HMAC回退或令牌提供的密钥URL。统一JOSE头解析只接受alg/typ/kid，拒绝未知crit/b64等扩展。
- CommandEnvelope携带不可变原始载荷字节，先检查SHA-256摘要再解析，校验请求/任务、操作、资源、能力版本和目标绑定版本与许可一致。输入、头、超时及工具名都在原始摘要覆盖范围内。
- 覆盖合法命令与结果、过期/错误受众/错误网关、原始载荷篡改（包括只添加空白）、签名许可与载荷元数据冲突、超长载荷，以及非整秒授权查询时间的边界。结果带task/claim/执行实例/digest，设备事件带绑定版本/流ID/序号。
- 签名证明不等于执行授权；当前父凭据状态、任务claim、注册公钥状态及事件去重仍由后续任务接入。

## T10 统一脱敏（2026-11-08）

- 先新增日志捕获测试，首轮退出码101（接口未实现），实现后两项测试通过。
- URI移除authority及全部query内容；请求头只保留白名单传输元数据；所有路由默认省略请求/响应body；配置所有标量值脱敏；连接URL去掉用户信息、路径、query及fragment。SignedDocument和PayloadBytes的Debug不暴露凭据或内容。
- tracing捕获断言覆盖凭据标记、编码query参数名、无等号query、未知头、配置嵌套/数字值和数据库URL，确认实际日志输出不包含秘密标记。
- 这里只提供统一工具，现有各服务日志接入及真实签发/同步/启动日志的回归仍需T65，不能据此声明全链日志泄露已修复。
- 最终回归：`cargo test -p auth --lib --locked` 退出码0，32 passed、0 failed、0 ignored；`cargo test --test mod --locked -- --test-threads=4` 退出码0，51 passed、0 failed、0 ignored。
- `cargo clippy -p auth --lib --tests --locked -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check` 均退出码0。
- T09、T10组件任务已完成（累计10/80）；端到端checklist继续保持未验证。
