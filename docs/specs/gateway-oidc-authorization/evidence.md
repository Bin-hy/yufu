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
