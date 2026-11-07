# 网关 OIDC、RBAC/ABAC 与执行授权整改 Checklist

依据：[已批准 Spec](./spec.md)、[已批准 Plan](./plan.md)、[已批准 Tasks](./task.md)。

状态：已批准。用户已于 2026-11-07 指示“开始”，四份文档全部通过审批，进入实现；验收条目尚未执行。

## 验收方法与证据规则

本清单检查可观察行为，不以函数、文件的存在或逐行代码检查代替验证。重命名、移动文件且行为不变，不影响条目成立。

验证命令缩写如下，case 使用 task.md 规定的待新增测试标识，不能当成已经存在或已经通过的测试：

- U(case)：`cargo test -p auth --lib case -- --nocapture`。
- I(case)：`cargo test --test mod cobotusr::routes::security::case -- --nocapture`。
- S(case)：`cargo test --test mod cobotsrv::case -- --nocapture`。
- G(case)：`cargo test -p cobotlet --lib case -- --nocapture`。
- 浏览器验证：使用隔离 IdP、云端和模拟设备，记录操作、脱敏网络结果及设备请求计数；不向真实设备下发控制指令。

每项在 evidence.md 记录编号、环境、提交版本、执行命令/操作、实际结果、退出码/用例数及脱敏证据。过滤结果为0、ignore、依赖缺失、数据库不可达、尚未实现或只做源码推断，都不能勾选通过。失败项保持未勾选并记录预期、实际及修复结果；修复后重跑受影响项。

## 身份与凭据边界（AC1）

- [ ] C01：相同 subject 字符串的用户、机器、设备及不同 issuer 不会被当作同一身份。（验证：U(security_t03)，交叉凭据访问被拒绝。）
- [ ] C02：错误签名、算法、issuer、audience、用途、到期或未知登记凭据不能访问受保护资源。（验证：U(security_t07)、I(security_t18)，错误凭据均失败、合法凭据正常。）
- [ ] C03：普通API仅携带 query access_token 时不能登录；ID Token、refresh、许可和设备签名消息不能冒充普通访问令牌。（验证：U(security_t08)、U(security_t09)，返回认证失败且无副作用。）
- [ ] C04：业务受众凭据不能管理IdP；MCP长期凭据不能创建/轮换长期管理凭据；设备凭据不能调用用户管理或控制接口。（验证：I(security_t26)、I(security_t32)、G(security_t52)，各类混用均拒绝。）
- [ ] C05：安全服务分别验证机器身份和主体证据，不接受自报subject/application替换；不同机器的代理应用不能混用。（验证：I(security_t28)、I(security_t29)，合法绑定成功、越界绑定拒绝。）
- [ ] C06：IdP安全服务能在不向自身发HTTP安全请求的情况下完成机器认证和授权。（验证：I(security_t28)，拦截并计数其出站安全请求为0，合法请求仍成功。）

## 生命周期、吊销与并发重放（AC2、AC3）

- [ ] C07：本地登录/注册/刷新与OIDC登录产生的凭据均可统一检查和吊销。（验证：I(security_t24)、I(security_t77)，没有未登记但仍可用的登录路径。）
- [ ] C08：注销当前会话终止其访问/刷新能力，但不悄悄终止独立MCP授权；撤销应用授权则终止该应用下全部对应授权根。（验证：I(security_t25)，另一个无关会话/应用不被误撤销。）
- [ ] C09：删除或轮换MCP凭据后，旧凭据及其待执行委托在最迟10秒后不能读取、执行或申请新凭据。（验证：I(security_t27)、S(authorization::security_t74)，同时尝试MCP、REST和网关操作。）
- [ ] C10：授权码并发兑换最多一次成功；错误verifier/client/redirect不签发令牌，也不提前消耗合法授权码。（验证：I(security_t21)，核对成功数与后续正确兑换结果。）
- [ ] C11：普通refresh并发使用不会产生多个有效替代；签名或提交失败不留下可用的额外凭据。（验证：I(security_t22)，观察令牌有效性与事务回滚结果。）
- [ ] C12：复用已消费refresh被拒绝并终止正确刷新族，不影响其他会话；敏感值不以明文进入业务存储。（验证：I(security_t23)、I(security_t16)，查询隔离库并检查凭据有效性。）
- [ ] C13：MCP并发轮换最多一次成功，新凭据不因旧凭据被撤销而立即失效。（验证：I(security_t27)，检查新旧凭据的实际访问行为。）
- [ ] C14：撤销接口只有提交成功后返回成功回执；提交失败和断网不会显示“服务端已完成撤销”。（验证：I(security_t25)及浏览器退出故障场景，分别观察服务器与UI结果。）

