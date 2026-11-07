# 网关 OIDC、RBAC/ABAC 与执行授权整改 Tasks

依据：[已批准 Spec](./spec.md) 与 [已批准 Plan](./plan.md)。

状态：已批准。用户已于 2026-11-07 确认完整任务拆解，进入 checklist.md 验收设计；任务尚未执行，四份文档全部批准前不编写实现代码。

## 执行规则与验证命令

- 下列任务按 2–5 分钟的聚焦编辑单元拆分，编译、依赖下载和测试运行等待另计；实际超过该粒度时，在当前任务下细分步骤并保留全部验证要求，不合并成无验证的大任务。
- 每项行为修改先补对应的失败用例，再完成实现并取得通过证据。任务完成须记录命令、退出码、实际执行数量和结果；过滤结果为 0、ignore 或环境失败不能记为通过。
- 表中的测试名是待新增的测试标识，不表示现有仓库已经有这些测试。对应任务负责新增并注册用例；数据库测试使用隔离库，网关测试使用模拟设备。
- U(case)：`cargo test -p auth --lib case -- --nocapture`。
- I(case)：`cargo test --test mod cobotusr::routes::security::case -- --nocapture`。
- S(case)：`cargo test --test mod cobotsrv::case -- --nocapture`。
- G(case)：`cargo test -p cobotlet --lib case -- --nocapture`。
- 前端类型检查：分别运行 `pnpm --filter @cobotweb/cobotsrv exec vue-tsc -b`、`pnpm --filter @cobotweb/cobotusr exec vue-tsc -b`；行为验证使用隔离服务和浏览器，不把类型检查视为登录流程通过。
- 测试编号统一使用 security_tNN 前缀。I 用例置于 `tests/cobotusr/routes/security.rs`；S 用例置于表中指定的云端测试文件；U/G 用例置于对应实现文件的测试模块。
- 只提交本任务涉及的文件；保留现有两个 assets/.gitkeep 删除及其他未提交内容。每组逻辑相关任务完成且验证通过后提交代码，不自动推送或部署。

## 文件清单

此表给出文件级归属；各任务行进一步指定编辑位置。除文档外，下列均为计划变更。

