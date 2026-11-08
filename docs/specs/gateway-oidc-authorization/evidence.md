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

## T11–T12 纯 RBAC/ABAC 判定（2026-11-08）

- 先新增重叠allow/deny、当前角色/上界、所有者和版本约束测试；首轮 `cargo test -p auth --lib security_t11 --locked` 退出码101，待实现Policy类型未定义。
- 实现可信PolicySnapshot与纯evaluate_policy；要求已登记操作、应用一致、凭据上界和系统资源约束同时满足。扫描所有匹配拒绝后才返回主体允许或角色允许，最后默认拒绝；没有超级管理员跳过分支。
- 新增10项T11/T12测试：规则排列及宽拒绝/窄允许、无关拒绝、用户允许优先级、角色新增/撤销、旧凭据范围不扩张、未知操作、应用/网关/设备/种类隔离、完整身份命名空间、所有者/能力/目标绑定版本，以及缺少条件拒绝所需事实时失败关闭。
- PolicySnapshot和资源事实没有请求反序列化入口。当前凭据/根状态与一致快照加载仍需生产仓储及适配器；任务读取的强制所有者约束由资源适配器提供。未将共享判定器完成等同于现有路由已修复。
- `cargo test -p auth --lib --locked`：退出码0，42 passed、0 failed、0 ignored；`cargo clippy -p auth --lib --tests --locked -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check` 均退出码0。
- 额外运行 `cargo test -p auth --doc --locked`：退出码101，0 passed、8 failed、1 ignored。失败来自现有示例中的私有根导入、my_crate占位、缺失泛型/字段/分号及未声明sea_orm依赖；这些示例未被本组修改。完整质量门禁仍未通过，后续T79需修正，不能记为通过。
- T11、T12组件任务已完成（累计12/80）；实际IdP URI/application权限加载与路由接入仍在后续任务中。

## T13–T14 IdP 身份与绑定迁移（2026-11-08）

- T13先新增测试，首次退出码101，迁移模块未公开；补注册身份迁移及公开Migrator测试入口后，测试实际暴露down路径先执行旧不可逆种子的错误。检查锁定SeaORM实现确认它保留注册顺序，修正原有错误注释并将新迁移放在旧schema/seed之后，未重排历史迁移。
- 新增grant/session/refresh_family/credential表。复合外键约束issuer/主体/应用与稳定根一致；凭据(issuer,jti)唯一；只存32字节refresh摘要并唯一约束，禁止refresh无族、access无签名kid、自引用parent/replacement及非法时间状态。根使用RESTRICT，不级联删除撤销事实。
- T14先运行负向用例：退出码101，security_binding不存在。补机器用途/应用/受众授权绑定、设备身份到具体目标绑定和逐网关公钥表及实体。默认enabled=false；重复身份/设备目标、重复kid和公钥指纹被数据库拒绝。公钥解析及canonical DER指纹计算属于后续可信登记服务，数据库列本身不替代密码学验证。
- `cargo test --test mod cobotusr::routes::security::security_t1 --locked -- --nocapture`：退出码0，3 passed、0 failed、0 ignored；包括空库up、down/up、跨issuer同jti合法及同issuer重复拒绝、错误主体根关系拒绝、实体往返和禁用状态持久化。
- `cargo test --test mod --locked -- --test-threads=4`：退出码0，54 passed、0 failed、0 ignored。
- `cargo clippy -p cobotusr --lib --locked -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check`：退出码0。依赖proc-macro-error2仍有工具链future-incompat提示，未把该提示算成本次lint失败。
- T13、T14已完成（累计14/80）。当前只是持久化基础；旧令牌签发和业务路由尚未迁移到新仓储。

## T15–T16 一次许可、审计与身份实体（2026-11-08）

- T15先补测试，首轮退出码101，两项用例均实际报告security_permit不存在。新增第二项迁移：一次许可记录与结构化审计；许可按(application,request_id)及非空(application,task_id)唯一，外键绑定原凭据的issuer/grant/application，绝对有效期不能超过查询新鲜期。审计只提供结构化主体、资源、操作、原因和关联ID列，没有自由秘密载荷列。
- `cargo test --test mod security_t15 --locked -- --nocapture`：退出码0，2 passed、0 failed、0 ignored；验证同任务新request/jti冲突、非任务请求去重、错误父/期限拒绝，以及许可和审计在同一数据库事务提交或回滚。实际业务签发/审计事务接入仍需T30/T64。
- T16先补实体往返测试，首轮退出码101，四项实体尚未注册。新增credential/grant/session/refresh_family实体并注册；不提供请求反序列化入口，credential不存访问令牌或refresh明文。
- 往返用例验证：消耗旧refresh后替代记录保留同一稳定grant/session/family；minted_by审计来源不自动成为revocation_parent，只有委托明确指向父凭据。该测试验证持久化表示，不替代后续生命周期有效状态测试。
- `cargo test --test mod cobotusr::routes::security::security_t1 --locked -- --nocapture`：退出码0，6 passed、0 failed、0 ignored；包括再次运行两个新增迁移的down/up。
- `cargo test --test mod --locked -- --test-threads=4`：退出码0，57 passed、0 failed、0 ignored。
- `cargo clippy -p cobotusr --lib --locked -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check`：退出码0。
- T15、T16已完成（累计16/80）；主库仓储、根状态判定及生产路由接入尚未实现，完整安全链和checklist仍未验收。
- 额外验证集成测试目标：`cargo clippy --test mod --locked -- -D warnings` 退出码0；覆盖本组新增的实际PostgreSQL用例，不等于全workspace/all-features门禁通过。

## T17 主库安全仓储（2026-11-08）