## RBAC、ABAC 与管理边界（AC4、AC5、AC9）

- [ ] C15：多个重叠规则中任一匹配显式拒绝优先于所有允许，普通用户与超级管理员均受约束。（验证：U(security_t11)、I(security_t20)，变换规则顺序仍拒绝。）
- [ ] C16：用户显式允许可补足角色未授予的操作，但不能突破凭据用途、资源范围或系统约束；无授权时默认拒绝。（验证：U(security_t12)，覆盖允许与拒绝两条路径。）
- [ ] C17：角色新增权限不自动扩大旧受限凭据上界；撤销角色或修改资源范围在10秒内影响全部实例。（验证：S(authorization::security_t74)，记录变更提交与最后一次允许时间。）
- [ ] C18：必要资源字段缺失、编码歧义和网关/设备范围不匹配不能取得授权。（验证：U(security_t04)、I(security_t20)，包括网关A授权尝试设备B。）
- [ ] C19：列表、详情、工具provider和可见计数不泄露未授权资源；分页不会恢复被过滤对象。（验证：S(authorization::security_t39)，比较受限与管理员视图。）
- [ ] C20：普通用户不能修改角色、覆写权限、公钥或服务绑定；合法管理员仍可完成已授权管理操作。（验证：I(security_t32)，分别检查拒绝与成功。）
- [ ] C21：其他客户端持有正确secret也不能向未获绑定的应用写权限目录；同步目录不等于获得角色分配权。（验证：I(security_t33)，越应用与越操作请求拒绝。）
- [ ] C22：新增或遗漏授权登记的保护路由默认拒绝；公开清单以外的调试接口不匿名开放。（验证：I(security_t34)，临时漏登记的测试路由不能通过。）
- [ ] C23：普通有效JWT不能报废/回收EasyTier设备或查看受限MCP管理数据；相关安全配置缺失时不开放受影响能力。（验证：S(authorization::security_t48)，包含未启用和已启用feature配置。）

## 多入口一致性（AC6）

- [ ] C24：同一主体操作同一设备，通过REST、MCP、代理和网关直达获得一致的业务授权结果。（验证：S(authorization::security_t73)，分别覆盖允许及拒绝。）
- [ ] C25：只有mcp:access的用户可进行获准的协议连接，但不能执行设备工具，也看不到无权访问的provider。（验证：S(authorization::security_t46)、S(authorization::security_t73)，设备执行计数为0。）
- [ ] C26：只有remote:proxy不能调用下游管理/控制操作；未知method/path映射默认拒绝。（验证：S(authorization::security_t47)，模拟目标服务收到的越权请求为0。）
- [ ] C27：网关HTTP与MQTT均拒绝无许可裸任务、错目标或过期命令；不会回落到匿名standalone控制模式。（验证：S(gateway::security_t59)、S(gateway::security_t70)，设备执行计数为0。）

## 设备身份与服务目标（AC7）

- [ ] C28：未认证设备不能注册/上报；设备A不能以注册、心跳、能力更新或注销操作覆盖设备B。（验证：G(security_t52)，对每种操作分别发送合法与越权请求。）
- [ ] C29：合法且明确允许的局域网/loopback服务可使用；未允许的origin、IP、端口、路径拒绝。（验证：G(security_t53)，不把所有内网目标一律视为合法或非法。）
- [ ] C30：DNS结果变化、跨目标重定向和自由覆盖服务地址不能访问被禁止服务；未获准头不透传。（验证：G(security_t54)，观察两个模拟服务的连接及请求头。）
- [ ] C31：设备绑定或目标版本未确认、授权后发生变化时，旧许可不能在新目标执行。（验证：S(gateway::security_t55)，原目标与新目标均无错误派发。）
- [ ] C32：签名身份、topic、设备归属、流版本或序号不匹配的状态事件不能覆盖可信新状态；清除retained空消息不误标离线。（验证：S(gateway::security_t62)，观察最终设备状态。）

