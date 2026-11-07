# 网关 OIDC、RBAC/ABAC 与执行授权整改 Plan

依据：[已批准的 Spec](./spec.md)。

状态：已批准。全部技术设计段落及完整汇总稿均已于 2026-11-07 获得用户确认，进入 task.md 任务拆解阶段。

## 架构概览

采用“中心决策、入口执行、网关复核”的架构。保留 Rust workspace、现有 OIDC 服务、PostgreSQL 与网关 redb，在应用层补齐身份和授权证据；保持现有网络传输技术。

### A1 共享认证与授权契约：libs/auth

统一表示用户、机器、设备主体以及签发方、受众、令牌用途、凭据标识、会话和授权范围。入口只提取一次凭据，验签后生成可信认证上下文，后续中间件与业务服务复用该上下文，禁止从自由填写的请求字段构造主体。

共享层提供凭据验证、明确的授权结果与错误分类、请求安全校验预算、日志脱敏及纯规则判定契约。数据库访问和网络调用由服务内的适配器承担；共享库不直接依赖任一业务服务。

普通 API 使用 Authorization Bearer，不继续接受 URL 查询参数中的长期令牌。需要浏览器流式连接的调用方按受限连接凭据迁移。内嵌角色和权限名称不能单独替代当前凭据状态及资源授权检查。

覆盖：F1、F4、F10、F11。

### A2 身份及权限事实来源：cobotusr

负责用户/机器/设备凭据的签发、状态与用途校验、吊销、会话结束，以及授权码、刷新凭据和 MCP 长期令牌的原子消费与轮换。安全状态持久化到 PostgreSQL；成功响应发生在事务提交之后。

保留两套现有权限数据的职责：URI authority 管理 IdP 的管理 API，application permission 管理业务资源。通过适配器使用一致的判定顺序：系统约束 → 全部匹配的显式拒绝 → 用户允许 → 角色允许 → 默认拒绝。规则不存在或必要资源信息缺失时拒绝。多个匹配权限中存在显式拒绝时，不能被另一条允许规则覆盖。

IdP 管理入口只接收对应受众与用途的凭据。资源服务器通过受限机器身份调用专用安全服务入口；原始用户凭据或受限委托凭据作为待验证的主体证据传递，不能充当机器身份。中心从已验证证据派生主体，并检查调用服务与应用的授权绑定，不能信任请求任意指定的 subject/application。

IdP 自身的状态检查与决策直接调用本地服务及数据库，避免为了验证自身请求再次调用自身 HTTP/PDP。机器客户端的权限同步目标在服务端显式登记。

覆盖：F1–F4、F9、F10。

### A3 云端业务授权与任务入口：cobotsrv

REST、MCP 与远程代理作为协议适配器，共用业务授权和任务服务。服务先解析实际操作及可信资源，选择并固定目标网关和设备，再做资源级授权；不得授权工具名称后任意替换执行设备。

MCP 连接、工具可见性和工具执行分别检查。远程代理既检查代理范围，也要求下游操作授权。资源列表按可见范围过滤；任务结果读取校验归属或明确的管理权限。

任务创建前持久化调用主体、目标、请求摘要与幂等关系。任务及结果以 PostgreSQL 为多实例共享事实来源，本地等待器和缓存只用于加速。任务执行占位使用共享的原子状态迁移，避免同一网关身份的多个执行实例各自重复派发。

覆盖：F5、F6、F8、F9。

### A4 网关执行与设备接入：cobotlet

HTTP 本地入口和 MQTT 消息入口统一进入认证、授权及执行服务。网关校验面向自身、用途受限且绑定主体、网关、设备、操作、任务及请求摘要的执行凭证，并在实际派发前检查其有效期和必要的当前授权状态。直接访问网关也受同样的业务权限约束。

设备接入凭据绑定具体设备；设备注册、心跳、能力修改与注销均检查该绑定。设备服务目标遵循管理员配置的地址、协议和端口规则，网络连接使用经过检查的目标，重定向不得绕过限制，转发头遵循允许清单。

网关在发送设备指令前，取得共享执行占位并写入本地持久去重状态；同一任务跨 REST/MQTT、重投递、进程重启和多实例只能取得一次派发资格。设备互斥取得失败必须终止派发。

执行中崩溃或响应丢失、无法确定实际结果时，记录“结果未知”，不自动重发控制指令。此处保证同一任务不重复派发；物理动作是否已经发生须据设备回执确认。用户显式创建的新任务是新的操作，不能伪装为原任务重试。

删除任务展示历史不能删除仍有效的防重放状态；执行凭证到期后不得通过重新签发同一任务来重新取得执行资格。

覆盖：F5–F8、F10。

### A5 传输上的身份与完整性

HTTP 与 MQTT 传输携带同一套可验证的应用层身份和任务绑定信息。执行许可由受信任的签发方签名，接收方校验目标和请求摘要；结果使用绑定网关身份的独立签名证明，绑定原任务及请求摘要。设备状态事件也校验来源网关与设备归属。

Topic、alias、task_id 和消息正文中的主体字符串只用于定位，不独立构成身份凭据。消息与其已登记任务、签名来源或目标不匹配时拒绝接收，不写入可信任务结果或设备状态。

这部分增加应用消息封装、身份绑定和校验，不重构 MQTT、Net4MQTT 或 EasyTier 协议，也不实施逐设备 VPN 准入。

覆盖：F1、F7、F8。

### A6 审计、迁移与验证

统一记录认证/授权结果、凭据与权限变更、任务派发和设备结果，关联主体、目标、任务与请求追踪信息；日志入口统一脱敏。公开接口和受保护操作建立明确清单，防止新增路由静默放行。

迁移覆盖凭据用途和受众、机器与设备绑定、旧权限规则、任务归属数据、配置及必要前端调用适配。历史数据无法证明来源或归属时，不自动赋予可信身份或访问权限。部署说明包含升级顺序和影响清单，开发放宽配置须显式启用。

验证使用模拟 IdP 故障、隔离数据库、测试 MQTT broker、模拟设备和多实例服务；实际未运行的检查保持“未验证”，不引用源码推断作为运行通过证据。

覆盖：F9、F11、N1、N6–N8。

### 全链路时限与故障约束

- 认证、吊销与权限结论从主数据库的一致状态取得；不依赖复制延迟无上界的读副本。
- 安全结论携带源头确定的绝对有效截止时间，各层缓存和执行凭证继承该期限，不在转发或读取缓存时重新计时；配置上界须留出时钟与处理余量，确保变更提交后 10 秒内对所有实例生效。
- 同一请求的完整安全校验共享 2 秒预算，包括连接池、并发排队、数据库锁等待和安全服务调用。业务设备执行时长单独计量；异步任务在派发前重新校验时，不得延长旧结论的有效期。
- 缓存严格按主体、凭据、用途、应用、操作和资源隔离，并受凭据到期及决策截止时间共同限制；容量有上限。
- 安全依赖不可用且没有有效结论时拒绝访问与新指令下发；认证失败、明确拒绝与依赖不可用使用不同的内部原因和可识别的协议错误。

覆盖：N2–N5、F10。

### 主调用链

用户/机器请求 → 入口认证上下文 → 解析并固定目标资源 → 中心状态与资源决策 → 持久化任务及绑定的执行许可 → 网关验证与派发前复核 → 原子执行占位和持久去重 → 设备调用 → 验证来源和任务绑定的结果 → 按访问权限返回。