- 先新增仓储测试，首轮 `cargo test --test mod security_t17 --no-run --locked` 退出码101，security仓储模块尚未定义。实现时修正测试时钟的跨测试模块可见性，并处理SeaORM生成ModelEx的Debug要求；许可值改用脱敏数据库值类型，Model与ActiveModel的Debug均不显示签名原文。
- 定义SecurityRepository/SecurityTransaction及PostgreSQL适配器；事务以REPEATABLE READ/READ WRITE启动，查询pg_is_in_recovery和transaction_read_only，拒绝备用/只读节点。主库连接由调用方配置注入；全部凭据、grant/session/family、父记录和身份绑定都在同一事务读取，没有副本或缓存回退。
- 在获取连接前记录checked_at；SQL等待和事务结束使用继承预算，设置本地statement_timeout。刷新查找只将SHA-256摘要作为SQL参数，保留已消费记录供后续重放检测。
- 机器绑定精确匹配issuer/主体/用途/application/audience，解析显式操作范围；设备绑定精确匹配主体/application；公钥精确匹配issuer/application/gateway/kid、启用状态和有效期，并验证2048–8192位RSA SPKI DER指纹。许可按application/request查询并校验结构，不能作为签名或执行证明。
- 新增3项T17真实PostgreSQL用例，覆盖未知/禁用绑定、跨issuer摘要查找、空范围、消费记录保留、许可与ActiveModel秘密输出、设备跨应用隔离、一致快照、公钥作用域/有效期/指纹，以及下一事务看到已提交禁用。主库保护在独立PostgreSQL实例上检查，未把该测试宣称为物理复制集演练。

## T18 当前凭据与撤销根检查（2026-11-08）

- 先新增生命周期测试，首轮 `cargo test --test mod security_t18 --no-run --locked` 退出码101，lifecycle模块缺失；实现后覆盖合法用户、机器、设备和MCP记录及单层委托。
- CredentialLifecycle加载同一快照的凭据、稳定根与绑定，拒绝未知、非access记录、过期/尚未生效、已撤销/被替代、缺失或错误用途/根/资源绑定。存储格式损坏作为安全依赖错误失败关闭，不补默认授权。
- 单层委托要求父凭据仍活动，主体、应用、grant/session/family一致，子期限不晚于父；拒绝再委托、循环依赖及不匹配目标。minted_by仅为审计信息，不沿其撤销替代凭据。
- 实现CurrentCredentialChecker：真实RS256/JWKS验签后，必须与主库claims及签名kid完全一致才产生可用上下文；同一claims换用另一可信kid、已签名但未知jti、篡改到期值均被拒绝。实际StrictAccessState接入生产仓储的隔离测试通过，但各业务服务组装仍待后续任务。
- 新增6项T18用例，覆盖凭据/grant撤销、会话/族与独立MCP根隔离、父撤销/期限/循环、设备当前归属、真实JWT状态接入及数据库锁/断连/不健康时钟。锁等待使用100ms子预算，实际断言1秒内结束且不返回可用结论；未将该子链测试当作全链2秒验收。
- 源头有效期从查询开始计算，最多5秒并取当前/父凭据最短期限，提交后复核新鲜度。StreamConnect仍失败关闭，受限浏览器票据生命周期尚未接入。
- `cargo test --test mod cobotusr::routes::security::security_t1 --locked -- --test-threads=4`：退出码0，15 passed、0 failed、0 ignored（其中T17/T18共9项）。后续补充公钥时间边界与ActiveModel断言，已由下面完整回归覆盖。
- `cargo test --test mod --locked -- --test-threads=4`：退出码0，66 passed、0 failed、0 ignored。
- T17、T18组件任务已完成（累计18/80）。URI/application权限适配、生命周期写入和业务入口接线仍未完成，checklist保持未验收；原有8项文档示例失败尚待T79修正。
- `cargo test -p auth -p cobotusr --lib --locked`：退出码0，auth 42 passed、cobotusr 22 passed，均0 failed、0 ignored。`cargo clippy -p cobotusr --lib --locked -- -D warnings`、`cargo clippy --test mod --locked -- -D warnings`、`cargo fmt --all -- --check`及`git diff --check`均退出码0；未运行或宣称通过完整workspace/all-features门禁。

## T19 URI 适配与默认拒绝（2026-11-08）

- 先新增用例，首轮 `cargo test --test mod security_t19 --no-run --locked` 退出码101，URI操作映射及授权服务未实现。新增5项T19用例，涵盖用户允许、角色允许、普通用户/超级管理员显式拒绝、未知方法/路由、签名范围上界、管理受众隔离和预算/新鲜期不能重启。
- 新AuthorizationService先复核真实AuthenticatedContext与主库登记及签名kid，再在同一主库事务加载URI权限、用户覆写和当前角色，由共享evaluate_policy统一判定。管理路径只接受配置的management application/audience及UserSession用户；URI操作按登记ID确定，资源同时绑定method与route模板。
- AuthenticatedContext保留认证入口预算和原始有效期，授权调用只可缩短，不能创建新预算或新查询窗口延长原结论。修复API前缀只按完整路径段去除、query一致清理，以及旧公开checker角色查询漏method过滤的问题。
- 删除旧受保护中间件和公开checker的“未登记即放行”。真实HTTP测试先成功访问已登记GET /user，删除其登记后同一超级管理员Bearer得到403；公开checker对未知规则返回allow=false。
- 首轮登记修复用例实际发现旧种子只按URI去重，导致同一路径不同方法缺失。为使默认拒绝可落地，提前完成T34的一部分：增加版本化m20261107_000003_security_operations，补齐既有URI方法，保留已有记录及其管理员授权；只对新登记的个人服务行补标准角色授权，不向User授予管理或网关执行权限。down不删管理员配置，up幂等；新旧库、保留已撤角色权限及T13回滚/up测试通过。T34完整目录/路由覆盖和应用操作种子仍未完成。
- `cargo test --test mod security_t19 --locked -- --test-threads=4`：退出码0，5 passed、0 failed、0 ignored；`cargo test --test mod security_t13_identity_schema_up_down_up --locked`：退出码0，1 passed、0 failed、0 ignored。
- 旧JWT中间件的其余验证/生命周期接线仍在T24/T32；不能将新适配器完成理解为IdP全部入口已切换到严格凭据。

## T20 应用权限、资源边界与旧入口关闭（2026-11-08）