## 任务、执行领取和可信结果（AC8）

- [ ] C33：调用者不能读他人任务结果；相同幂等键只在对应owner/application内生效，异摘要请求返回冲突。（验证：S(execution::security_t37)、S(execution::security_t40)，原任务内容不变。）
- [ ] C34：原始用户Bearer不进入任务、outbox、MQTT命令或审计；同任务最多得到一份不可延期许可。（验证：S(execution::security_t40)、I(security_t30)，检查存储/消息及许可jti/期限。）
- [ ] C35：签名载荷任何生效字段被修改后拒绝执行，包括目标、输入、头、超时和能力版本。（验证：U(security_t09)、G(security_t56)，模拟设备不收到篡改指令。）
- [ ] C36：许可已签发但云端激活事务失败时，准备任务无法领取；不能重签或续期同任务绕过。（验证：S(execution::security_t41)、S(execution::security_t76)，设备执行计数为0。）
- [ ] C37：同任务跨实例并发领取只有首请求获得Acquired；同executor重试、查询与响应丢失重试都不再返回派发资格。（验证：S(execution::security_t42)、G(security_t57)，统计Acquired数量及请求计数。）
- [ ] C38：同设备互斥在多个执行实例间有效；取得设备失败立即停止，旧claim不能释放新任务占用。（验证：G(security_t58)、S(execution::security_t42)，并发执行不穿透。）
- [ ] C39：首次领取成功但响应丢失、本地写入失败或DispatchMarked后崩溃时，不会恢复旧任务重新发送设备请求。（验证：G(security_t57)、G(security_t60)、S(execution::security_t76)，每个故障窗口计数最多1。）
- [ ] C40：设备调用超时/断线标记Unknown；不自动转Pending、不超时接管、不自动创建替代控制任务。（验证：G(security_t60)及浏览器Unknown场景，等待后设备计数不增加。）
- [ ] C41：删除展示历史、网关重启或达到容量上限不能删除仍有效的防重放事实并使旧任务重执行。（验证：G(security_t49)、G(security_t63)，重投原命令仍拒绝/返回已有状态。）
- [ ] C42：错误网关密钥、claim、实例或摘要的结果被拒绝；重复可信结果幂等确认，冲突结果不覆盖终态。（验证：S(execution::security_t43)，核对结果与设备占用未错误变化。）
- [ ] C43：设备许可过期后，原claim的可信迟到结果仍可结束Unknown，但不能产生新派发。（验证：S(execution::security_t44)、S(gateway::security_t61)，结果归属正确、设备计数不增加。）
- [ ] C44：结果回报失败时只重报结果，跨实例读取从共享事实得到一致结果，不依赖原实例等待器。（验证：S(gateway::security_t61)、S(execution::security_t45)，关闭原请求实例后仍可读取。）
- [ ] C45：解除Unknown占用需要管理授权及核对证据，不能据此重新执行旧任务。（验证：S(execution::security_t44)，普通用户失败，合法核对后旧任务保持不可重派。）

## 新鲜度、预算、容量及故障（AC10、AC12）