设备注册/心跳 → 网关验证设备身份与归属 → 目标规则及变更权限检查 → 本地持久化 → 发布可验证来源的状态事件 → 云端核对网关及设备绑定后更新。

IdP 管理请求 → 验证专用受众、用途及当前凭据状态 → 本地 URI 操作授权与资源约束 → 原子修改 → 事务提交 → 审计和成功响应。

## 核心数据结构与接口

本节给出拟实现的 Rust 类型、存储约束与接口契约。类型名和签名用于技术设计，不代表已经存在的实现；本阶段仅修改文档。

### B1 主体、凭据用途与认证上下文

| 类型 | 字段或变体 | 约束 |
|---|---|---|
| `PrincipalKind` | `User / Machine / Device` | 网关是绑定 gateway_id 的机器主体；机器与设备不转换成用户 ID。 |
| `PrincipalRef` | `issuer, kind, subject` | 三者共同构成主体身份；不同 issuer/kind 下相同 subject 不是同一主体。 |
| `CredentialRef` | `issuer, jti` | 唯一标识已登记凭据；不通过 jti 的存在与否猜测令牌类型。 |
| `TokenUse` | `UserSession / McpAccess / SecurityService / PermissionSync / DeviceAccess / GatewayOperation / StreamConnect` | 用途与权限范围分别校验；ID Token、刷新凭据和签名设备消息都不是普通 API Bearer。 |
| `CredentialBounds` | 允许的 `application, actions, resource_selectors` | 是签发时的权限上界；不是当前权限的替代品。未知或缺失的必要边界不补成通配允许。 |
| `IdentityBinding` | `None / Gateway(gateway_id) / Device(gateway_id, device_id)` | 绑定由管理员登记与可信签发结果产生；设备不得自行选择其他绑定。 |
| `AuthenticatedContext` | `principal, credential, token_use, audience, application, bounds, binding, expires_at, evidence` | 仅由可信验证器构造；evidence 使用脱敏秘密类型，禁止 Debug/日志/普通响应序列化泄露。 |

凭据用途与受众矩阵：

| 用途 | 主体及受众 | 允许的入口与限制 |
|---|---|---|
| `UserSession` | 用户；`cobotusr` 或 `cobotsrv`，每枚令牌只指定一个受众 | 对应服务的交互/管理或业务 API；访问 IdP 需独立的 cobotusr 受众凭据，不能直接复用 cobotsrv Bearer。 |
| `McpAccess` | 用户；`cobotsrv` | 仅 MCP 操作集；仍须通过当前工具/设备权限，不能用来创建/刷新长期管理凭据或调用普通 REST 管理接口。 |
| `SecurityService` | 已登记机器；`cobotusr:security` 或 `cobotsrv:execution` | 前者用于受限中心决策，后者用于网关执行领取与回报；分别签发、分别校验应用和网关绑定。 |
| `PermissionSync` | 已登记机器；`cobotusr:permission-sync` | 仅同步服务端绑定的应用权限目录，不授予用户管理或任意应用代理能力。 |
| `DeviceAccess` | 设备；`cobotlet:<gateway_id>` | 仅本设备接入、心跳及获准的能力变更；不授予用户任务执行或网关管理能力。 |
| `GatewayOperation` | 用户/机器的受限委托；`cobotlet:<gateway_id>` | 仅执行被签名绑定的一次具体请求；直接访问网关的用户读写也通过该凭据携带原主体证据。 |
| `StreamConnect` | 用户；指定服务与连接端点 | 只有现有客户端确需 URL 连接票据时启用；一次兑换、短期、仅用于绑定的流连接，不能提交工具或换取其他凭据。 |

JWT 签名、issuer、单一 audience、token_use、凭据登记状态与到期均须匹配；用途相关的签名类型也必须匹配。普通 access JWT 使用明确的 access 类型，网关操作许可与网关签名事件使用独立类型，禁止互相作为 Bearer 使用。可接受的 audience 来自服务配置与登记关系，不由请求头 Host 或任意 URL 推导。

机器和设备使用既有 client_credentials 能力与明确的身份登记绑定，不增加 OAuth device-code 登录协议。`client_credentials` 从客户端认证及主库登记建立身份；IdP 安全入口直接本地校验该机器凭据，避免递归 HTTP 验证。

### B2 凭据登记、撤销根与原子轮换

IdP 新增统一的 `CredentialRecord` 安全索引，原有 oauth_token/mcp_token 记录保留必要兼容展示职责；迁移后安全判定只采用统一索引，未知凭据默认无效。

| 模型 | 必要字段与约束 |
|---|---|
| `CredentialRecord` | `credential_ref, principal, token_use, audience, application, bounds, binding, grant_id, session_id?, family_id?, revocation_parent?, minted_by?, expires_at, revoked_at?, consumed_at?, replacement_id?`；`(issuer,jti)` 唯一。 |
| `GrantRecord` | `grant_id, principal, application, grant_kind, revoked_at?`；长期 MCP 使用独立授权根。 |
| `SessionRecord` | `session_id, principal, revoked_at?`；交互登录及其刷新族关联同一会话。 |
| `RefreshFamilyRecord` | `family_id, session_id, grant_id, revoked_at?`；刷新值只存安全摘要，旧值重用可定位并撤销对应族。 |
| `ServiceBinding` | `client_id, token_use, allowed_applications, allowed_audiences, allowed_operations, identity_binding, enabled`；权限同步代理关系和网关服务身份必须显式登记。 |
| `GatewaySigningKey` | `gateway_id, kid, public_key, enabled, valid_from, valid_until`；公钥绑定由管理权限维护，消息提供的任意公钥/地址不受信任。私钥留在对应网关。 |

关系规则：

- `minted_by` 只记录签发审计来源。新 refresh/MCP 替代凭据关联稳定的会话、族和授权根，不把被消费的旧凭据当作继续有效的父节点。
- 网关操作许可的 `revocation_parent` 指向原用户/机器凭据；其有效性还依赖相应授权根。仅支持这一层受限委托，拒绝委托再委托及循环依赖。
- 注销当前会话撤销该 session、刷新族和关联访问凭据；独立 MCP 授权根不随普通浏览器退出自动撤销。
- 撤销应用授权撤销该用户在该应用下的所有授权根，包括长期 MCP；单枚 MCP 删除只撤销该凭据及其待执行委托。MCP 轮换撤销旧凭据并签发同一稳定授权根下的新凭据。
- MCP 新建和轮换要求有效且获准的交互管理凭据，不接受 MCP 长期凭据自身作为管理登录。
- 授权码及 refresh/MCP 轮换通过事务内条件消费或行锁实现；只有成功取得旧记录的那次事务能提交替代记录。普通 refresh 轮换同时使对应旧 access 凭据失效，新的 access/refresh 仍关联活动会话及族。
- refresh 已消费值的再次使用拒绝，并撤销对应刷新族及派生访问凭据；这属于重放后的防护，不向请求方返回第二枚替代令牌。

### B3 资源与授权请求