| 操作 | 文件 | 职责 |
|---|---|---|
| 新建 | `libs/auth/src/security/{mod,identity,credential,policy,message,budget,error,redaction}.rs` | 共享身份、验证、规则、消息、时限及脱敏契约 |
| 修改 | `libs/auth/src/{lib,claims,claims_extractor,jwks,token_manager}.rs`、`libs/auth/Cargo.toml` | 接入统一契约及专用凭据验证 |
| 新建 | `cobotusr/src/service/security/{mod,lifecycle,authorization,delegation,repository}.rs` | 凭据生命周期、中心授权、一次许可及仓储接口 |
| 新建 | `cobotusr/src/entity/security_{credential,grant,session,refresh_family,binding,permit,audit}.rs` | 身份与安全事实模型 |
| 新建 | `cobotusr/src/route/{security,security_admin}.rs` | 受限安全服务、身份绑定与管理入口 |
| 修改 | `cobotusr/src/service/{mod,user,oidc,mcp_token,permission,authority}.rs`、`rbac.rs`、`lib.rs`、`config.rs` | 统一两条登录链及权限路径 |
| 修改 | `cobotusr/src/route/{mod,auth,oidc,oidc_admin,mcp,enforce,sync_permissions,role,role_permission,user_authority,user_permission,permission,application}.rs` | 路由、覆写、权限管理与生命周期接入 |
| 新建 | IdP `migration/m20261107_000001_security_identity.rs`、`m20261107_000002_security_permit_audit.rs`，`seed/m20261107_000003_security_operations.rs` | schema、约束及操作登记，完整根目录为 `cobotusr/src/` |
| 新建 | `cobotsrv/src/service/{authorization,task,execution,gateway_message,gateway_binding}.rs` | 云端资源授权、任务协调、执行与可信消息 |
| 新建 | `cobotsrv/src/entity/{task,task_reservation,execution_claim,task_outbox,gateway_binding,security_audit}.rs` | 任务、占位、绑定和审计事实 |
| 新建 | `cobotsrv/src/route/{execution,gateway_operation}.rs` | 机器执行服务及直达网关准备入口 |
| 修改 | `cobotsrv/src/{lib,config,enforce,mqtt}.rs`、`entity/{mod,device}.rs`、`service/{mod,device}.rs`、`route/{mod,task,device,tool,cobotlet,remote,easytier_config}.rs`、`mcp/{mod,route,handler,admin,revocation}.rs` | 移除旧绕过路径并组装统一服务 |
| 新建 | `cobotsrv/src/migration/m20261107_000001_secure_task_execution.rs`、`m20261107_000002_gateway_binding_audit.rs` | 云端任务及绑定迁移 |
| 新建 | `cobotlet/src/service/{mod,authorization,execution,device_registration,target_policy,gateway_signer}.rs`、`store/{replay,schema}.rs` | 网关统一接入、执行、目标校验和持久去重 |
| 修改 | `cobotlet/src/{lib,config,dispatcher,mqtt,events,heartbeat}.rs`、`store/{mod,task,device}.rs`、`route/{mod,task,device,tool,hello}.rs` | 入口保护、状态及受限派发 |
| 新建 | `cobotweb/apps/cobotsrv/src/idp-management-auth.ts`、`pages/idp-management-callback.vue` | 独立管理受众 PKCE |
| 修改 | cobotsrv 前端 `src/{auth,router,http}.ts`、`src/pages/{callback,tasks}.vue`、`src/pages/mcp/{tokens,connect}.vue`、`src/composables/useControlEngine.ts`，共享 `src/types/task.ts` | 双受众、刷新锁、控制输入及 Unknown 展示；根分别为 `cobotweb/apps/cobotsrv/`、`cobotweb/packages/shared/` |
| 修改 | `cobotweb/apps/cobotusr/src/pages/oidc-login/index.vue`、`pages/oidc-consent/api/index.ts`、`http/axios.ts`、`pinia/stores/user.ts` | 新管理会话、同意和退出适配 |
| 新建/修改 | `tests/cobotusr/routes/security.rs`、`tests/cobotsrv/{authorization,execution,gateway}.rs`、对应 `common.rs` 与 `mod.rs`；既有 OIDC/MCP/auth/sync 测试 | 单元、事务、路由、多实例与故障验证 |
| 修改 | 三个服务的 `migration/mod.rs` 或 `store/mod.rs`、`entity/mod.rs`、`service/mod.rs`、Cargo 清单 | 新模块注册和实际必要依赖 |
| 修改 | `conf/{cobotusr,cobotsrv,cobotlet}.example.toml`、`src/{cobotusr,cobotsrv,cobotlet,cobotrix,cobotnil}.rs`、`.github/workflows/rust.yml` | 安全配置、启动依赖、脱敏及回归门禁 |
| 新建 | `docs/specs/gateway-oidc-authorization/{migration,evidence}.md` | 迁移说明与实际验证证据 |

花括号列举表示每个具体文件，不表示新建一个包含花括号的路径。实体、路由和测试新增时必须同步注册其父模块。

## 任务组一：环境与共享契约

| 任务 | 文件 | 依赖 | 步骤 | 验证 |
|---|---|---|---|---|
| T01 环境基线 | `evidence.md`、Cargo.lock（仅必要时） | 无 | ①记录版本与本地改动；②准备锁定依赖、protoc、Docker及前端包，解决缺失子模块，不擅自升级版本 | `cargo metadata --locked --format-version 1`、`docker version`、`pnpm --version` 成功；保存真实失败或成功证据 |
| T02 测试入口 | `tests/cobotusr/routes/{mod,security}.rs`、`tests/cobotsrv/{mod,authorization,execution,gateway}.rs`、两侧 `common.rs` | T01 | ①注册新测试模块；②提供隔离主库、测试issuer、模拟设备和可注入时钟夹具 | `cargo test --test mod -- --list` 包含新夹具验证用例；运行夹具用例确认数据库相互隔离 |
| T03 主体与秘密类型 | `libs/auth/src/security/{mod,identity,error}.rs`、`lib.rs` | T01 | ①定义 PrincipalRef/TokenUse/IdentityBinding/秘密类型与错误；②校验身份命名空间并实现脱敏输出 | U(security_t03)：同名不同主体不相等，秘密 Debug 不泄露 |
| T04 资源和授权契约 | `libs/auth/src/security/policy.rs` | T03 | ①定义 ResourceRef/OperationContext/授权上界；②对每种资源执行必填字段与无歧义编码校验 | U(security_t04)：资源缺失拒绝、分隔符不能混淆目标 |
| T05 请求预算 | `libs/auth/src/security/budget.rs` | T03 | ①定义可注入时钟与 SecurityBudget；②子调用只能消耗/缩短剩余预算 | U(security_t05)：排队计时、外部期限不能延长2秒上限 |
| T06 授权新鲜期 | `libs/auth/src/security/{budget,credential}.rs` | T05 | ①定义 checked_at/valid_until；②取5秒、父凭据和原截止期最小值并扣除时钟余量 | U(security_t06)：延迟响应/重复缓存/时钟边界不能续期 |
| T07 JWT 负向用例 | `libs/auth/src/security/credential.rs`、`jwks.rs` | T03 | ①定义 CredentialExpectation 和验证接口；②新增错误issuer/aud/use/算法/过期测试，完成可信密钥适配 | U(security_t07)：所有错误凭据拒绝，合法凭据可解析 |
| T08 认证上下文 | `libs/auth/src/{lib,claims_extractor}.rs`、`security/credential.rs` | T07 | ①只提取一次Bearer并生成可信上下文；②移除普通API query bearer回退 | U(security_t08)：query不能单独认证，伪造扩展不能绕过验证 |
| T09 签名消息契约 | `libs/auth/src/security/message.rs`、`token_manager.rs` | T03、T04 | ①定义许可/消息类型与不可变载荷摘要；②分离access、许可和事件的签名类型 | U(security_t09)：篡改完整载荷或互换用途验签/验证失败 |
| T10 统一脱敏 | `libs/auth/src/security/redaction.rs` | T03 | ①实现URI、头、敏感路由体与配置脱敏；②用完整凭据标记做日志捕获断言 | U(security_t10)：请求、响应、错误输出都无秘密标记 |