- 先新增应用授权用例，首轮 `cargo test --test mod security_t20 --no-run --locked` 退出码101，应用授权接口缺失；实现后新增5项用例全部通过。
- 同一主库一致快照加载应用操作、全部相关用户覆写和当前角色，统一交给纯判定器；所有匹配拒绝先于任何允许。资源规则采用有限类型ResourceSelector JSON，要求application和kind一致；缺失、裸通配或旧分隔符glob不能补成全部设备，配置损坏以依赖错误失败关闭。长结构化选择器已在实际PostgreSQL往返验证，不做分隔符拼接。
- 最终权限为活动凭据/根、签发上界、当前角色/用户规则、当前机器绑定及系统约束的交集。测试验证跨网关不能借用授权、角色撤销后立即失去当前许可、超级管理员不能盖过显式拒绝；task:read必须有可信owner并属于主体，读取他人任务使用独立task:read-any授权。
- 机器身份只读取对应issuer/主体/用途/app/audience的当前服务绑定，不回退同名用户。测试创建同名超级管理员用户与机器，机器只获当前g1范围，g2拒绝；清空/禁用绑定后分别拒绝授权/拒绝凭据。
- 旧/enforce仅提供自报subject，缺乏可信主体证据和凭据上界，现返回401，不再推断授权。新机器/主体证据协议将由T28/T29接入，云端调用方迁移在T38；部署前必须完成这些任务。旧字符串权限解析仅保留为迁移期间的元数据，不作为新PDP证明。
- `cargo test --test mod security_t20 --locked -- --test-threads=4`：退出码0，5 passed、0 failed、0 ignored；随后补充角色撤销断言，由完整回归覆盖。
- 最终完整集成：`cargo test --test mod --locked -- --test-threads=4` 退出码0，76 passed、0 failed、0 ignored。`cargo test -p auth -p cobotusr --lib --locked` 退出码0，auth 45 passed、cobotusr 22 passed，均0 failed、0 ignored。auth数量包含工作区另一组OIDC sub字符串修复的3项测试，该修复已独立提交为8b01876，未混入本网关权限实现。
- `cargo clippy -p auth -p cobotusr --lib --tests --locked -- -D warnings`、`cargo clippy --test mod --locked -- -D warnings`、`cargo fmt --all -- --check`、`git diff --check`均退出码0。
- T19、T20组件任务完成（累计20/80）；全入口、生命周期写入和端到端撤权仍未验收。之前记录的8项原有文档示例失败尚待T79处理，checklist不提前勾选。
- 最后增加真实HTTP兼容性断言：旧超级管理员Bearer携带自报subject的/enforce请求返回401。`cargo test --test mod security_t19_live_legacy_routes --locked` 退出码0，1 passed、0 failed、0 ignored；集成目标clippy重跑退出码0。

## 2026-11-08：T21–T23 原子兑换、轮换与重放保护

工作目录为 `/Users/xionghaoyi.binhy/BinhyPC/cobot-for`，分支 `codex/gateway-oidc-authorization`，接续基线 `86628ac`。用户给出的旧路径 `/Users/binhy/Binhy-Projects/cobotrix` 在本机不存在；当前仓库的远端为 Bin-hy/cobot-for。开始时工作区干净。四份设计已批准，没有重新审批。仓库及父目录未发现AGENTS.md；按mew-spec和rust-dev-workflow执行，并使用已就绪CodeGraph导航。

环境：macOS Apple Silicon，rustc 1.99.1（2026-10-13）、cargo 1.99.1；Docker colima-binim 中的隔离 PostgreSQL 18-alpine。测试不涉及真实设备。Cargo.lock未变更。

### 先失败后实现的证据

| 命令/阶段 | 实际结果 | 失败行为 |
|---|---|---|
| `cargo test --test mod security_t21 --locked -- --test-threads=4`，接口桩阶段 | 0 passed / 3 failed，退出1 | 合法兑换、并发唯一结果、失败回滚尚未实现；隔离夹具已实际运行 |
| `cargo test --test mod security_t22 --locked -- --test-threads=4`，轮换接口桩阶段 | 0 passed / 4 failed，退出1 | 原子刷新轮换尚未实现 |
| `cargo test --test mod security_t23 --locked -- --test-threads=4`，实现T22后 | 0 passed / 4 failed，退出1 | 旧refresh被拒绝但没有撤销族；提交失败未被识别；实例竞争后仍可能留下活动后代 |
| `cargo test --test mod test_oidc_authorization_code_pkce_exchange --locked`，旧路由 | 0 passed / 1 failed，退出1 | 错误PKCE后再次正确兑换同一码仍返回400，确认提前烧码 |
| `cargo test --test mod security_t21_signing_wait --locked`，同步签名阶段 | 0 passed / 1 failed，退出1 | 600ms签名等待突破继承的100ms预算，并返回成功 |

中间修正了测试夹具缺少用户、重复种子应用、JWKS路径，以及旧T19测试假定操作种子始终为最后一个迁移的问题。这些夹具/编译失败没有算作行为通过。一次增量编译遇到rustc ICE（伴随尚未修正的类型错误），修正类型并执行`cargo clean -p cobotusr`后重编译通过。先前clippy因async_trait生成的重复must_use在Rust 1.99下失败；仅对相关trait/方法注解抑制该宏生成lint，其他警告仍为错误。

### 已实现与实际观察