| 类型 | 字段或语义 |
|---|---|
| `OperationId` | 已登记操作名称，如 device:read、task:execute、task:read、remote:proxy；IdP 管理 API 由已登记的 method + route 模板映射为操作。 |
| `ResourceRef` | `application, kind, gateway_id?, device_id?, tool_name?, task_id?, route_id?`；每种 kind 定义必填字段，缺失拒绝；线上的规范标识采用版本化、无歧义编码，不直接拼接未经转义的别名。 |
| `ResourceFacts` | `owner?, capability_revision?, target_binding_revision?`；只允许由对应资源服务的存储或可信设备事件产生。任务 owner 不从客户端传参取得。 |
| `AuthorizationRequest` | `subject_evidence, operation, resource, facts, request_binding?`；不提供自由指定 subject 的字段。业务 application 必须同时匹配凭据及调用机器获准代理的应用。 |
| `AuthorizationDecision` | `decision_id, effect, reason_code, checked_at, valid_until, credential_ref, resource, bounds_fingerprint`；effect 为 Allow/Deny，依赖故障通过错误返回，不伪造 Deny 或 Allow。 |
| `PolicySnapshot` | 当前主体、角色、全部匹配允许/拒绝、凭据上界和必要资源约束；在主库一致快照中获取，纯判定器不进行 I/O。 |

最终允许必须同时满足：当前凭据及撤销根有效 ∩ 签发时的授权上界 ∩ 当前 RBAC/ABAC ∩ 当前资源和系统约束。对所有匹配规则先检查显式拒绝，再查用户允许和角色允许；超级管理员仅在系统约束内获得角色允许，不跳过显式拒绝。角色后来新增的权限不自动扩张已签发的受限凭据上界。

业务资源使用 `application permission`，IdP 管理使用 `URI authority` 适配器；两者映射到相同判定契约。仅属于自己任务的读取是资源约束；读取他人任务需要单独登记并授予的管理操作，普通 task:read 不自动包含该权限。

资源列表按实际候选资源批量检查，批次最多 100 个；过滤后才形成返回页面及可见计数。工具聚合只包含获准的 provider，不能因为一个 provider 可见就暴露全部设备。

### B4 网关操作许可、指令与结果

`GatewayOperationPermit` 是 IdP 签发的专用签名凭据，包含：

- 标准身份字段：`iss, aud, jti, iat, exp, token_use=GatewayOperation`。
- 委托关系：`actor, presenter_service, parent_credential, grant_id, application`。
- 请求绑定：`request_id, operation, resource, request_digest, capability_revision?, target_binding_revision?`。
- 执行约束：控制任务必须包含 `task_id`；所有请求包含不可续期的 `authorization_valid_until`，exp 不得晚于该时间及父凭据到期。

`request_digest` 使用 SHA-256，覆盖版本化请求载荷的完整确定字节，包含实际网关/设备/工具、操作、能力和目标绑定版本、输入、允许转发的头及设备执行超时。云端生成后保持同一份字节，通过 HTTP/MQTT 传递，不在各跳解析后重序列化计算摘要。网关必须按这些已验证字节解析执行，不能再从未绑定字段取参数。

`CommandEnvelope` 包含 `version, permit, payload_bytes`。令牌与载荷的目标、摘要、当前登记的能力及服务目标版本必须匹配；版本变更后旧请求拒绝执行。能力的实际服务地址由获准设备登记解析，不接受客户端自由覆盖。代理请求也必须映射到已登记的操作和具体 method/path，请求体及生效头绑定在摘要内，未知操作默认拒绝。

网关结果和设备状态采用 `SignedGatewayMessage`，签名覆盖完整消息；字段分为：

| 部分 | 字段或约束 |
|---|---|
| 公共头 | `version, kind, gateway_id, kid, event_id, issued_at, message_expires_at`；kind 区分 TaskResult、DeviceSnapshot、Heartbeat、Offline。 |
| 任务结果 | `task_id, claim_id, executor_instance_id, request_digest, outcome, result_payload`。 |
| 设备事件 | `device_id, binding_revision, event_stream_id, sequence, state_payload`；流标识和序号登记在可信服务中，防止旧状态重放覆盖新状态。 |

首轮沿用已有 RS256 能力，IdP 操作许可与逐网关签名密钥分开。消息签名密钥通过登记的 gateway_id/kid 查询，校验当前启用状态与有效期，拒绝消息指向任意 key URL。传输 topic、设备归属和签名身份必须一致。消息不能作为普通访问令牌使用。

任务结果逐项匹配 PostgreSQL 已保存的 gateway_key_id、claim_id、executor_instance_id 和 request_digest；更新终态及释放设备均以当前 claim 为条件。重复结果可以确认已接收，不同结果不能覆盖已确认终态或释放另一执行的设备占用。

设备执行许可过期限制的是开始派发，不代表设备必须在 5 秒内执行完。有效执行的迟到结果使用新签名消息回报，匹配既有 claim；不能以延长原许可的方式重启任务。

### B5 任务状态、执行领取与持久去重

| 类型 | 必要字段 |
|---|---|
| `TaskRecord` | `task_id, owner, application, gateway_id, device_id, operation, request_digest, payload_bytes, credential_ref, permit_jti, authorization_valid_until, idempotency_key?, state, claim_id?, result?`。 |
| `ExecutionClaim` | `claim_id, task_id, permit_jti, gateway_id, device_id, gateway_key_id, executor_instance_id, request_digest, start_before`。 |
| `ReplayRecord` | `task_id, request_digest, permit_jti, claim_id?, disposition, retain_until`；与任务展示历史分别保存。 |
| `TaskOutcome` | `Succeeded / DeviceRejected / RejectedBeforeDispatch / Unknown`，附脱敏结果或原因；设备 HTTP 超时/断线可能已发生副作用，归入 Unknown。 |

任务 ID 由服务生成且全局唯一；调用方的重试标识作为 owner/application 范围内的幂等键，不直接覆盖任务 ID。同一幂等键的不同请求摘要返回 Conflict，相同摘要返回已登记的任务引用，但读取结果仍重新授权。

云端 PostgreSQL 状态机：

```text
Pending → Claimed → Succeeded / DeviceRejected
   │         │
   │         └→ Unknown → 同一 claim 的有效迟到结果 → Succeeded / DeviceRejected
   └→ RejectedBeforeDispatch
```

Unknown 不回到 Pending；超时不自动转移已领取的派发资格。只有明确的管理核对可结束未知状态并解除设备占用，不自动重发旧任务。用户明确创建新任务时须提示原任务状态不确定，并重新完整授权。

网关 redb 在调用设备前持久化 `DispatchMarked`；恢复时已有 Claimed/DispatchMarked 的记录进入 Unknown，不再次调用设备。重复收到同 ID 同摘要只返回状态/已有结果；不同摘要拒绝。只有当前请求首次取得的领取成功结果能使本次调用进入派发分支，磁盘历史不能重新生成这种资格。

领取接口结果：

| 变体 | 语义 |
|---|---|
| `Acquired(ExecutionClaim)` | 仅首次成功执行 Pending→Claimed 原子提交的该次请求获得。 |
| `AlreadyClaimed(ClaimStatus)` | 只返回查询状态；即使相同 executor_instance_id 重试，也不返回可再次派发的成功许可。 |
| `Final(TaskOutcome)` | 返回已登记结果，不能重新派发。 |
| `Rejected(ClaimRejection)` | 无效/过期许可、吊销、无权限、目标或摘要不匹配、设备占用、未知任务等。 |

PostgreSQL 同一事务校验任务目标、摘要、许可、网关身份和截止期，原子占用任务及设备。设备占用以 claim_id 为所有权，释放必须匹配该 claim，不能释放其他任务。独立 redb 不承担跨实例互斥。