## 任务组二：规则判定与 IdP 存储

| 任务 | 文件 | 依赖 | 步骤 | 验证 |
|---|---|---|---|---|
| T11 拒绝优先 | `libs/auth/src/security/policy.rs` | T04 | ①先写多个重叠allow/deny用例；②实现全部匹配拒绝先于任何允许 | U(security_t11)：普通用户和超级管理员均不能绕过显式拒绝 |
| T12 上界与资源约束 | `libs/auth/src/security/policy.rs` | T11 | ①先覆盖角色新增/撤销、用户允许、无授权；②实现当前权限与凭据上界、资源约束交集 | U(security_t12)：角色扩权不扩大受限旧凭据，缺失授权默认拒绝 |
| T13 身份 schema | IdP `m20261107_000001_security_identity.rs`、`migration/mod.rs` | T02、T03 | ①创建凭据/根/会话/族表与唯一约束；②注册迁移 | I(security_t13)：空库up、隔离库down/up及重复jti约束通过 |
| T14 绑定 schema | 同一身份迁移、`entity/security_binding.rs` | T13 | ①加入机器用途/应用、设备归属、受众及网关公钥绑定；②创建相应唯一约束 | I(security_t14)：冲突设备/密钥绑定被约束，禁用状态可持久化 |
| T15 许可和审计 schema | IdP `m20261107_000002_security_permit_audit.rs`、`migration/mod.rs` | T13 | ①创建一次签发及审计表；②对同任务/请求建立唯一许可索引 | I(security_t15)：重复许可键失败，审计事务回滚不留半条事实 |
| T16 身份实体 | `cobotusr/src/entity/security_{credential,grant,session,refresh_family}.rs`、`entity/mod.rs` | T13 | ①映射字段与状态；②定义稳定根与审计来源的不同关系 | I(security_t16)：往返加载保留根/parent区别，不存refresh明文 |
| T17 安全仓储接口 | `cobotusr/src/service/security/{mod,repository}.rs`、安全实体 | T14–T16 | ①定义状态/绑定/许可事务仓储接口；②实现最小主库查询和秘密摘要查找 | I(security_t17)：未知/禁用记录拒绝，查询不依赖只读副本 |
| T18 凭据有效状态 | `service/security/lifecycle.rs` | T17、T06 | ①加载凭据及稳定根；②实现到期、撤销和单层委托父检查 | I(security_t18)：未知凭据、父被撤销、会话/族失效均拒绝 |
| T19 URI 权限适配 | `service/authority.rs`、`rbac.rs`、`service/security/authorization.rs` | T12、T17 | ①建立已登记URI操作映射；②删除未知规则放行并使用纯判定器 | I(security_t19)：未知路由拒绝，用户覆写优先级正确 |
| T20 应用权限适配 | `service/permission.rs`、`service/security/authorization.rs` | T12、T17 | ①主库一致快照加载应用权限；②要求资源字段完整并扫描全部拒绝 | I(security_t20)：网关A权限不能操作B，重叠允许不盖过拒绝 |