- [ ] C46：决策有效期从主库查询开始计最多5秒，并受父凭据及原截止期限制；跨缓存/HTTP/MQTT转发不重置。（验证：U(security_t06)、S(authorization::security_t74)，覆盖延迟响应及排队过期。）
- [ ] C47：安全依赖故障时，可使用尚有效的结论；过期、畸形或缺必要字段的响应不能放行资源读取或新指令。（验证：S(authorization::security_t38)、S(authorization::security_t75)，检查协议错误和设备计数。）
- [ ] C48：跨主体、凭据、用途、受众、应用、操作、资源及事实版本的请求不复用不适用的Allow。（验证：S(authorization::security_t38)，逐一变更缓存键维度。）
- [ ] C49：连接池、并发排队、锁等待和多个安全调用共享2秒预算，外部deadline不能延长上限；设备业务耗时单独计量。（验证：U(security_t05)、S(authorization::security_t75)，记录单调时钟安全链耗时及协议失败。）
- [ ] C50：预热全部实例后撤销令牌/角色/范围，最迟10秒后各实例新请求均拒绝；切换实例不会恢复权限。（验证：S(authorization::security_t74)，保留提交及各实例判定时间证据。）
- [ ] C51：决策缓存不超过配置上限，未命中校验受并发上限约束；容量不足返回可识别错误，不提前删有效安全事实。（验证：S(authorization::security_t38)、G(security_t63)，观察容量与过载结果。）
- [ ] C52：时钟误差超出约束或无法确认同步时，节点健康失败且不开始安全操作；暂停后恢复重验期限。（验证：S(authorization::security_t75)、G(security_t60)，设备无过期执行。）

## 前端、审计与迁移（AC11、AC12）

- [ ] C53：业务与IdP管理PKCE流程的state/verifier/回调及令牌相互隔离，错state拒绝，管理凭据不扩大成全IdP管理权。（验证：浏览器执行T66场景并尝试越权管理。）
- [ ] C54：同受众并发过期请求只进行一次刷新，不同受众不混用refresh；正常刷新后原业务流程继续。（验证：浏览器执行T67，检查脱敏网络请求数量。）
- [ ] C55：用户可以创建、查看元数据、轮换和删除本人MCP令牌；完整秘密仅在签发响应展示一次，列表不返回秘密。（验证：浏览器执行T68闭环，旧令牌失效行为同时符合C09。）
- [ ] C56：凭据生命周期、权限变更、授权拒绝和设备执行可关联主体、资源、任务、结果和原因；回滚不留下假成功审计。（验证：I(security_t64)、S(execution::security_t64)，检查隔离审计记录。）
- [ ] C57：启用详细日志后，敏感URI/请求头/响应体/启动配置/数据库错误均不泄露完整token、密码、secret或私钥。（验证：S(authorization::security_t65)，使用测试秘密标记捕获输出。）
- [ ] C58：生产缺少必需issuer/受众/绑定/密钥/目标配置时不开放相关安全能力；开发放宽必须显式启用并可观察。（验证：S(gateway::security_t70)，分别运行生产和开发配置。）
- [ ] C59：单进程集群在IdP安全服务未就绪时不开放控制；就绪后合法调用恢复，秘密不出现在启动日志。（验证：S(gateway::security_t71)，延迟IdP启动并探测云端入口。）
- [ ] C60：空库安装、旧库升级及redb升级均可重复演练；旧令牌、缺owner历史记录和旧裸任务不会被默认为可信。（验证：I(security_t72)、S(gateway::security_t72)、G(security_t49)，比较升级前后访问与执行计数。）
- [ ] C61：迁移说明包含备份、维护窗口、旧客户端影响、重新登录/签发、身份和密钥登记、切换及恢复步骤。（验证：用隔离备份按migration.md演练，执行者可据说明完成并记录结果。）
- [ ] C62：失败恢复保留claim/去重/审计，不通过旧二进制重新接受旧凭据或重放任务；未在生产实际切换或向真实设备测试。（验证：隔离维护窗口故障演练和目标请求记录，保留恢复前后安全事实。）

## 端到端完整场景