领取事务成功但响应丢失，或已获 Acquired 后本地持久化失败，均不能靠重试获得第二次派发资格；允许出现未实际派发但结果未知的任务，以满足至多一次派发。设备网络层关闭自动重定向和自动重试控制调用的行为，执行层不做失败重发。

每个任务最多签发一份不可延期的执行许可。任务 ID 和领取事实在 PostgreSQL 保留不可重用标识；网关防重放墓碑至少保留到原许可到期及允许时钟误差结束。删除任务历史只删除展示载荷，不删除仍需要的执行/幂等安全事实；到达容量上限时拒绝接收新的安全记录，不通过提前清理有效墓碑腾出空间。

### B6 服务接口与错误契约

以下为接口签名；实现中采用项目已有 async-trait 风格，依赖通过接口注入。

```rust
// libs/auth：同步纯验证/规则，与异步状态决策分离。
fn verify_credential(token: &SecretToken, expected: &CredentialExpectation)
    -> Result<VerifiedCredential, SecurityError>;
fn evaluate_policy(snapshot: &PolicySnapshot, operation: &OperationContext)
    -> PolicyEffect;
async fn authorize(context: &AuthenticatedContext, operation: &OperationContext,
    budget: &SecurityBudget) -> Result<AuthorizationDecision, SecurityError>;

// cobotusr：身份事实与中心决策。
async fn check_credential(credential: &VerifiedCredential, budget: &SecurityBudget)
    -> Result<CredentialStatus, SecurityError>;
async fn authorize_subject(caller: &ServiceContext, request: AuthorizationRequest,
    budget: &SecurityBudget) -> Result<AuthorizationResponse, SecurityError>;
async fn consume_authorization_code(request: CodeExchangeRequest,
    budget: &SecurityBudget) -> Result<TokenSet, SecurityError>;
async fn rotate_credential(manager: &AuthenticatedContext, request: RotationRequest,
    budget: &SecurityBudget) -> Result<TokenSet, SecurityError>;
async fn revoke_credentials(manager: &AuthenticatedContext, scope: RevocationScope,
    budget: &SecurityBudget) -> Result<RevocationReceipt, SecurityError>;

// cobotsrv：对所有入口共用的业务任务服务。
async fn prepare_task(context: &AuthenticatedContext, request: TaskInput,
    budget: &SecurityBudget) -> Result<TaskReference, SecurityError>;
async fn claim_execution(gateway: &ServiceContext, request: ClaimExecutionRequest,
    budget: &SecurityBudget) -> Result<ClaimExecutionReply, SecurityError>;
async fn accept_gateway_message(message: SignedGatewayMessage,
    budget: &SecurityBudget) -> Result<MessageReceipt, SecurityError>;
async fn read_task(context: &AuthenticatedContext, task_id: TaskId,
    budget: &SecurityBudget) -> Result<TaskView, SecurityError>;

// cobotlet：HTTP/MQTT 共用的入口；具体协议层不得直接调用设备 forward。
async fn handle_gateway_operation(command: CommandEnvelope, budget: &SecurityBudget)
    -> Result<GatewayOperationReply, SecurityError>;
async fn handle_device_operation(context: &AuthenticatedContext,
    operation: DeviceOperation, budget: &SecurityBudget)
    -> Result<DeviceOperationReply, SecurityError>;
```

专用服务端点的对应关系：

| 端点 | 调用方与核心契约 |
|---|---|
| `POST /internal/security/authorize`（IdP） | Authorization 是 SecurityService 机器凭据；正文携带 subject_evidence 及最多 100 个资源操作。中心从证据派生主体，返回逐资源结论；具体网关请求可同时申请绑定的操作许可，不能把列表结论作为执行许可。 |
| `POST /internal/security/validate-operation`（IdP） | 已登记资源服务/网关校验 GatewayOperation 证据、父凭据和当前资源权限；有效期不延长，过期许可拒绝。 |
| `POST /internal/execution/claim`（云端） | 绑定该网关的 cobotsrv:execution 机器凭据、操作许可及本实例标识；原子领取，重试只返回状态。 |
| `GET /internal/execution/tasks/{task_id}`（云端） | 绑定网关的机器凭据查询自身任务状态；不返回再次执行的许可。 |
| `POST /internal/gateway/messages`（云端） | HTTP 回报采用机器凭据和签名消息；MQTT 订阅适配器将签名消息交给相同 accept_gateway_message 服务校验。 |

公开业务路由继续保留已有用途，由模块设计明确各路由的操作映射和必要的客户端迁移。内部端点必须认证和授权，路径中的 internal 不代表可信。

`RevocationScope` 明确区分 Credential、CurrentSession、RefreshFamily、ApplicationGrants；`RevocationReceipt` 仅在提交后返回 committed_at 和最迟生效时间。签发/轮换响应中的秘密值只返回一次且禁止记录。

`SecurityError` 至少区分 InvalidCredential、CredentialRevoked、Forbidden、InvalidResource、Conflict、DependencyUnavailable、DeadlineExceeded、CapacityExceeded。HTTP 分别映射为 401、401、403、400、409、503、503、503；资源存在性需要隐藏时统一 404。MCP 使用等价的协议错误及内部 reason_code，不能在授权失败时返回执行成功。内部详细原因进入脱敏审计，客户端不获取凭据或无权查看的资源细节。

### B7 新鲜度、预算及缓存类型

| 类型/参数 | 确定规则 |
|---|---|
| `SecurityBudget` | 入口单调时钟截止时间，最大 2 秒；包含并发/连接池排队、锁等待、网络及状态检查。跨服务只接受缩短后的剩余预算，不能由外部输入延长本地上限。 |
| `DecisionValidity` | 主库安全查询开始时刻 checked_at + 最多 5 秒，与当前凭据、委托父凭据及已继承截止期取最小值。查询完成时已过期则拒绝结果。 |
| 时钟约束 | 安全节点时钟误差部署上限为 1 秒；过期检查预留该余量且不增加宽限。时钟同步不可确认或超出约束时停止安全操作并暴露健康失败。 |
| `DecisionCacheKey` | `issuer, principal_kind, subject, credential_jti, token_use, audience, application, operation, resource, facts_fingerprint, bounds_fingerprint`；键和值均不得包含明文秘密。 |
| 缓存界限 | 每实例安全决策缓存默认最多 10,000 项、并发未命中校验最多 64 个；同键请求合并；所有等待计入剩余预算。过期项不能因服务异常继续使用。 |

执行许可、状态检查和网关复核继承源头绝对截止时间。签名任务在消息队列中等待过久必须拒绝，不为同任务重新签发许可；异步执行前的校验使用独立的最多 2 秒处理预算，但不得重置消息携带的授权有效期。同步串联的安全步骤共同使用入口预算。

IdP 每次未命中校验直接从主库取一致快照，不在中心叠加额外正向缓存时长。收到新鲜度结论时不能按收到时间重新加 5 秒。该设计保留时钟与处理余量，满足批准的 10 秒全实例失效上限，不依赖 Redis 或失效广播保证正确性。

## 模块职责与交互

### C1 模块职责及依赖