## 任务组三：生命周期与安全服务

| 任务 | 文件 | 依赖 | 步骤 | 验证 |
|---|---|---|---|---|
| T21 授权码原子消费 | `service/security/lifecycle.rs`、`service/oidc.rs` | T18 | ①先验证client/redirect/PKCE；②事务内条件消费并登记替代凭据 | I(security_t21)：并发只有一次成功，错误verifier不烧码 |
| T22 普通刷新原子轮换 | `service/security/lifecycle.rs`、`service/oidc.rs` | T21 | ①消费旧refresh并签发同根替代值；②同时使旧access失效 | I(security_t22)：并发不产生多个替代，签名/提交失败回滚 |
| T23 refresh 重放保护 | `service/security/lifecycle.rs`、`entity/security_refresh_family.rs` | T22 | ①保留已消费值摘要；②检测复用并撤销对应族 | I(security_t23)：重放只撤销正确族，不影响其他会话 |
| T24 本地登录汇入 | `service/user.rs`、`route/auth.rs` | T18、T22 | ①登录/注册登记UserSession及根；②本地refresh调用统一轮换，修正IdP受众 | I(security_t24)：本地登录凭据可统一吊销，无旧签发旁路 |
| T25 撤销作用域 | `service/security/lifecycle.rs`、`route/{auth,oidc}.rs` | T18、T24 | ①实现单凭据/当前会话/刷新族/应用根撤销；②提交后回执，Cookie变更校验CSRF | I(security_t25)：浏览器退出保留独立MCP，撤销应用授权终止全部对应根 |
| T26 MCP 新建 | `service/mcp_token.rs`、`route/mcp.rs` | T18、T20 | ①要求有效IdP管理凭据和目标app权限；②建立独立grant及权限上界 | I(security_t26)：旧/吊销/MCP凭据不能新建，合法个人管理可新建 |
| T27 MCP 轮换 | `route/mcp.rs`、`service/security/lifecycle.rs` | T26 | ①事务内锁定旧凭据；②仅一次替代且不把旧凭据作为撤销父 | I(security_t27)：并发一次成功，新凭据不被旧凭据撤销连带作废 |
| T28 机器认证与引导 | `route/security.rs`、`service/security/authorization.rs`、`route/oidc.rs` | T18、T14 | ①本地校验client_credentials和机器状态；②绑定用途/应用/受众，不递归调用PDP | I(security_t28)：非法app代理拒绝，机器subject不回退用户0 |
| T29 主体证据决策 | `route/security.rs`、`service/security/authorization.rs` | T28、T20、T06 | ①机器Bearer与subject evidence分别验证；②实现批量资源决策及源头有效期 | I(security_t29)：自报subject无效，超100资源拒绝，返回不可续期结论 |
| T30 一次许可签发 | `service/security/delegation.rs`、`entity/security_permit.rs` | T29、T15、T09 | ①将许可绑定摘要/父/目标并登记；②同绑定只返回原许可，冲突拒绝 | I(security_t30)：同任务无新jti或期限，不同摘要冲突 |
| T31 许可复核 | `route/security.rs`、`service/security/delegation.rs` | T30 | ①复核用途、父凭据及当前资源权限；②拒绝过期及再委托 | I(security_t31)：撤权/吊销父影响许可，复核不延长exp |
| T32 IdP 管理接入 | `lib.rs`、`route/{mod,security_admin,oidc_admin,role,role_permission,user_authority,user_permission,permission,application}.rs` | T19、T25、T28 | ①给管理路由绑定明确操作和用途；②角色/覆写/绑定写入使用本地授权服务 | I(security_t32)：普通或跨受众主体不能修改角色、公钥及应用绑定 |
| T33 权限同步隔离 | `route/sync_permissions.rs`、`service/security/authorization.rs` | T28、T32 | ①限定PermissionSync凭据；②校验客户端到app的服务端绑定后事务写入 | I(security_t33)：持有其他client secret不能写cobotsrv目录 |
| T34 操作种子与路由覆盖 | IdP `seed/m20261107_000003_security_operations.rs`、两服务 `route/mod.rs`、IdP `seed/mod.rs` | T19、T20、T32 | ①登记明确公开/保护路由；②添加必需种子而不向User自动授予执行权限 | I(security_t34)：新库/升级库迁移顺序正确，未登记保护路由被检查发现 |

## 任务组四：云端授权与任务事实