- T21：先在同一主库事务内认证客户端并校验授权码、redirect、PKCE及到期，再条件消费、签名、登记access/refresh和稳定grant/session/family，提交后才返回。16路并发仅一次成功。错误client/redirect/verifier不烧码；签名、延迟提交触发器或预算失败均不留下凭据及根，随后仍能正确兑换。
- 客户端允许的服务受众来自新的`security_session_client`登记，访问令牌的aud/application为cobotusr或cobotsrv，ID Token保持client_id受众及字符串sub。未登记/停用绑定拒绝兑换；变更登记不能借refresh切换旧会话受众。同受众不同client_id重用他人的已消费refresh不会撤销该族。
- T22：锁定稳定grant→session→family→refresh，在同一事务消费旧refresh、撤销明确配对旧access、登记新配对并建立替代关联。原有非空g1设备权限上界在当前规则改为g2后仍保持g1；缺失/旧glob资源元数据不被提升为权限。旧access到期而refresh有效时仍可轮换。16路并发最多一个替代配对，签名/提交失败回滚。重放竞争可能使唯一替代族被撤销，这是T23的预期防护。
- T23：仅保存refresh摘要和消费历史；重用已消费摘要时，先认证client并验证issuer、client、稳定根归属，再撤销该族和通过明确revocation_parent_jti派生的凭据。已过期祖先仍能终止活动后代。其他会话、其grant/session及仅通过minted_by关联的凭据不受影响。双生命周期实例并发重放/轮换不会恢复活动族；提交失败返回依赖不可用，回滚后合法后代仍可用，随后成功重放防护可重试且保持幂等。
- PostgreSQL序列化/死锁冲突最多重试4次，每次重开主库一致快照，复用原始SecurityBudget；不重新给2秒。生产RSA签名在spawn_blocking中执行，其等待受同一预算限制。时钟不健康、没有监控及HMAC签名均失败关闭并保留授权码。
- 实际OIDC授权码/refresh路由已经接入新服务，移除了`take_authorization_code`、`issue_authorization_code_tokens`、`issue_refresh_tokens`。真实HTTP覆盖公共PKCE、机密客户端secret、错误PKCE后正确重试、字符串sub/UserInfo、refresh与配对撤销、已消费重放。新access由实际JWKS严格验签并与主库登记比对。SecurityError的HTTP转换保留401/403/400/409/503分类。

### 最终运行记录

以下均实际执行，退出0；数量为本轮最终代码的结果：

```text
cargo test --test mod --locked -- --test-threads=4
92 passed; 0 failed; 0 ignored; 0 filtered out

cargo test -p auth -p cobotusr --lib --locked
auth: 45 passed; 0 failed
cobotusr: 23 passed; 0 failed

cargo test --test mod oidc --locked -- --test-threads=4
4 passed; 0 failed; 88 filtered out

cargo clippy -p auth -p cobotusr --lib --tests --locked -- -D warnings
cargo clippy --test mod --locked -- -D warnings
cargo fmt --all -- --check
git diff --check
```

最终92个集成用例包含T21的5个事务/预算/绑定测试及3个HTTP故障测试、T22的4个测试、T23的4个测试；扩展了原有OIDC完整HTTP测试和T19迁移顺序验证。最终全量集成运行包括增强后的显式委托后代与minted_by隔离断言。auth/cobotusr单元测试共68个，新增安全错误HTTP分类回归。

proc-macro-error2的future-incompat提示仍存在，不影响上述退出码。旧证据中的auth文档示例8项失败/1项ignored没有在本轮修正或重新验收。完整workspace/all-features、前端、跨服务10秒撤权、真实时钟健康接线、故障矩阵与完整checklist仍未验收。

### 生产组装与后续任务

新迁移在原安全操作种子之后添加客户端受众表及凭据的oidc_client_id、paired_access_jti。旧记录允许NULL，运行时不能借缺失的配对/客户端事实刷新；没有将旧JWT自动登记为可信。绑定表默认不启用，也不从client_id猜测服务受众。

宿主必须使用`serve_with_security_clock`注入真实同步状态监控，并在维护/配置流程中登记客户端受众。原`serve`没有监控时，新的OIDC兑换和刷新返回503。本轮只用隔离测试中明确声明健康的时钟完成HTTP接线，不把它当成生产同步监控。N1/T70/T71及迁移演练仍待后续完成。

refresh-token hint的现有撤销入口兼容了统一配对登记；完整access/会话/族/应用根撤销、无hint适配、Cookie退出/CSRF、consent撤销及普通API当前状态检查仍待T24/T25/T32及后续接线。不能据本轮组件和OIDC测试推断旧access已被所有业务入口拒绝。

当前完成23/80，T34仍部分完成，下一项T24。本轮只做分组本地提交，不push、不部署、不操作真实设备。checklist保留未验收状态；没有宣称全部漏洞已修复。

### 分组提交与自动扫描说明

- 核心生命周期、存储、迁移与事务测试：`73e01d1`（`feat(auth): add atomic OIDC session exchange rotation and replay protection`）。
- 实际OIDC路由、时钟注入、HTTP分类/测试和本轮文档作为第二组提交；完整提交身份由本分支git log查询。两组代码与上述最终验证时工作区代码相同，后续只更新文档。
- 核心提交后宿主自动触发神医最近commit分析，原始回传9个文件均没有具体display/finds/vulnerability内容。该回传可能不完整，没有作为“安全”或“全部漏洞修复”的证据；本轮没有额外主动扫描或上传。

## T24 本地登录汇入统一生命周期（2026-11-08）

### 测试先行与失败记录

- 先增加5项真实PostgreSQL事务、负向和并发测试，提供待实现接口的失败关闭桩后运行 `cargo test --test mod security_t24 --locked -- --test-threads=4`。首轮退出码101，0 passed、5 failed、0 ignored；夹具已实际启动，登录/注册/刷新桩返回DependencyUnavailable，分别未满足成功、认证失败、冲突和原始预算断言。不是将编译失败作为行为覆盖。首轮记录保存在本机临时日志 `/tmp/cobot-t24-red.log`。
- 后续补3项真实HTTP用例，并将2项既有本地登录/注册测试改为RSA和显式健康测试时钟。中间缺少直接bcrypt测试依赖、authority名称与不合法撤销时间的夹具/编译问题已修正，没有计为行为通过；Cargo.lock未变更。

### 已实现与实际观察