| 模块 | 职责与对外接口 | 依赖 |
|---|---|---|
| 共享身份与验证 | 定义主体、用途、上下文、秘密类型；实现 verify_credential 和消息验签契约 | 可信密钥适配器、时钟；不依赖业务数据库 |
| 共享规则判定 | evaluate_policy；完整检查显式拒绝、授权上界和资源约束 | 已加载的 PolicySnapshot、OperationContext；无 I/O |
| IdP 凭据生命周期 | check_credential、授权码消费、轮换、撤销；兼容登录入口统一调用此服务 | IdP 主库、事务仓储、签名器 |
| IdP 决策与委托 | authorize_subject、操作许可签发和 validate-operation | 凭据生命周期、规则判定、服务绑定和权限仓储 |
| IdP 管理授权 | URI 操作映射、角色/覆写管理、客户端及密钥绑定管理、受限权限同步 | 本地凭据与规则服务；不向自身发 HTTP 请求 |
| 云端资源与授权 | 解析可信设备/任务/工具，选择并固定目标，构建 OperationContext，批量过滤资源 | 云端资源仓储、中心安全客户端 |
| 云端任务协调 | prepare_task、claim_execution、read_task；共享执行占位与结果持久化 | 任务事务仓储、云端授权服务、消息发送适配器 |
| 网关接入与执行 | handle_gateway_operation、handle_device_operation；身份绑定、目标校验、本地持久去重与派发 | 中心安全客户端、云端执行客户端、redb、受限设备 HTTP 客户端 |
| 可信消息接收 | accept_gateway_message；验签、归属/版本/摘要校验及条件状态更新 | 密钥绑定、任务和设备仓储 |
| 协议与审计适配 | HTTP、MCP、MQTT 解析及错误映射，脱敏审计与追踪 | 上述服务；协议层不能绕过授权直接调用设备 |

跨进程只使用明确的安全服务与执行服务接口，不共享数据库连接或直接修改另一服务的表。IdP 拥有身份和权限事实，云端拥有任务及设备目录事实，网关拥有本地执行事实。数据库适配器实现上层定义的仓储接口，纯规则层不依赖 SeaORM/redb。

### C2 路由到操作的映射

| 入口 | 必要检查 |
|---|---|
| REST /devices、/tools、/cobotlets | 对实际资源检查 device:read；列表和工具 provider 在聚合前过滤 |
| REST /tasks 提交 | task:execute + 固定的网关/设备/工具 + 凭据用途及资源上界 |
| REST /tasks/{id} | task:read + 任务归属；跨用户读取额外要求明确的管理授权 |
| MCP initialize、ping、流连接 | mcp:access + 对应用途；不附带设备执行能力 |
| MCP tools/list | mcp:access + 与 REST 相同的设备可见性过滤 |
| MCP tools/call | mcp:access + 与 REST 相同的 task:execute 和目标资源检查 |
| /remote 代理 | remote:proxy + 已登记 method/path 对应的下游业务操作；无映射拒绝 |
| 网关本地 HTTP /tasks | 接收已绑定的命令封装，进入统一执行流程；不接受裸任务 JSON 绕过许可 |
| 网关设备注册、心跳、变更、注销 | DeviceAccess 身份及具体设备绑定；管理覆写必须另有管理授权 |
| IdP 管理、MCP 管理及 EasyTier 管理接口 | 分别登记读取与修改操作，校验管理用途和具体资源；有效 JWT 本身不授予管理权 |

路由清单区分公开协议入口、认证设备入口、用户入口和机器入口。公开协议入口不等于无校验：token、revoke、introspect 等仍执行各自的客户端/凭据验证。仅健康检查、发现文档及公钥等明确登记的非敏感内容可匿名读取；调试 /error 不作为生产匿名入口。受保护路由缺少映射时默认拒绝，并通过路由覆盖检查发现遗漏。

### C3 登录、管理受众与生命周期交互

1. 将现有本地 /login、/register、/refresh、/logout 和 OIDC 授权码/刷新入口都接入凭据生命周期服务。旧 refresh_token 与 oauth_token 路径不能继续独立签发未登记凭据。
2. IdP 自有登录签发 aud=cobotusr 的 UserSession；业务 SPA 的授权码流程签发 aud=cobotsrv 的 UserSession。客户端对应的允许受众由服务端登记，禁止请求任意切换受众。
3. cobotsrv 前端管理 MCP 令牌时，使用单独登记的管理客户端及 PKCE 流获取 aud=cobotusr、管理操作及目标应用受限的凭据。它与业务凭据采用独立存储键、state、verifier、回调分流及刷新状态；不复用 cobotsrv Bearer，也不把机器 secret 放进浏览器。现有登录/同意页面复用，界面仅做必要适配。
4. 授权码兑换先认证客户端、验证 redirect_uri、到期、授权类型和 PKCE，再在事务中重新核对记录并条件消费，同时登记令牌及稳定授权根。失败校验不提前烧掉授权码；成功响应在提交之后返回。
5. refresh 与 MCP 轮换在同一事务中条件消费旧记录、创建替代记录并更新必要关联。锁等待和签名处理受安全预算限制；签名或提交失败不返回可用替代凭据。旧 refresh 重放触发对应族撤销。
6. 同一受众的前端刷新请求合并，避免多个并发业务请求各自轮换同一 refresh token。不同受众的刷新互不混用。响应丢失时不自动反复重放已消费凭据，可要求重新登录；安全失败不静默降级。
7. 注销和撤销授权在 IdP 主库提交后返回回执及最迟生效时间；前端区分“本地清除成功”和“服务端撤销失败”，不把网络失败显示为全局注销完成。Cookie 参与的会话变更校验受信 Origin 及防 CSRF 证据。

IdP 自身 URI 授权、机器凭据验证及 client_credentials 建立身份直接访问本地服务和主库，不依赖对自身安全端点的 HTTP 回调，避免认证递归。

### C4 创建任务、签发许可与发布

任务准备使用分步提交，不跨 IdP/云端数据库持有事务锁等待 HTTP：

1. 云端完成入口认证，解析并固定真实网关、设备、能力和目标绑定版本；生成规范载荷及摘要。
2. 云端事务登记不可执行的 TaskReservation：预留 task_id，绑定 owner、父凭据引用、目标、摘要及幂等键。它属于准备记录，尚不是可领取的 Pending 任务。原始用户令牌只在当前请求内存中短暂持有，不写任务表、消息或审计。
3. 以独立机器身份向 IdP 提交原主体证据和具体资源，取得当前授权及绑定的操作许可。IdP 按 application + task_id（非任务请求按 request_id）唯一登记许可；相同绑定的重试最多取回原许可，不能重新签发或延长。不同摘要、主体、父凭据或目标返回 Conflict。
4. 云端验证返回绑定及剩余有效期，在本地事务中创建可执行的 Pending 任务、绑定 permit_jti 和原载荷，并写入待发布记录。只有这次提交成功后才能发送命令。
5. 发布器读取待发布记录，将相同 CommandEnvelope 通过 MQTT 或 HTTP 发出；重复传输不创造新的任务和许可。已过期记录直接结束为派发前拒绝，不重新签发。
6. IdP 已签发但云端提交失败时，未激活的任务不能被领取。可补偿撤销孤立许可；原请求仍有效且经过重新验证时，也只能取回并激活同一未过期许可。缺少原主体证据或许可已过期则终止准备，不用后台机器身份代替原用户重签。

IdP 许可登记、云端任务和待发布记录、网关去重记录各自使用本地事务。它们之间不宣称分布式原子提交；安全性由目标/摘要绑定、不可执行准备状态、领取检查和不续期保证。

### C5 网关直达、代理与执行领取