| 任务 | 文件 | 依赖 | 步骤 | 验证 |
|---|---|---|---|---|
| T35 云端任务 schema | 云端 `m20261107_000001_secure_task_execution.rs`、`migration/mod.rs` | T02 | ①建reservation/task/claim/outbox及唯一约束；②明确任务与设备占用条件 | S(execution::security_t35)：并发插入的task/idempotency/设备占用约束通过 |
| T36 云端绑定 schema | 云端 `m20261107_000002_gateway_binding_audit.rs`、`entity/{device,gateway_binding,security_audit}.rs` | T35 | ①增加设备/事件版本、公钥绑定及审计；②注册迁移与实体 | S(gateway::security_t36)：版本/消息去重往返持久化正确 |
| T37 任务实体与仓储 | 云端 `entity/{task,task_reservation,execution_claim,task_outbox,mod}.rs`、`service/{task,execution}.rs` | T35 | ①定义已批准状态及仓储接口；②实现按owner/application查任务 | S(execution::security_t37)：不同owner不能复用幂等键覆盖任务 |
| T38 中心客户端与缓存 | `cobotsrv/src/enforce.rs`、`service/authorization.rs` | T29、T07、T06 | ①调用受限安全端点；②实现完整缓存键、10000项上限和64并发合并 | S(authorization::security_t38)：资源/用途不串缓存，故障不使用过期Allow |
| T39 固定资源与过滤 | `service/{authorization,device}.rs`、`route/{device,tool,cobotlet}.rs` | T38、T36 | ①从可信目录选定目标及版本；②批量过滤后聚合provider/计数 | S(authorization::security_t39)：受限用户看不到其他provider与任务归属 |
| T40 任务预留 | `service/task.rs`、`entity/task_reservation.rs` | T37、T39 | ①规范载荷/摘要并预留不可执行记录；②同幂等键异摘要冲突，不存原Bearer | S(execution::security_t40)：未激活记录不能领取，表中无用户令牌明文 |
| T41 许可激活及outbox | `service/task.rs`、`entity/task_outbox.rs` | T40、T30 | ①取得原许可；②本地事务复核版本并写Pending与outbox | S(execution::security_t41)：许可成功但本地回滚不能执行，不重签旧task |
| T42 原子执行领取 | `service/execution.rs`、`route/execution.rs` | T41、T31 | ①验证绑定机器和许可；②同事务CAS任务及设备占用，只首请求Acquired | S(execution::security_t42)：同实例/跨实例重试均不再Acquired |
| T43 结果条件更新 | `service/{gateway_message,execution}.rs` | T42、T36、T09 | ①匹配key/claim/实例/digest；②条件写结果与释放对应设备 | S(execution::security_t43)：伪造/冲突结果拒绝，重复不释放另一任务 |
| T44 Unknown 管理核对 | `service/execution.rs`、`route/execution.rs` | T43 | ①禁止Unknown转Pending；②核对授权、原期限和设备状态后解除占用并审计 | S(execution::security_t44)：超时不转移资格，迟到结果只结束原claim |
| T45 发送与跨实例读取 | `mqtt.rs`、`service/task.rs`、`route/task.rs` | T41、T43 | ①outbox发送原封装并拒绝过期任务；②结果从共享主库读取，等待器仅加速 | S(execution::security_t45)：重复发送不产生新任务，不同实例读取同一可信结果 |
| T46 REST/MCP 共用授权 | `route/task.rs`、`mcp/{route,handler,revocation}.rs`、`lib.rs` | T39、T45 | ①REST/MCP统一调用任务服务；②移除MCP专属吊销旁路和旧claims回退 | S(authorization::security_t46)：只有mcp:access不能执行，吊销覆盖全部入口 |
| T47 代理和直达准备 | `route/{remote,gateway_operation}.rs` | T41、T39 | ①已登记代理映射执行双重授权；②直达请求获得同一绑定封装，不转发万能token | S(authorization::security_t47)：未知路径/仅代理权限不能执行下游管理 |
| T48 网络/MCP 管理权限 | `route/easytier_config.rs`、`mcp/admin.rs`、`route/mod.rs` | T38、T34 | ①明确各管理操作与资源；②处理相关公开入口的必需凭据，禁止空配置静默放行 | S(authorization::security_t48)：普通JWT不能报废/回收；缺安全配置不开放相关能力 |

## 任务组五：网关接入和持久执行