- [ ] C63 合法操作闭环：交互登录 → 获得设备A权限 → 提交控制任务 → 模拟设备执行一次 → 验证签名结果 → 同主体跨实例读取；其他用户读取失败。（验证：S(authorization::security_t73)、S(execution::security_t45)及模拟设备计数，覆盖AC1、AC5、AC6、AC8。）
- [ ] C64 MCP撤销闭环：管理PKCE → 创建长期MCP凭据 → 允许的工具执行 → 删除凭据 → 等待生效窗口 → MCP/REST/代理/网关再访问及续签均不能绕过。（验证：I(security_t77)、S(authorization::security_t74)及浏览器T68，覆盖AC2、AC6、AC10。）
- [ ] C65 重放与故障闭环：同一控制命令并发投递到HTTP和MQTT → 注入领取/设备响应丢失 → 重启及重复回报 → 任务保持Unknown或被原claim结果确认，设备调用最多一次。（验证：S(execution::security_t76)、G(security_t60)、S(gateway::security_t61)，覆盖AC3、AC8、AC12。）
- [ ] C66 授权变更闭环：两个服务实例预热允许 → 修改资源范围并撤销角色 → 10秒后分别通过四入口访问 → 全部拒绝，中心故障也不使用过期Allow。（验证：S(authorization::security_t74)、S(authorization::security_t75)，覆盖AC4、AC5、AC10、AC12。）

## 编译、测试和质量门禁

- [ ] C67：核心库单元测试实际执行并通过。（验证：`cargo test -p auth -p cobotusr -p cobotsrv -p cobotlet --lib`，记录数量，无本次必需用例被跳过。）
- [ ] C68：路由、事务和跨服务测试实际注册并通过。（验证：`cargo test --test mod -- --test-threads=4`，同时用`-- --list`核对新增测试存在。）
- [ ] C69：两个受影响前端类型检查通过，必要页面可完成上述浏览器场景。（验证：分别执行`pnpm --filter @cobotweb/cobotsrv exec vue-tsc -b`与`pnpm --filter @cobotweb/cobotusr exec vue-tsc -b`。）
- [ ] C70：前端产物和完整workspace构建兼容，webui嵌入文件就绪。（验证：`pnpm -r --filter './cobotweb/apps/*' build`后执行全feature质量门禁，不把缺产物当作安全测试通过。）
- [ ] C71：格式检查通过。（验证：`cargo fmt --all -- --check`。）
- [ ] C72：所有目标及feature的静态检查无警告。（验证：`cargo clippy --workspace --all-targets --all-features -- -D warnings`。）
- [ ] C73：完整workspace各目标与feature测试通过。（验证：`cargo test --workspace --all-targets --all-features -- --test-threads=4`，保留平台及环境证据。）
- [ ] C74：文档测试通过。（验证：`cargo test --workspace --doc`。）
- [ ] C75：依赖使用及已知漏洞检查完成，无未解决的必需门禁失败。（验证：`cargo machete`、`cargo audit`；保留漏洞库时间与结果，工具/库不可用标阻塞。）
- [ ] C76：evidence.md逐项记录C01–C76的实际结果；失败、未验证、历史基线与新增问题清晰区分，必需项无遗漏。（验证：对照清单和测试输出核查；交付不宣称未经运行的场景通过。）

## 需求覆盖表

| Spec验收标准 | 检查项 |
|---|---|
| AC1 身份与凭据 | C01–C06、C63 |
| AC2 生命周期及吊销 | C07–C09、C14、C64 |
| AC3 并发与防重放 | C10–C13、C34–C41、C65 |
| AC4 权限优先级 | C15–C17、C66 |
| AC5 资源隔离 | C18–C19、C33、C63、C66 |
| AC6 入口一致性 | C24–C27、C63–C64 |
| AC7 设备接入及目标 | C28–C32 |
| AC8 任务及结果 | C33–C45、C63、C65 |
| AC9 管理边界与默认拒绝 | C20–C23 |
| AC10 变更、缓存与故障 | C46–C52、C64、C66 |
| AC11 审计及脱敏 | C56–C57 |
| AC12 非功能与迁移 | C46–C55、C58–C62、C65–C76 |

N1对应C58–C59；N2对应C09/C17/C50；N3对应C47；N4对应C49/C51；N5对应C37/C44/C50；N6对应C53–C55/C60–C62；N7对应C14/C56–C57；N8对应C63–C76。

## 审批与验收报告

四份文档均已完成审批，按task.md开始开发。条目只有取得实际运行证据后才能勾选，文档审批不代表实现通过。

最终报告包含：通过数/总数、未通过与未验证条目、对应命令和行为证据、故障注入结果、端到端结果、迁移影响及剩余风险。所有必需项实际通过后才能宣布本次整改完成。