直接操作网关的客户端先通过云端的准备接口取得针对具体请求的许可及命令封装，再向网关提交同一封装；控制任务仍具有中心登记的 task_id。只读请求使用绑定操作、资源及请求摘要的 GatewayOperation 许可，不能拿读取许可执行控制。设备自身接入使用 DeviceAccess，不走用户控制流程。

代理入口解析固定路由映射并核对输入，执行代理授权及下游操作授权，生成受限封装；禁止把云端用户 Bearer、固定万能节点 token 或未验证的主体头当成下游身份。

控制任务的网关处理顺序：

1. 检查消息大小、类型、签名、issuer/audience、用途、主体委托、目标、摘要、绑定版本和截止时间。
2. redb 原子登记接收事实；重复同摘要返回已有状态，冲突摘要拒绝。
3. 在剩余预算内复核父凭据及当前业务权限，或采用仍在原绝对有效期内的可信结论；不延长原许可。
4. 以绑定本 gateway_id 的机器凭据向云端领取。云端再次核对许可及安全结论，在一个 PostgreSQL 事务中条件占用任务和设备，产生 claim_id、绑定网关签名 key 及 executor_instance_id。
5. 只有本次请求收到 Acquired 才继续；AlreadyClaimed、Final、Rejected 均不调用设备。领取响应丢失时只能查询状态，不能重新获得 Acquired。
6. redb 持久化 DispatchMarked；写入失败禁止调用设备。再次检查 start_before、目标版本和设备互斥，使用允许清单中的头及经过验证并固定的连接目标发送一次设备请求。
7. 派发后先持久化结果，再签名回报；回报丢失仅重发同一结果。设备 HTTP 的自动重试和跨目标重定向关闭。

节点应持续检查时钟和安全期限；进程暂停后恢复必须重新检查，不能使用暂停前缓存的“仍可执行”布尔值。设备锁由 claim_id 持有；未知执行不得因租约超时自动转移派发资格。

### C6 结果、状态事件与崩溃恢复

接收结果时核对登记的签名密钥、gateway/device、task_id、claim_id、executor_instance_id 和 request_digest。在云端一个事务内条件更新当前 claim 对应结果与任务状态，并释放同 claim 的设备占用。

| 场景 | 处理结果 |
|---|---|
| 相同结果重复回报 | 确认已接收，不重复写业务副作用或重新释放设备 |
| 不同结果试图覆盖确认终态 | 拒绝并审计；不覆盖原可信结果 |
| Unknown 收到原 claim 的可信迟到结果 | 可结束该执行并按原 claim 释放占用，不产生新派发 |
| 领取已提交但 Acquired 响应丢失 | 保持已领取/未知，不再取得执行资格 |
| Acquired 后 redb 写入失败、进程崩溃 | 不自动重发；记录或恢复为 Unknown |
| DispatchMarked 后设备超时或断线 | 视为可能已执行，标 Unknown，不自动重试设备 |
| 缺少有效许可或必要安全依赖 | 在派发前拒绝，保留可追踪原因 |

未知执行的管理核对需要单独授权和审计，确认原实例不能再依据旧期限开始派发、并核对设备实际状态后，才可解除占用；解除不将旧任务恢复为 Pending。

设备事件使用独立消息类型，验证设备归属、绑定版本及登记的事件流和单调序号。云端条件更新已登记流；旧流或旧序号不能覆盖新状态。流切换/重新登记由可信服务推进版本，不能由消息中的任意新 stream_id 自行取得写入资格。

### C7 设备登记、目标安全与配置变更

管理员预先登记设备主体与 gateway/device 归属；设备凭据只能确认该绑定，不通过请求自报的 ID 自动取得其他记录。新增身份与变更归属属于受控管理操作。

云端设备目录与网关本地登记通过绑定版本协调：设备注册/能力或目标变更先通过归属、权限和地址规则校验，可信服务分配并记录新版本，网关持久化相应配置并回报签名确认；确认完成前不开放该版本的控制能力。授权与领取使用已确认版本，网关执行时再比对；中途版本变化拒绝旧请求，不把旧授权转移到新服务目标。

目标规则由管理员配置，按部署明确允许 origin/IP/端口及能力路径；需要的 loopback/局域网服务可以显式允许。每次实际连接重新校验 DNS 结果并固定本次连接地址，防止注册时校验后目标变化。注册设备不能修改允许清单；原始 Authorization、Cookie、代理控制头及其他未获准头不透传给设备服务。

网关无可信身份、缺少已确认绑定、无法保存去重事实或没有必要安全服务时，不退回匿名 standalone 控制模式。显式开发模式的放宽限制单独显示并排除生产部署。

### C8 权限变更、审计及验证边界

角色、用户覆写、应用授权和机器绑定修改在 IdP 事务提交后成功；各实例只采用源头绝对有效期内的结论。权限目录同步同时检查专用机器用途、操作权限和 application 绑定，不用“客户端认证成功”替代写入授权。

敏感生命周期/权限变更在同一事务中写入脱敏审计记录；任务创建和状态变更在各自事务中留下主体、目标和原因。设备派发前持久化对应执行事实；拒绝及协议错误使用有界审计入口，过载计数和告警可观察，不静默产生无界队列。HTTP 通用日志默认不缓冲敏感请求/响应体，启动配置和数据库连接日志也必须移除秘密值。

验证按上述边界注入故障，特别覆盖 IdP 许可提交后云端提交失败、领取响应丢失、网关持久化失败、设备副作用后响应丢失、结果重复/冲突和跨实例读取。证据来自隔离环境的实际行为；文件清单、具体测试命令及执行步骤分别在后续设计和任务/验收文档中细化。

## 文件组织、迁移安排与技术决策

### D1 文件组织

沿用现有 workspace，不新增部署服务或 crate。下列为计划文件；“新增”不表示当前已创建，具体编辑步骤在 task.md 中安排。已有同名根模块继续承担组装或兼容适配职责，不保留绕过新安全服务的独立实现。