| 任务 | 文件 | 依赖 | 步骤 | 验证 |
|---|---|---|---|---|
| T49 redb schema | `cobotlet/src/store/{schema,replay,mod}.rs` | T03、T09 | ①建独立去重表及schema版本；②独占迁移旧执行中记录为历史未知 | G(security_t49)：升级/重启不自动重发，删除历史不删墓碑 |
| T50 网关安全客户端 | `service/{mod,authorization}.rs`、`config.rs` | T31、T38 | ①接入逐网关机器身份与凭据状态；②固定aud/use并继承预算和原期限 | G(security_t50)：错网关/吊销/过期许可拒绝，中心故障不放行 |
| T51 网关签名与公钥绑定 | `service/gateway_signer.rs`、`events.rs` | T09、T14 | ①加载本机独立私钥；②签名类型化消息，禁止日志输出私钥和凭据 | G(security_t51)：别的网关公钥不能验证本机消息，敏感输出为空 |
| T52 设备归属认证 | `service/device_registration.rs`、`route/device.rs` | T50、T14 | ①注册/心跳/能力/注销使用DeviceAccess；②ID取已验证绑定，不能覆盖其他设备 | G(security_t52)：设备A所有写操作不能作用于B |
| T53 目标与路径规则 | `service/target_policy.rs` | T52 | ①实现管理员origin/IP/端口/能力路径清单；②拒绝任意覆盖服务目标 | G(security_t53)：允许的局域网目标可用，未允许目标/路径拒绝 |
| T54 DNS/重定向/头 | `service/target_policy.rs`、`dispatcher.rs` | T53 | ①固定校验后的实际连接地址；②禁设备重试/重定向并限制头 | G(security_t54)：DNS变换与跨目标重定向不访问禁止服务，计数仅一次 |
| T55 设备版本确认 | `service/device_registration.rs`、`store/device.rs`、云端 `service/gateway_binding.rs` | T52、T36 | ①可信控制面分配版本并等待网关确认；②旧版本许可不能转移到新目标 | S(gateway::security_t55)：未确认/变化版本不能执行 |
| T56 本地接收去重 | `service/execution.rs`、`store/replay.rs` | T49、T50 | ①验完整命令后原子登记；②同摘要返回状态、异摘要冲突 | G(security_t56)：HTTP/MQTT并发重复不会进入多个首次分支 |
| T57 领取与派发标记 | `service/execution.rs`、`store/task.rs` | T56、T42 | ①只有当次Acquired允许继续；②发设备请求前持久DispatchMarked并重验期限 | G(security_t57)：领取响应丢失/持久化失败时设备计数为0 |
| T58 设备互斥所有权 | `store/device.rs`、`dispatcher.rs` | T57 | ①领取Ok(None)终止；②设备状态及释放条件绑定task/claim | G(security_t58)：并发不穿透占用，旧结果不能释放新任务 |
| T59 HTTP/MQTT 接入 | `route/{mod,task,tool,hello}.rs`、`mqtt.rs`、`lib.rs` | T57、T58 | ①两协议调用同一服务；②裸任务、queryBearer及无身份入口拒绝 | S(gateway::security_t59)：两入口授权一致，无JWT/MQTT裸消息不能控制 |
| T60 崩溃与Unknown | `service/execution.rs`、`store/task.rs`、`heartbeat.rs` | T57、T58 | ①恢复已领取/派发标记为Unknown；②网络超时不归为可重试普通失败 | G(security_t60)：在各持久化窗口恢复后模拟设备请求数不增加 |
| T61 结果持久化与重报 | `service/execution.rs`、`events.rs`、`mqtt.rs` | T60、T51、T43 | ①先存结果再签名；②只重报结果，执行已过期仍可提交匹配claim的迟到结果 | S(gateway::security_t61)：结果丢失重报后设备调用仍一次 |
| T62 设备事件顺序 | `events.rs`、`mqtt.rs`、`heartbeat.rs`、云端 `service/gateway_message.rs` | T51、T55 | ①签名并绑定登记流/版本/序号；②条件接收，空retained清除不冒充离线 | S(gateway::security_t62)：旧流/旧序号/错设备事件不能覆盖新状态 |
| T63 容量及墓碑清理 | `store/{replay,task}.rs`、`config.rs`、云端 `service/task.rs` | T49、T60 | ①区分展示历史与安全事实；②上限拒绝新记录，许可有效期内禁止删墓碑 | G(security_t63)：清理/容量超限后旧任务仍不能重复执行 |

## 任务组六：审计、前端与配置迁移