- 新LocalLoginRequest、LocalRegisterRequest和LocalPassword汇入CredentialLifecycle。本地登录/注册固定签发RS256、at+jwt、aud/application=cobotusr、UserSession访问凭据及opaque refresh；登记稳定grant/session/family和明确配对关系，提交后才返回。账户、初始权限上界、凭据与根在同一主库一致事务写入；错误密码/未知用户不产生会话，RSA签名或延迟提交触发器失败同时回滚注册账户、凭据和根，随后相同用户名可重试成功。
- 密码校验与散列使用spawn_blocking，其等待继承入口SecurityBudget；RSA签名及数据库事务继续受原始2秒预算限制，没有新建预算延长窗口。100ms登录子预算和1ms注册子预算实际返回DeadlineExceeded且不留下部分账户或凭据；这些子链断言不等于完整链路2秒验收。密码空格合法，拒绝空密码和超过72字节，72字节边界接受，Debug只显示脱敏标记，不静默截断或自动改写现存密码。
- 两路同用户名注册只有一个账户/会话成功，另一请求返回Conflict；16路同一refresh轮换只有一个替代配对。旧access通过明确配对撤销，新配对保留同一grant/session/family和原权限上界。并发旧摘要重放最终撤销对应刷新族，这是T23的预期保护，不能把唯一成功响应理解为该族此后仍活动。
- 本地refresh复用T22/T23原子轮换、摘要历史及重放保护，但核对明确来源。本地记录oidc_client_id为NULL，OIDC记录要求登记client_id；同为cobotusr受众的OIDC refresh仍不能从本地刷新，本地refresh也不能被OIDC客户端使用。旧refresh JWT即使签名有效且在旧refresh_token表登记，也不能从新路径签发。缺完整配对/根的旧NULL记录没有自动升级为可信凭据。
- 登录上界包含当前允许并剔除显式拒绝，超级管理员也不能覆盖拒绝；刷新后新增权限不进入原上界。会话根撤销后本地刷新被拒绝；HTTP重放使该族替代access通过严格登记检查失败，另一次独立登录的会话仍可用。minted_by与revocation_parent_jti保持分离，没有把轮换审计来源变成撤销父。
- UserService的auth/register/refresh使用统一生命周期，移除旧独立refresh JWT轮换及AppState::sign_access_token用户签发辅助。管理员创建账户只返回UserVO，不签发凭据。本地期限取config.auth，测试设access=300秒、refresh=3600秒，同时OIDC覆盖值=30/60秒，实际access与Cookie仍取本地期限。SessionTokenSet.id_token改为可选，本地不签发ID Token，OIDC继续保持client_id受众ID Token。
- 真实HTTP注册/登录/刷新凭据通过实际JWKS的StrictAccessState验签并与主库登记匹配；检查错误密码、缺少Cookie、非法/缺字段请求、旧JWT拒绝、刷新配对失效与重放后的独立会话隔离。缺时钟监控、不健康时钟和HMAC签发配置分别使登录/注册返回503，不设置成功Cookie或留下新账户/凭据。健康测试时钟不代表生产同步监控。

### 验证记录

以下命令均实际执行并退出0：

```text
cargo test --test mod --locked -- --test-threads=4
100 passed; 0 failed; 0 ignored; 0 filtered out

cargo test -p auth -p cobotusr --lib --locked
auth: 45 passed; 0 failed; 0 ignored
cobotusr: 23 passed; 0 failed; 0 ignored

cargo test --test mod security_t24 --locked -- --test-threads=4
8 passed; 0 failed; 0 ignored; 92 filtered out

cargo clippy -p auth -p cobotusr --lib --tests --locked -- -D warnings
cargo clippy --test mod --locked -- -D warnings
cargo fmt --all -- --check
git diff --check
```

100项集成测试与68项单元测试运行后，仅按clippy要求移除了PrincipalRef::new调用中的多余借用；最终clippy与8项T24测试在该修正后通过，没有后续代码变化。本机临时日志为 `/tmp/cobot-t24-integration-final.log`、`/tmp/cobot-t24-libs-final.log`、`/tmp/cobot-t24-targeted-final.log`、`/tmp/cobot-t24-clippy-libs.log` 和 `/tmp/cobot-t24-clippy-mod.log`。proc-macro-error2仍有future-incompat提示。历史auth文档示例8项失败/1项ignored未在本轮修正或重新验收，仍待T79。

### 未完成边界与分组提交

- 当前完成24/80，T34仍部分完成，下一项T25。旧logout仍验证旧refresh JWT，新opaque refresh会被拒绝；旧kickout只修改旧表，不能据此声称新会话已撤销。全撤销作用域、OIDC退出/无hint撤销与Cookie变更的CSRF校验尚待T25；浏览器退出保留独立MCP根仍未验收。
- 普通API当前凭据检查完整接线仍待T32。生产宿主必须通过serve_with_security_clock注入真实同步监控；普通serve无监控时新的OIDC及本地签发/刷新均失败关闭。生产时钟组装、前端、完整workspace/all-features、故障矩阵、跨服务10秒撤权与完整checklist尚未验收，不能宣称完整链路或全部漏洞已修复。
- 核心生命周期、仓储、OIDC可选ID Token适配及事务测试提交为 `19cf7df7ab113c139169404bd3f73121b31cec1a`（`feat(auth): add registered local sessions to the shared lifecycle`）。实际本地入口、用户服务清理、HTTP测试与本轮文档作为第二组本地提交；提交身份由本分支git log查询。仅暂存任务文件，没有push、部署或真实设备操作。
- 核心提交后宿主自动触发神医分析，4个文件回传均没有具体display/finds/vulnerability内容。该回传不证明安全或完整分析已结束，没有作为验收证据；本轮没有额外主动扫描或上传。

## 2026-11-08：T25 明确撤销作用域与浏览器证据

四份设计已经批准，沿用mew-spec和rust-dev-workflow继续，不重新审批。工作目录为 `/Users/xionghaoyi.binhy/BinhyPC/cobot-for`，分支 `codex/gateway-oidc-authorization`；保留T21–T24本地提交，只暂存本任务文件。没有Cargo.toml/Cargo.lock变化，没有push、部署或真实设备操作。

### 失败测试先行与修正过程