| 范围 | 新增文件 | 修改文件及职责 |
|---|---|---|
| 共享安全契约 | `libs/auth/src/security/mod.rs`、`identity.rs`、`credential.rs`、`policy.rs`、`message.rs`、`budget.rs`、`error.rs`、`redaction.rs` | `libs/auth/src/lib.rs` 导出接口、统一中间件；`claims.rs`、`claims_extractor.rs`、`jwks.rs`、`token_manager.rs` 适配用途、受众、issuer 和专用签名类型 |
| IdP 安全服务 | `cobotusr/src/service/security/mod.rs`、`lifecycle.rs`、`authorization.rs`、`delegation.rs`、`repository.rs` | `service/oidc.rs`、`user.rs`、`mcp_token.rs` 统一调用生命周期；`permission.rs`、`authority.rs`、`rbac.rs` 适配统一判定器 |
| IdP 安全数据 | `cobotusr/src/entity/security_credential.rs`、`security_grant.rs`、`security_session.rs`、`security_refresh_family.rs`、`security_binding.rs`、`security_permit.rs`、`security_audit.rs` | `entity/mod.rs` 注册；已有 OAuth/MCP 表仅保留必要兼容元数据；绑定模型包含机器、设备、公钥及客户端受众登记 |
| IdP 路由 | `cobotusr/src/route/security.rs`、`security_admin.rs` | `route/auth.rs`、`oidc.rs`、`mcp.rs`、`enforce.rs`、`sync_permissions.rs`、角色及覆写管理路由、`route/mod.rs` 接入统一保护；`lib.rs`、`config.rs` 负责安全配置、组装与脱敏 |
| 云端业务服务 | `cobotsrv/src/service/authorization.rs`、`task.rs`、`execution.rs`、`gateway_message.rs`、`gateway_binding.rs` | `enforce.rs` 作为中心安全客户端适配；`service/device.rs` 提供可信资源事实和版本；`service/mod.rs` 注册依赖 |
| 云端持久状态 | `cobotsrv/src/entity/task.rs`、`task_reservation.rs`、`execution_claim.rs`、`task_outbox.rs`、`gateway_binding.rs`、`security_audit.rs` | `entity/mod.rs`、`entity/device.rs` 增加绑定版本及事件序列；任务占位/去重约束在数据库中实施 |
| 云端入口 | `cobotsrv/src/route/execution.rs`、`gateway_operation.rs` | `route/task.rs`、`device.rs`、`tool.rs`、`cobotlet.rs`、`remote.rs`、`easytier_config.rs`、`route/mod.rs`、`mcp/route.rs`、`handler.rs`、`admin.rs` 共用服务；`mqtt.rs`、`lib.rs`、`config.rs` 接入安全消息和有界运行时 |
| 网关服务 | `cobotlet/src/service/mod.rs`、`authorization.rs`、`execution.rs`、`device_registration.rs`、`target_policy.rs`、`gateway_signer.rs` | `dispatcher.rs` 只接受已取得执行资格的内部请求；`lib.rs`、`config.rs` 组装认证及受限设备客户端 |
| 网关持久化/入口 | `cobotlet/src/store/replay.rs`、`schema.rs` | `store/task.rs`、`device.rs`、`mod.rs` 实施版本和条件迁移；`route/mod.rs`、`task.rs`、`device.rs`、`tool.rs`、`hello.rs`、`mqtt.rs`、`events.rs`、`heartbeat.rs` 使用统一接入与签名事件 |
| 前端适配 | `cobotweb/apps/cobotsrv/src/idp-management-auth.ts`、`pages/idp-management-callback.vue` | `auth.ts`、`router.ts`、`pages/callback.vue`、MCP 连接/令牌页面、共享任务类型及控制调用；cobotusr 登录/同意/HTTP 调用适配新会话与错误，不改版界面 |
| 启动和配置 | `docs/specs/gateway-oidc-authorization/migration.md` | `conf/*.example.toml` 中有关服务模板、`src/cobotusr.rs`、`cobotsrv.rs`、`cobotlet.rs`、`cobotrix.rs`、`cobotnil.rs` 校验/生成开发身份并脱敏；真实本机配置不自动写入或提交 |
| 集成与回归 | `tests/cobotusr/routes/security.rs`、`tests/cobotsrv/authorization.rs`、`execution.rs`、`gateway.rs` | 既有 `common.rs`、`mod.rs`、OIDC/MCP/权限测试扩展；纯判定器和网关存储测试位于对应模块内，浏览器流程使用可重复的测试客户端或浏览器验收 |

Cargo 清单只按这些模块实际需要增加或启用依赖；复用已有 RS256、SHA-256、异步测试及有界缓存能力，不为设计预先引入 Redis、通用策略引擎或新任务队列。

### D2 数据库与本地存储迁移

| 位置 | 计划迁移文件 | 内容 |
|---|---|---|
| IdP | `cobotusr/src/migration/m20261107_000001_security_identity.rs` | 凭据、会话、授权根、刷新族、机器/设备/客户端受众绑定和网关公钥记录；必要唯一键和状态约束 |
| IdP | `cobotusr/src/migration/m20261107_000002_security_permit_audit.rs` | 专用许可一次签发索引、绑定摘要及短期许可载荷、脱敏安全审计 |
| IdP | `cobotusr/src/seed/m20261107_000003_security_operations.rs` | 已登记操作及必要初始绑定；时间顺序晚于所依赖的 schema，显式验证新库及升级库的迁移排序 |
| 云端 | `cobotsrv/src/migration/m20261107_000001_secure_task_execution.rs` | 任务预留、任务、共享 claim/设备占用、待发布记录和不可重用标识 |
| 云端 | `cobotsrv/src/migration/m20261107_000002_gateway_binding_audit.rs` | 网关/设备绑定与事件版本、签名消息接收去重及任务安全审计 |
| 网关 | `cobotlet/src/store/schema.rs` 中的版本迁移 | redb 增加独立 replay 表及设备/任务版本，旧任务数据归档而非重新进入可执行状态 |

IdP `(issuer,jti)`、许可 `(application,task_id/request_id)`、任务幂等 `(owner,application,idempotency_key)`、任务 claim 的 task_id、活动设备占用 `(gateway_id,device_id)` 都建立数据库约束。可空幂等键的唯一行为单独验证，不靠进程锁保证并发正确性。

准备记录、已签发许可和云端 Pending 任务须保存完全匹配的身份、目标及摘要。许可生成需保存可恢复的原签名结果或固定签名材料，重试只可返回同一份许可；不能以生成新 jti/截止期替代恢复。原始用户访问令牌、refresh 明文、私钥及客户端 secret 不写入这些业务表。

网关升级前备份 redb，首次升级在独占存储锁下完成版本变更。旧 Queued/Dispatched 任务不得恢复执行；可标为历史未知供管理核对。展示历史清理不能删除 claim、防重放墓碑和仍需使用的幂等事实；安全存储容量达到上限时拒绝新任务并告警。

### D3 权限、凭据与历史数据迁移规则

1. **旧令牌**：不为缺少用途、可信授权根或完整登记的旧令牌猜测安全属性。切换后要求重新登录或明确重新签发；已泄露或用途过宽的长期 MCP 凭据不自动升级为新凭据。
2. **旧用户与角色**：保留账号、角色和可验证的授权关系；生成差异报告，逐项检查显式拒绝、通配资源、管理操作及默认角色。初始化脚本只登记必要操作，不把新执行权限自动授予全部用户。
3. **旧权限规则**：先将已有 method/route 与业务权限映射到注册操作清单；缺失映射维持拒绝，不能恢复“未登记即放行”。资源规则缺少必要边界时要求明确修正，不静默转换为全部设备。
4. **旧机器/节点凭据**：分配专用 client 和允许应用、用途、受众及网关绑定；停用万能固定节点 token。机器 secret 仅部署在服务端，浏览器包和客户端日志不得携带。
5. **设备与密钥**：管理员确认设备归属及获准服务目标，登记逐网关签名公钥并将私钥安全置于网关。绑定由控制面确认后才能执行；这不涉及 VPN 私钥或网络准入改造。
6. **旧任务与缓存**：没有可信 owner、执行身份或摘要的记录仅供受控历史查看，不自动赋予当前用户读取权，也不重新发布。切换时清除旧授权缓存，不能导入旧 Allow 作为新决策。
7. **前端与 SDK**：替换 query bearer、裸任务提交、跨受众管理请求以及不带 client_id 的撤销调用；分别实现受众刷新锁。直接控制网关的旧客户端需要采用准备接口及命令封装，不提供生产匿名兼容开关。

### D4 部署切换和恢复安排

以下是同一个整改阶段内的部署依赖顺序，不拆分安全验收范围。部署操作本身不在本次授权内自动执行。