| 任务 | 文件 | 依赖 | 步骤 | 验证 |
|---|---|---|---|---|
| T64 事务审计 | IdP `service/security/lifecycle.rs`、云端 `service/{task,execution}.rs`、两侧安全审计实体 | T25、T27、T43 | ①成功变更事务内写脱敏审计；②记录拒绝与过载计数 | I(security_t64)及S(execution::security_t64)：回滚无假成功，主体/资源/原因可关联 |
| T65 全链日志脱敏 | `cobotusr/src/lib.rs`、`cobotsrv/src/lib.rs`、`cobotlet/src/lib.rs`、`cobotusr/src/service/database.rs`、`cobotsrv/src/service/database.rs`、`src/{cobotusr,cobotsrv,cobotlet,cobotrix}.rs` | T10、T64 | ①通用日志应用脱敏并禁敏感body；②移除原始配置/数据库URL秘密输出 | S(authorization::security_t65)：捕获签发/同步/URI/启动日志，无秘密标记 |
| T66 独立管理 PKCE | 前端 `idp-management-auth.ts`、`pages/idp-management-callback.vue`、`router.ts` | T26、T32 | ①实现独立state/verifier/回调；②只接受登记管理受众，令牌状态与业务分开 | 两前端类型检查；浏览器合法管理回调成功，错state拒绝、业务token不变 |
| T67 刷新与退出适配 | 前端 `auth.ts`、`http.ts`、cobotusr `http/axios.ts`、`pinia/stores/user.ts` | T22、T25、T66 | ①每受众刷新single-flight；②撤销含client_id并区分服务器失败 | 浏览器并发401只刷新一次；断网退出不显示服务端已撤销 |
| T68 MCP 页面接入 | 前端 `pages/mcp/{tokens,connect}.vue`、`auth.ts` | T66、T67 | ①管理调用使用专用凭据；②一次显示秘密并清理弹窗内存，不通过URL传递 | 浏览器创建/轮换/删除闭环，MCP凭据不能作管理登录 |
| T69 控制及Unknown UI | `useControlEngine.ts`、共享 `types/task.ts`、`pages/tasks.vue` | T47、T60 | ①适配幂等键/任务引用及错误；②展示Unknown并禁止自动补发新任务 | 类型检查；模拟超时后用户未操作时设备计数不增加 |
| T70 安全模板 | 三服务 `config.rs`、`conf/{cobotusr,cobotsrv,cobotlet}.example.toml` | T50、T55、T38 | ①声明issuer/受众/绑定/目标/容量/时钟要求；②生产缺必需配置拒绝，开发放宽显式 | S(gateway::security_t70)：缺配置不开放控制，开发配置显著标记 |
| T71 启动依赖 | `src/{cobotrix,cobotnil,cobotusr,cobotsrv,cobotlet}.rs`、相关 `lib.rs` | T70、T65 | ①等待IdP安全就绪再开放云端；②开发夹具生成隔离身份，不记录secret | S(gateway::security_t71)：IdP未就绪时不接受控制，显式开发集群可运行 |
| T72 迁移与恢复说明 | `migration.md`、三服务迁移及网关schema测试 | T34、T35、T49、T70 | ①写备份/维护窗口/绑定/令牌客户端迁移/恢复步骤；②隔离旧库演练，禁止旧任务重放 | I(security_t72)和S(gateway::security_t72)：演练后旧凭据与旧裸任务拒绝，claim保留 |

## 任务组七：端到端与门禁