- 先注册并实际运行6项事务/并发测试：`0 passed; 6 failed; 100 filtered out`（`/tmp/cobot-t25-red.log`）。覆盖会话退出/独立MCP、单凭据/族显式委托传播、应用所有根、提交失败/继承预算、跨实例刷新竞争及OIDC客户端隔离。
- CSRF的3项单元失败测试：`0 passed; 3 failed; 45 filtered out`（`/tmp/cobot-t25-csrf-red.log`），覆盖issuer/refresh绑定、签名/专用用途、到期及不健康时钟。早期缺少Clone/sha2造成的编译失败不作为行为失败证据；SecretToken仍没有Clone，验证后显式复制必要证据。
- 实际HTTP先运行6项失败测试：`0 passed; 6 failed; 106 filtered out`（`/tmp/cobot-t25-http-red.log`），覆盖缺失/非受信/重复Origin及Cookie/证据、提交失败不清Cookie、consent、kickout、OIDC无hint撤销和GET退出确认。
- 通用日志脱敏先运行1项失败测试：`0 passed; 1 failed; 23 filtered out`（`/tmp/cobot-t25-logging-red.log`），随后剔除x-csrf-token头及csrf/logout表单和响应。
- 核心首轮实现为5通过/1失败（`/tmp/cobot-t25-core.log`）：100毫秒继承预算的数据库statement timeout可能先于外层截止返回DependencyUnavailable。测试接受DeadlineExceeded或DependencyUnavailable，两者均503；继续要求耗时小于1秒且回滚后凭据活动，没有调整全局错误分类或重新计时。初版12项T25集成测试随后全部通过（`/tmp/cobot-t25-integration.log`）。
- 扩展到9项事务/并发及9项HTTP测试，加入管理用途/所有权/显式拒绝/旧权限上界、kickout保留独立MCP、撤销与新根登记的旧快照竞争、consent提交回滚/授权码取消、真实CSRF启动/轮换、OIDC Basic认证与未知值无操作。确认POST退出从真实GET表单取得证据，并验证非法redirect无副作用。
- 完整回归曾为116通过/1失败（`/tmp/cobot-t25-all.log`）：历史OIDC夹具auth.secret为空，新CSRF确认页正确503，补齐夹具配置后重跑。随后117通过/1失败（`/tmp/cobot-t25-all-final.log`）：新增Basic测试把含空格的登录密码作为client secret，被SecretToken严格规则拒绝。改用实际regenerate_client_secret生成值，保留生产验证规则。最终专项18项及完整118项全部通过。

### 实现及证明范围

- RevocationScope明确区分单凭据、当前session、refresh family和应用下全部grant；kickout内部使用用户交互会话作用域。同一主库一致事务锁定稳定根/凭据、更新明确状态并提交，回执仅在提交后返回；committed_at/effective_by为毫秒，effective_by=committed_at+10_000。最多4次序列化/死锁冲突重试继承最初SecurityBudget。
- 本地logout通过opaque refresh摘要和历史定位完整配对及根；已消费旧refresh仍能终止对应当前会话。logout/kickout撤销对应session、family及凭据，保留独立MCP/其他用户根。应用撤销包含该用户在该应用下的所有session/MCP grant，并在同一事务删除对应consent和未兑换授权码、标记旧oauth_token撤销。consent须归属当前主体，绑定/历史应用映射缺失或歧义失败关闭。
- 应用撤销和kickout与新会话根登记共用用户行写屏障，撤销前的REPEATABLE READ签发快照不能在撤销提交后创建活动根；冲突后按原预算重新决策。T26的独立MCP新根登记必须复用这一约束。T25中的MCP记录由测试夹具创建，仅证明作用域隔离及撤销，未证明MCP新建/轮换入口已整改。
- 撤销只沿显式revocation_parent_jti传播，minted_by仍仅审计。单凭据作用域不扩散到同grant的独立凭据；族/session/应用按明确稳定根更新。管理撤销要求登记UserSession、aud/application=cobotusr、当前RSA签名/claims/kid与锁定主库事实相符。单凭据限同用户；kickout和应用/consent删除在撤销事务重新检查URI权限、显式拒绝和旧权限上界，提交前仍检查原身份新鲜期。
- LocalStrictAccessState只使用配置RSA进行本地验签并检查当前登记，支持调用方原预算；kickout和consent DELETE从旧中间件移入get_revocation_routes，没有对自身JWKS的HTTP回调。移除旧UserService JWT退出、旧OidcService撤销辅助及生命周期过渡revoke_session_refresh。
- OIDC revoke支持Basic或body客户端认证并拒绝冲突；客户端认证先于登记查询和未知值成功无操作。hint仅作提示，可通过refresh摘要/消费历史或严格签名access引用撤销，配对refresh撤销也撤销其配对access；其他客户端/本地来源不被误撤销。revocation_reference允许过期签名凭据仅用于引用，校验RS256、at+jwt、kid和issuer，不生成可认证上下文，不回退旧明文登记。
- CsrfProtector使用配置auth.secret签名HS256 csrf+jwt，绑定issuer、专用audience、nonce、iat/exp及当前refresh/启动状态的SHA-256摘要，期限900秒。CSRF不作为access凭据；access仍严格RS256。Cookie变更必须携带精确配置Origin（或配置issuer Origin）及匹配签名Cookie/Header或退出form证据，不信任Host/通配符；缺失/null/非受信/重复Origin、重复或非法Cookie、重复证据头及错误绑定均无副作用。
- GET /api/v1/csrf提供启动或当前refresh绑定证据及Secure、Path=/、SameSite=Lax的__Host-cobotusr-csrf Cookie，响应no-store。登录/注册/刷新使用原预算校验证据，成功后返回绑定新refresh的csrf_token。管理撤销携带Cookie时也要求CSRF。GET /oidc/logout仅确认、不清会话；POST在事务复核redirect及登记会话，提交后才清Cookie/存储并跳转。提交失败HTTP503且不设置清Cookie响应。
- 以上失败到通过验证覆盖当前仓储和实际撤销入口，保留原2秒预算、最多5秒当前状态新鲜期、拒绝优先与权限上界；不能据此认定普通业务API、前端、生产时钟或跨服务十秒撤权已验收。

### 最终验证记录

以下命令均实际执行并退出0；最后修改仅修正Basic测试夹具，随后重跑专项、完整集成及root-test Clippy，未再修改生产代码：

```text
cargo test --test mod security_t25 --locked -- --test-threads=4
18 passed; 0 failed; 0 ignored; 100 filtered out

cargo test --test mod --locked -- --test-threads=4
118 passed; 0 failed; 0 ignored; 0 filtered out

cargo test -p auth -p cobotusr --lib --locked
auth: 48 passed; 0 failed; 0 ignored
cobotusr: 24 passed; 0 failed; 0 ignored

cargo clippy -p auth -p cobotusr --lib --tests --locked -- -D warnings
cargo clippy --test mod --locked -- -D warnings
cargo fmt --all -- --check
git diff --check
```