1. **切换前检查**：备份数据库/redb，核对 schema 版本、操作覆盖、主体绑定、TLS/可信服务地址、密钥权限、时钟同步及旧客户端影响；完成隔离环境的全部安全验收。
2. **暂停安全变更与新控制指令**：进入维护窗口，停止旧版本的签发、刷新、权限写入、任务创建和匿名网关接入；限制旧 MQTT 发布方。确认旧实例及旧命令不再获得执行机会；不能把未经确认的在途动作当作已取消。
3. **迁移与配置**：执行 IdP/云端 schema 和受控数据迁移，升级网关存储；登记服务身份、设备归属和网关密钥，完成安全配置。
4. **按依赖启用**：IdP 生命周期与专用安全服务 → 云端授权/任务/消息服务 → 网关验证与执行服务 → 新前端和 SDK。原有单进程启动器改为等待 IdP 安全就绪后再开放云端入口，避免仅凭进程已启动判断可用。
5. **验收后开放**：先使用模拟/隔离目标检查双受众登录、拒绝/允许、吊销、资源隔离及消息重放，再开放新凭据登录与控制。生产真实设备验证另行安排。
6. **失败恢复**：保持停止新指令，保留 claim、去重、安全审计及所有可能发生副作用的事实；优先修复后前进。只有证明切换后未产生新安全状态或设备动作时，才可按备份恢复到维护状态；不能直接运行会重新接受旧凭据的旧二进制恢复业务。

不把“新旧版本同时接收控制流量”作为兼容策略。历史凭据失效和旧客户端迁移是明确的升级影响，由 migration.md 提供用户操作说明及管理员检查清单。

### D5 验证组织与已知环境约束

- 纯规则测试覆盖所有匹配拒绝优先、授权上界、资源缺失、用途混用及边界匹配；不以只测试正常 Allow 代替安全验证。
- 事务测试使用独立 PostgreSQL，验证授权码/刷新/MCP 并发消费、一次签发、任务/设备原子占用、终态条件更新和回滚。
- 网关使用临时 redb 和模拟 HTTP 设备，通过设备收到的请求次数证明重复投递、重启和故障恢复不重复派发。
- 多实例测试共享各自服务的主库，分别请求不同实例，验证吊销/权限变更期限与任务结果归属；跨实例不共享进程内等待器作为事实来源。
- 时钟与期限使用可注入时钟及受控故障验证；端到端记录真实耗时，验证 2 秒预算和最多 10 秒失效上界。
- 前端验证独立 PKCE 回调、state、刷新锁、管理凭据用途、注销失败提示及敏感响应脱敏。
- 既有测试入口中被注释或 ignore 的用例不计为已验证；本阶段涉及的测试必须实际注册并执行。
- 当前已知 Cargo 离线依赖缺少 EasyTier/WinDivert 子模块，之前测试没有进入编译。任务执行前先解决依赖准备并记录基线；不能把环境失败、跳过或空测试结果记为通过。

具体命令、运行顺序和逐项证据由 task.md/checklist.md 指定。文档检查只证明文档结构和需求映射完整，不证明安全实现已经通过测试。

### E1 关键技术决策

| 决策点 | 选择 | 理由与代价 |
|---|---|---|
| 权限事实来源 | IdP 主库 + 统一纯判定器 | 保持现有身份平台，避免不同入口独立解释权限 |
| 两套权限数据 | 保留 URI authority 与 application permission，各自适配同一判定契约 | 减少不必要的表合并迁移，同时统一拒绝优先及默认拒绝 |
| 凭据有效性 | 统一登记，未知凭据拒绝，区分稳定撤销根与签发审计来源 | 补齐吊销闭环，避免轮换误伤替代凭据 |
| 机器代理主体 | 独立机器认证 + 已验证主体证据 + 显式应用绑定 | 不接受自由填写 subject，也不把业务 Bearer 当作 IdP 管理凭据 |
| 管理前端 | 独立受限 PKCE 管理流程，复用现有界面 | 保持令牌管理功能，代价是需要独立回调及令牌状态 |
| 决策新鲜度 | 源头最多 5 秒绝对有效期，各跳不续期 | 预留时钟余量满足 10 秒要求；离线不能无限使用旧授权 |
| 安全预算 | 同步安全链共享 2 秒；排队、连接和锁等待计入 | 防止逐层超时叠加；依赖慢时明确失败 |
| 任务事实 | PostgreSQL 持久任务 + 原子 claim + redb 执行事实 | 多实例一致，不依赖单实例缓存或 sticky session |
| 消息证明 | 独立类型的操作许可及逐网关签名消息，首轮 RS256 | 复用已有加密能力；不以 topic/alias 代替身份，不改传输协议 |
| 跨库提交 | 不可执行预留 → 一次许可 → 本地激活/待发布事务 | 不引入分布式事务；孤立许可或失败准备不能执行 |
| 重复与崩溃 | 至多一次派发，Unknown 不自动接管重发 | 可能牺牲完成率；无法在任意设备 HTTP 副作用上承诺物理 exactly-once |
| 执行领取重试 | 只有首次 CAS 请求返回 Acquired，后续仅查询 | 响应丢失不重新产生派发资格，区别于普通创建任务的幂等响应 |
| 设备目标 | 管理员允许清单 + 实际连接地址固定 + 版本绑定 | 保留必要内网服务，同时限制 SSRF 和授权后目标替换 |
| 迁移兼容 | 维护窗口切换，旧凭据/裸任务明确迁移 | 避免同时开放新旧安全链造成绕过；需要安排业务窗口 |
| 技术范围 | 不新增外部 IdP、Redis、通用策略语言、VPN 身份系统 | 聚焦已批准范围，减少额外部署和安全边界 |

### E2 Spec 覆盖检查

| 需求 | 设计归属 |
|---|---|
| F1 身份与凭据 | A1/A2/A5、B1/B2、C2/C3 |
| F2 生命周期 | B2、C3、D3/D4 |
| F3 防重放与轮换 | B2/B5、C3/C4/C6 |
| F4 RBAC/ABAC | A2、B3、C1/C2/C8 |
| F5 资源级权限 | B3、C2/C4/C7 |
| F6 多入口一致性 | A3/A4、C2/C5 |
| F7 网关与设备绑定 | B1/B4、C5/C7 |
| F8 任务及结果隔离 | B4/B5、C4–C6、D2 |
| F9 权限管理 | A2/A6、B2/B3、C2/C8、D3 |
| F10 变更及故障 | B7、C5/C6/C8、D4/D5 |
| F11 审计及凭据保护 | A6、B1/B6、C8、D2/D5 |
| N1 安全默认值 | C2/C7、D3/D4 |
| N2/N3/N4 时限与故障 | B7、C3–C6、D5 |
| N5 多实例 | B5/B7、C4/C6、D2/D5 |
| N6 兼容与迁移 | C3、D2–D4 |
| N7/N8 观测与验证 | C8、D5 |

### E3 设计自检与审批

需求均有设计归属；共享规则层无 I/O，服务通过接口依赖仓储与客户端，IdP 不对自身进行递归安全调用。安全截止期不能被跨层缓存、消息队列、重试或异步处理延长；跨库步骤失败不会把未激活任务变为可执行任务。

架构、核心契约、模块职责与交互、文件组织、迁移安排及技术决策均已逐段确认，完整 plan.md 已获得用户批准。当前进入 task.md；全部四份文档批准前不编写实现代码。