| 任务 | 文件 | 依赖 | 步骤 | 验证 |
|---|---|---|---|---|
| T73 四入口授权对照 | `tests/cobotsrv/authorization.rs`、`gateway.rs` | T46–T48、T59 | ①同主体/资源通过REST/MCP/代理/直达调用；②覆盖连接允许但执行拒绝 | S(authorization::security_t73)：四入口业务权限结果一致，模拟设备未收越权指令 |
| T74 多实例撤权时限 | `tests/cobotsrv/authorization.rs` | T38、T46、T59 | ①至少两个云端及网关实例预热缓存；②撤销凭据/角色/范围后轮流访问 | S(authorization::security_t74)：所有实例10秒内拒绝，角色扩权不扩大旧上界 |
| T75 故障及2秒预算 | `tests/cobotsrv/authorization.rs`、`gateway.rs` | T05、T38、T59 | ①注入连接/锁/中心延迟和中断；②覆盖有效缓存、过期缓存及时钟异常 | S(authorization::security_t75)：安全链按2秒预算结束，失效状态不派发；记录命中/未命中耗时 |
| T76 跨库故障矩阵 | `tests/cobotsrv/execution.rs`、`gateway.rs` | T41–T45、T60、T61 | ①注入许可提交/本地回滚/领取响应丢失/设备响应丢失；②记录设备请求次数和持久状态 | S(execution::security_t76)：不可执行准备与Unknown均不重新取得派发资格 |
| T77 迁移后的登录闭环 | `tests/cobotusr/routes/security.rs`、`evidence.md` | T24–T27、T66–T72 | ①本地/OIDC登录与双受众管理闭环；②注销/授权撤销后重测旧凭据 | I(security_t77)及浏览器操作：旧凭据不能读取/执行/续签，作用域符合Spec |
| T78 CI 接入 | `.github/workflows/rust.yml`、测试 `mod.rs`、必要Cargo测试依赖 | T73–T77 | ①确保新用例真实注册与Linux隔离依赖可运行；②保留现有跨平台门禁 | `cargo test --test mod -- --list` 显示全部安全用例；CI不跳过/ignore新增验收 |
| T79 完整质量检查 | 全部改动文件、`evidence.md` | T78 | ①按下述完整命令运行门禁；②区分历史基线问题与新失败，修复本次相关问题并重跑受影响项 | 完整命令取得实际通过证据；阻塞不得记完成 |
| T80 对照验收与交付 | `checklist.md`、`evidence.md`、`migration.md` | T79及已批准checklist | ①逐条运行已批准checklist；②记录AC1–AC12证据和迁移风险，提交相关变更 | 所有必需条目通过；没有生产部署或真实设备指令 |

## 完整验证命令

```bash
cargo fmt --all -- --check
cargo test -p auth -p cobotusr -p cobotsrv -p cobotlet --lib
cargo test --test mod -- --test-threads=4
pnpm --filter @cobotweb/cobotsrv exec vue-tsc -b
pnpm --filter @cobotweb/cobotusr exec vue-tsc -b
pnpm -r --filter './cobotweb/apps/*' build
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features -- --test-threads=4
cargo test --workspace --doc
cargo machete
cargo audit
```

前端构建先于开启webui的全feature编译，避免rust-embed缺少产物。cargo audit需要可用漏洞库；工具缺失或数据库不可用须先解决，不能当作通过。禁止测试输出真实配置中的密码、凭据或连接URL。

## 执行顺序与进度

1. T01–T10 建立可验证基线和共享契约。
2. T11–T20 完成纯规则与IdP主库基础。
3. T21–T34 完成统一生命周期、中心决策和受限管理。
4. T35–T48 完成云端任务、资源授权和各入口接入。
5. T49–T63 完成网关执行与消息闭环。
6. T64–T72 完成审计、必要前端和迁移配置。
7. T73–T80 完成故障、多实例、端到端及完整门禁。

编号顺序是一个合法的默认拓扑顺序；依赖栏的区间表示依赖区间内全部任务。不存在要求后续任务先完成的循环依赖。任务按组形成可复核提交，开发阶段每项状态记录为未开始/进行中/阻塞/完成，只有实际验证通过才标完成。

## 覆盖检查与审批

| 设计范围 | 任务 |
|---|---|
| A1/B1 共享认证、主体与用途 | T03–T10、T28、T50 |
| A2/B2 生命周期与稳定撤销根 | T13–T18、T21–T27、T77 |
| B3 RBAC/ABAC、资源及默认拒绝 | T04、T11–T12、T19–T20、T32–T34、T39、T73 |
| B4/A5 许可与可信消息 | T09、T30–T31、T43、T51、T61–T62 |
| B5/C4–C6 任务、跨库提交、领取、Unknown | T35–T37、T40–T45、T49、T56–T63、T76 |
| C2/C5 入口一致性与代理 | T08、T46–T48、T59、T73 |
| C7 设备身份、目标与版本 | T14、T52–T55、T70 |
| B7 缓存、预算、多实例 | T05–T06、T38、T63、T74–T75 |
| C8/A6 审计与凭据保护 | T10、T64–T65、T80 |
| D1–D4 前端、配置、数据迁移与恢复 | T13–T17、T34–T37、T49、T66–T72 |
| D5 验证与门禁 | T01–T02、T73–T80及各任务验证列 |

本文件只拆解已批准范围；没有新增生产发布、逐设备VPN准入或外部身份平台。task.md 已获确认，当前进入 checklist.md；完整验收清单批准后才开始执行任务。