最终临时日志为 `/tmp/cobot-t25-targeted-final.log`、`/tmp/cobot-t25-regression-final.log`、`/tmp/cobot-t25-libs-final.log`、`/tmp/cobot-t25-clippy-libs-final.log`、`/tmp/cobot-t25-clippy-mod-final.log`。本节持久记录命令/结果，临时日志不作为唯一证据。工具链及隔离Docker PostgreSQL沿用T24基线。修正1项collapsible-if后Clippy通过，未添加对应lint抑制；proc-macro-error2 future-incompat提示仍存在。

### 未完成边界与分组提交

- 当前完成25/80，T34仍部分完成，下一项T26/T27 MCP新建及轮换。普通API当前状态/管理组装仍待T32；旧OIDC introspect尚未迁移。本轮不证明全部业务入口已拒绝被撤销access。
- 前端尚未适配当前CSRF、刷新及退出确认协议，仍待T67及后续链路任务。生产clock monitor/startup仍待T70/T71，普通serve没有注入真实监控时失败关闭。全workspace/all-features（含OpenAPI）、历史auth文档8失败/1 ignored、故障矩阵、跨服务十秒撤权和完整checklist仍待T79/T80等任务，不勾选完整链路验收或宣称全部漏洞已修复。
- 核心撤销、主库串行约束、严格本地认证、CSRF与事务测试提交为 `5b45bbca4d6406d9297b3f0943e979378bb4f618`（`feat(auth): add scoped registry revocation and CSRF evidence`）。为保持该提交可独立组装，旧revoke_session_refresh删除与其旧调用方清理放入第二组。实际路由、Cookie/用户服务迁移、HTTP测试及本轮文档作为第二组本地提交，身份以本分支git log为准。
- 核心提交后宿主自动触发神医分析，12个文件回传均没有具体display/finds/vulnerability内容；该回传不证明安全或完整分析已结束，没有作为验收证据，也没有额外主动扫描或上传。

## 2026-11-08：T26 MCP 新建与 T27 原子轮换

四份设计已批准，沿用mew-spec和rust-dev-workflow继续，不重新审批。工作目录为 `/Users/xionghaoyi.binhy/BinhyPC/cobot-for`，分支 `codex/gateway-oidc-authorization`；开始时工作区干净，保留T21–T25本地提交，显式暂存本任务文件。当前完成27/80，下一项T28，T34仍部分完成。Cargo.toml/Cargo.lock没有修改，没有push、部署或真实设备操作。

### 失败测试先行与修正过程

- typed资源与权限上界交集先运行2项auth单元测试：`0 passed; 2 failed; 48 filtered out`（`/tmp/cobot-t26-bounds-red.log`）；实现后同2项通过（`/tmp/cobot-t26-bounds-green.log`）。覆盖Application/Gateway/Device/Exact更窄交集、应用/kind/网关不匹配、不同operation及空上界。
- T26新建先运行4项事务/并发测试，桩返回Forbidden：`0 passed; 4 failed; 118 filtered out`（`/tmp/cobot-t26-red-corrected.log`）。覆盖合法管理新建/独立根、旧目标上界/deny/MCP管理身份拒绝、签名/提交/继承预算回滚及20根并发配额。早期夹具按不存在的英文角色名查询失败不作为行为失败证据；改用既有role_id=3后取得上述真实失败。
- T27轮换先运行3项事务/并发测试，桩返回Forbidden：`0 passed; 3 failed; 122 filtered out`（`/tmp/cobot-t27-red.log`）。覆盖稳定根及旧上界、显式委托撤销、并发一次替代与签名/提交失败回滚。
- 原9项MCP HTTP测试全部保留并适配健康时钟、真实RSA、登记UserSession及typed目标权限。Config::default本地TTL为0，夹具显式设access=300秒/refresh=3600秒；修正夹具后的旧入口仍出现新token缺token_use：`8 passed; 1 failed; 116 filtered out`（`/tmp/cobot-t26-http-red.log`），随后迁移实际管理入口。
- 新增2项T26严格HTTP失败测试：旧RSA凭据新建返回200而预期401、Cookie无CSRF返回200而预期403，`0 passed; 2 failed; 126 filtered out`（`/tmp/cobot-t26-http-strict-red.log`）。新增T27并发HTTP失败测试：8路旧入口轮换全部200，success=8而预期1，`0 passed; 1 failed; 127 filtered out`（`/tmp/cobot-t27-http-red.log`）。接入统一生命周期后通过，另检查未知legacy metadata不参与列表或轮换。
- 首轮MCP轮换实现试图写consumed_at/replacement_jti，触发现存schema只允许refresh使用这些历史字段的约束。改用access行条件设置revoked_at作为一次消费，不修改schema或放宽refresh重放历史规则。
- 补充旧一致快照与应用撤销竞争、管理URI权限/当前目标权限变化、重叠deny及非法元数据边界。非法deny operation先真实失败：签发成功而预期Forbidden，`0 passed; 1 failed; 130 filtered out`（`/tmp/cobot-t26-deny-red.log`）；实现目标应用deny-all后通过。最后增强元数据引用的grant issuer/subject/application及无session/family/parent校验，随后重新运行全部专项、单元、集成和Clippy。

### 实现及证明范围

- `CredentialBounds::intersection`只保留同operation且typed资源相交的更窄范围。新cobotusr UserSession除管理URI上界外携带当前cobotsrv结构化权限上界，refresh继续保留签发旧上界。新建取管理旧上界∩当前目标权限；轮换再∩旧MCP上界，角色或资源扩权不扩大旧凭据。
- 同一主库一致快照读取用户、角色、权限及显式deny。全部重叠deny优先；v1上界不能表示allow减deny，故剔除整个重叠allow selector，避免删deny后扩大既有凭据。deny selector非法时剔除对应operation所有allow；deny operation非法时目标应用deny-all。缺应用、旧glob、未登记操作或必要资源事实不成为授权。新建/轮换交集必须仍允许mcp:access作用于目标Application。
- `service/security/mcp.rs`只接受当前登记的UserSession，管理aud/application固定cobotusr，目标固定已批准的cobotsrv。列表/新建/重命名/删除/轮换逐一检查具体管理URI、拒绝优先、管理旧上界及本人所有权；MCP或业务凭据不能充当管理凭据。没有超级管理员绕过deny。
- 新MCP claims使用RS256、at+jwt、User/McpAccess及完整registry登记；创建独立grant_kind=mcp，没有session/family/revocation_parent。minted_by记录当前管理credential，仅审计。签名、grant、registry及mcp_token元数据同一REPEATABLE READ / READ WRITE事务，提交后才返回秘密；签名、预算或提交失败不留活动根、凭据或元数据。
- 新根登记复用应用撤销/踢出的用户行写屏障；旧撤销前签发快照不能在撤销提交后创建活动MCP根。配额按活动registry的不同MCP grant计数，最多20根，兼容表不作为配额或安全活动事实。并发用例验证不超额，旧快照用例验证撤销后无新活动根。
- 轮换锁定旧grant/credential，同事务以条件UPDATE旧access.revoked_at消费，撤销旧凭据及显式revocation_parent_jti后代，登记同grant新jti并提交。替代凭据没有旧jti撤销父，撤销旧凭据不连带撤销替代；当前会话退出保留独立MCP，应用根撤销仍终止对应MCP。并发事务及8路HTTP均只一个替代成功；回滚保留旧活动凭据且不留下替代。
- 每个MCP生命周期方法从manager.budget().child(caller_budget.remaining())继承原SecurityBudget，签名等待及主库连接/语句/锁等待受同一原2秒预算约束，没有重新计时或副本/缓存回退；提交前仍检查管理身份新鲜期。
- mcp_token只提供标签和时间。列表复核registry活动状态，重命名/删除核对本人及登记根；未知legacy行不会自动升级成可信credential。统一撤销事务同步兼容表revoked标记。访问令牌值不持久化，成功签发/轮换只在响应中返回一次。
- `route/mcp.rs`从旧auth中间件组迁入get_revocation_routes中的独立管理组，使用browser::authenticate_manager本地严格RSA及主库登记，无自身JWKS HTTP递归。Cookie参与时校验精确受信Origin和真实签名CSRF；成功响应Cache-Control:no-store。保留默认aud=cobotsrv、名称/TTL及一次返回秘密的响应格式，移除AppState::mint_access_token和service/mcp_token旧CRUD/独立签发辅助。
- 仅修正新插入mcp:access种子为action=mcp:access、typed Application selector；已有管理员或legacy权限条目未修改或升级。旧上界测试在登录前显式恢复legacy glob，验证不能借当前扩权创建MCP。T34完整操作目录、路由覆盖和应用种子/迁移仍未完成。

### 最终验证记录

以下命令均实际执行并取得退出0，最后两处生产修正后重新运行。新增13项integration（T26 8项、T27 5项）及2项auth单元；原9项MCP HTTP测试和历史集成用例没有删除或ignore：

```text
cargo test --test mod security_t26 --locked -- --test-threads=4
8 passed; 0 failed; 0 ignored; 123 filtered out

cargo test --test mod security_t27 --locked -- --test-threads=4
5 passed; 0 failed; 0 ignored; 126 filtered out

cargo test --test mod --locked -- --test-threads=4
131 passed; 0 failed; 0 ignored; 0 filtered out

cargo test -p auth -p cobotusr --lib --locked
auth: 50 passed; 0 failed; 0 ignored
cobotusr: 24 passed; 0 failed; 0 ignored

cargo clippy -p auth -p cobotusr --lib --tests --locked -- -D warnings
cargo clippy --test mod --locked -- -D warnings
cargo fmt --all -- --check
git diff --check
```

最终日志为 `/tmp/cobot-t26-targeted-final.log`、`/tmp/cobot-t27-targeted-final.log`、`/tmp/cobot-t26-t27-regression-final.log`、`/tmp/cobot-t26-t27-units-final.log`、`/tmp/cobot-t26-t27-clippy-libs-final.log`、`/tmp/cobot-t26-t27-clippy-mod-final.log`。本节持久记录命令/结果，临时日志不是唯一证据。沿用rustc/cargo 1.99.1及隔离Docker PostgreSQL 18-alpine；proc-macro-error2 future-incompat提示仍存在。

另实际尝试 `cargo clippy -p auth -p cobotusr --lib --all-features --locked -- -D warnings`，退出101：utoipa-swagger-ui构建下载v5.17.14.zip后 `InvalidArchive("Could not find EOCD")`（`/tmp/cobot-t26-t27-clippy.log`）。该依赖下载阻塞不记为通过；本轮未修改Cargo依赖或扩展范围修旧OpenAPI。

### 未完成边界与分组提交

- 当前完成27/80，下一项T28机器认证与引导，T34仍部分完成。旧OIDC introspect及MCP服务introspect仍待迁移；后者目前只返回兼容denylist，未知jti的revoked=false不证明活动。普通API当前状态/管理组装仍待T32，本轮不证明所有业务入口已拒绝旧access。
- 前端管理PKCE、CSRF/刷新/退出/MCP页面适配仍待T66–T68；生产clock monitor/startup仍待T70/T71。普通serve无真实监控时新的安全/MCP管理入口失败关闭。完整workspace/all-features、历史auth文档8失败/1 ignored、前端、故障矩阵、跨服务十秒撤权与checklist仍待后续T79/T80等任务，不能宣称完整链路或全部漏洞修复。
- 核心typed交集、MCP生命周期/主库仓储、撤销镜像、新插入种子与事务测试提交为 `5eca8fba15ac247f768046333011e22c70697159`（`feat(auth): add registered MCP issuance and atomic rotation`）。实际MCP HTTP接线、旧签发/CRUD清理、HTTP回归及本节进度/交接作为第二组本地提交，身份以本分支git log为准；只显式暂存本任务文件，没有push、部署或真实设备操作。
- 核心提交后宿主自动触发神医最近commit分析，12个文件回传均没有具体display/finds/vulnerability内容。该回传不证明安全或完整云端分析结束，不作为验收证据；本轮没有额外主动扫描或上传。
