# 鱼符产品可用性与发布可信度 Plan

日期：2026-11-13，Asia/Shanghai。状态：用户已于 2026-11-13 明确“确认，继续编写 task.md”批准；进入任务拆解，尚未进入实现。

依据：[已批准需求](spec.md)。本文件定义目标设计，所列新增类型、文件、接口和迁移尚未实现；不表示验收通过。

## 1. 架构与现有边界

沿用现有 Rust workspace：根 CLI、`yufu` 服务、`libs/auth` 及 Vue 控制台；不拆新 crate，不引入队列或新的身份提供方。

现有编排在 `service/identity`，领域类型在 `domain/identity`，数据库事务接口在 `IdentityTransaction`，PostgreSQL 实现在 `PgSecurityTransaction`。新增核心规则放领域层，服务经仓储接口执行；HTTP handler 只完成反序列化、浏览器证明、调用和错误映射。领域层不依赖 HTTP、数据库或文件系统。

身份写入继续使用 `execute_identity`、`IdentityUnitOfWork`、统一锁序和提交回执。不得在既有外层事务内调用另一个会独立提交的服务方法。新增复用点放事务内 helper，外层服务共用该 helper。读取调用 `read_current_actor` 与精确操作授权；写入锁定后调用 `authenticate_current_actor` 再授权。

| 组件 | 职责 | 需求归属 |
| --- | --- | --- |
| 发布证据与能力清单 | 版本基线、结果附件、公开说明一致性 | F1—F5 |
| 环境启动配置与访问解释 | 生效地址、同源启动入口、有效访问来源 | F6—F8、F10 |
| 人员流程 | 邀请关联、预选分配、步骤回执、停用预览与结果 | F9、F11—F12 |
| 审计查询与待办聚合 | 结构化服务端检索、稳定分页、全量计数 | F13—F16 |
| 诊断与运行监测 | 只读检查、状态采样、告警与恢复 | F17—F21 |

## 2. 数据结构与迁移

迁移以实现时尚未使用的新标识追加到现有链，禁止重命名或改写已执行的迁移。拟用三个独立文件：`m20261113_000001_product_lifecycle.rs`、`m20261113_000002_product_audit.rs`、`m20261113_000003_product_operations.rs`；如实施前发生冲突，整体顺延新增标识并记录。

### 2.1 环境配置

在 `EnvironmentConfigInput` 与 `EnvironmentConfig` 增加 `launch_url: Option<String/LaunchUrl>`、`launch_description: Option<String>`。输入字段使用缺省空值，历史 JSON payload 无需重写；输出及配置差异、摘要、审批固定快照包括这两个字段。`EnvironmentConfig::from_payload`、`to_input`、`to_payload` 使用同一校验流程。

`LaunchUrl::parse(value: String, environment: AppEnvironmentKind) -> Result<LaunchUrl, IdentityError>`：独立类型，最大 2048 字节，拒绝空白、控制字符、反斜杠、含用户名密码、非绝对地址和非允许协议。生产只接受 HTTPS，开发另允许与现有规则一致的 loopback HTTP；本期拒绝 fragment 以保持地址可审计，允许普通 query 但不注入中心参数。说明去除首尾空白，最大 1000 字符，不接受控制字符。空输入规范化为 `None`。

启动配置存入已有配置版本 payload，生产只读取 `active_version_id` 对应版本；不在 environment 表另设绕过审批的生效地址列。

### 2.2 人员流程

| 表 | 核心字段与约束 |
| --- | --- |
| `identity_onboarding` | `id UUID PK`、`invitation_id UUID UNIQUE`、`created_by`、`accepted_user_id?`、`status`、`revision`、创建/更新时间；引用既有邀请，状态不保存秘密 |
| `identity_onboarding_step` | `id UUID PK`、`onboarding_id`、`ordinal`、`kind`（group_member/direct_assignment）、`target_id`、`planned_revision`、`status`、`reason_code?`、`operation_id?`、`revision`；流程内目标唯一 |
| `identity_lifecycle_receipt` | `id UUID PK`、`user_id`、`actor_id`、`kind`、`request_id`、`operation_id UNIQUE`、`result_summary`、`committed_at`；只保存脱敏结果与关联事件 |

流程公开状态为 `pending_acceptance / pending_configuration / completed / expired / revoked`；邀请已接受由 `accepted_at` 和 `accepted_user_id` 表达，不另建互斥的“已接受但不知道是否完成”状态。步骤状态为 `pending / applied / already_present / blocked / skipped`；blocked 必须带可展示原因，不能静默跳过。

流程最多 100 个初始步骤，创建时去重并验证所有目标可见且可操作。初始直接分配按现有应用级生产分配模型保存 application ID，不创造与现有系统冲突的“某用户生产环境分配”新表。用户组步骤可间接获得其当前应用范围，确认页明确显示这一影响。

### 2.3 审计关联

不改写历史审计事件的内容，新增 `identity_audit_application(event_id UUID, application_id INT, environment_id UUID?, PRIMARY KEY(event_id, application_id))`。新事件在原审计事务内同步写关联；group 变更涉及多个应用时分别关联。新关系记录使用领域对象身份，不通过解析任意 JSON 或对象数字猜测应用。

历史回填只处理可确定的 application/environment/approval/RP 等对象映射，采用幂等批次并记录处理数；无法可靠关联的事件仍在无应用条件查询中出现。回填通过独立维护工具执行，不阻塞所有服务启动，不授予工具修改审计正文的能力。

索引：审计 `(happened_at DESC, id DESC)`、`(actor_id, happened_at DESC, id DESC)`、`(request_id)`、`(action, happened_at DESC, id DESC)`、`(object_kind, object_id, happened_at DESC, id DESC)`；应用关联 `(application_id,event_id)`。结果条件先配合时间排序索引，按十万行实测决定是否新增结果组合索引，不预建所有条件排列。

### 2.4 告警与运行状态

`identity_operational_alert`：`id UUID PK`、`rule`、`scope_kind`、`scope_id`、`severity`、`status`（active/recovered）、`first_seen_at`、`last_seen_at`、`recovered_at?`、`acknowledged_by?`、`acknowledged_at?`、`observed_value`、`reason_code`、`revision`。对 `(rule,scope_kind,scope_id) WHERE status='active'` 建唯一索引；恢复后再次触发建立新事件。

部署级数据库和时钟告警各一个 scope；积压按环境聚合；永久失败按投递记录建立 scope。已确认字段独立于 active/recovered。清理仅删除超过保留期的 recovered 记录，活动事件不因保留期删除。

`RuntimeSnapshot` 在内存保存有界的最新采样，不存登录凭证或人员明细；时钟状态扩展现有 `ClockStatus` 提供实际 offset 和不确定度，不能用 `max_error_millis` 冒充实际偏差。状态缺失时为 `None/unknown`。

## 3. 领域与服务接口

以下为计划签名，异步 trait 使用项目现有 async_trait 模式。浏览器写服务统一接受 `&BrowserWriteEvidence`、`request_id: Uuid`、`&SecurityBudget`；有幂等回执的接口额外接收已验证操作键。

| 领域类型 | 字段或职责 |
| --- | --- |
| `AccessSource` | `kind`、`source_id`、`label`，枚举 direct/group/maintainer/test_user |
| `EffectiveEnvironmentAccess` | 应用/环境标识、当前准入、全部来源、拒绝原因、相关 revision；最终准入复用现有 evaluator |
| `OnboardingPlan` | 人员类别、开发者资格、既有账户目标（可空）、去重的 group/app 目标及版本 |
| `OffboardingPreview` | 账户版本、访问来源、活动会话计数、负责人限制、维护关系、带分页明细入口的通知范围 |
| `AuditFilter` | 时间区间、actor（用户 ID 或显式 anonymous）、application、action、object_kind、object_id、result、request_id |
| `QueryPage<T>` | `items`、`next: Option<String>`、`sampled_at`，不伪造总数 |
| `TodoSummary` | 四类 `CountState { value: Option<u64>, status, sampled_at, reason_code? }` |
| `CheckResult` | `name`、`status`、`required`、`reason_code`、`remediation`、`sampled_at` |
| `AlertObservation` | `rule`、`scope`、`value`、`condition`、单调时间；状态转换纯函数 |

服务入口：

```rust
AccessService::list_personal(secret: &str, filter: PersonalAppFilter, budget: &SecurityBudget)
    -> Result<QueryPage<PersonalApplication>, IdentityError>;
AccessService::resolve_launch(secret: &str, environment_id: Uuid, budget: &SecurityBudget)
    -> Result<LaunchTarget, IdentityError>;
AccessService::explain(secret: &str, user_id: i32, page: AccessPage, budget: &SecurityBudget)
    -> Result<QueryPage<EffectiveEnvironmentAccess>, IdentityError>;
LifecycleService::create(evidence: &BrowserWriteEvidence, plan: OnboardingPlan,
    operation_key: &str, request_id: Uuid, budget: &SecurityBudget)
    -> Result<IssuedOnboarding, IdentityError>;
LifecycleService::resume_step(evidence: &BrowserWriteEvidence, flow_id: Uuid, step_id: Uuid,
    revisions: LifecycleRevisions, operation_key: &str, request_id: Uuid, budget: &SecurityBudget)
    -> Result<LifecycleStepReceipt, IdentityError>;
LifecycleService::preview_offboarding(secret: &str, user_id: i32, budget: &SecurityBudget)
    -> Result<OffboardingPreview, IdentityError>;
AuditService::search(secret: &str, filter: AuditFilter, page: AuditPage, budget: &SecurityBudget)
    -> Result<QueryPage<ConsoleAudit>, IdentityError>;
TodoService::summary(secret: &str, budget: &SecurityBudget)
    -> Result<TodoSummary, IdentityError>;
DiagnosticsService::check(config: &Config, mode: CheckMode)
    -> Result<DeploymentReport, DiagnosticError>;
RuntimeMonitor::sample() -> RuntimeSnapshot;
AlertEvaluator::evaluate(previous: AlertState, observation: AlertObservation, policy: AlertPolicy)
    -> AlertTransition;
```

仓储增加对应的 typed 查询和写方法，按 `repository_lifecycle.rs`、`repository_access.rs`、`repository_audit.rs`、`repository_operations.rs` 分文件实现，继续以 `IdentityTransaction` 为身份事务边界。监测专用 `OperationalRepository` 接收数据库连接与超时，不继承需要可信认证时钟的身份事务；只允许读取状态计数和写告警，不提供人员、分配、令牌或凭据写入方法。

## 4. 应用入口与人员流程

### 4.1 启动入口

成员列表加入环境筛选，默认为 production；筛选在服务端执行，分页不因前端过滤出现“有数据但当前页为空”。每个候选仍调用现有准入 evaluator，再输出生效配置和来源。管理员的 explain 接口不能反向成为普通成员查询全企业目录的入口。

卡片链接指向中心同源 `GET /account/apps/{environment_id}/launch`，服务读取当前会话、环境生效配置并重新计算准入，成功以 `303` 跳到当前生效 launch_url，响应 `Cache-Control: no-store`、`Referrer-Policy: no-referrer`。请求不接受 return_to、目标 URL 或认证参数；失败不重定向，显示原因并返回中心列表入口。地址在检查后被撤权的竞态由应用自己的 OIDC 准入检查兜底，不宣称跳转能撤回已公开的目标网址。

### 4.2 入职流程与授权

创建流程时，在一个身份事务中签发既有 invitation、保存预选步骤、写审计及幂等回执。原 `/admin/invitations` 行为保持，新增流程使用专用入口。签发秘密只在首次确定提交后返回；相同操作键重试只返回安全回执，不取回秘密，丢失时通过撤销并重新签发处理。

接受邀请时，仍由原邀请事务完成唯一账户、邀请消费和原 OIDC continuation；同一事务关联流程 accepted_user_id 并置为 pending_configuration。不在匿名动作处理器中直接替管理员执行分配，不让后续分配故障回滚已成功开户或破坏 continuation。

本期由有权管理员在已登录的人员流程页继续应用预选步骤：接受后待办可见，打开流程自动刷新状态，由管理员点击“应用有效配置”。浏览器依次调用步骤 API，每步独立提交并显示结果；因此本期不是无人值守自动配权。每一步重新检查：原创建管理员当前 enabled/administrator 与对应精确操作授权、当前操作者相同权限、受邀账户有效、目标存在、生效策略及版本均有效。原创建者失权时 blocked，另一个管理员可重新确认剩余配置并将计划的授权者变更为自己；变更记录审计并提升流程版本。

每个步骤锁定流程、目标账户、组/应用及相关身份屏障，复用现有成员/分配事务 helper。将 `applied/already_present`、操作回执和真实关系变更原子提交；相同键可读已有结果。目标版本改变返回冲突，要求重新预览确认，不悄悄接受扩大的组范围。最多每次调用处理一步，避免百目标事务耗尽安全预算。分页与工作台将此阶段呈现为“入职配置待完成”。

重新确认只更新仍未完成的步骤，不能撤销已成功分配；取消步骤标 skipped 并审计。过期/撤销的邀请未接受时不能继续；已接受后不能用“撤销邀请”掩盖现有账户及授权，需明确执行停用或移除关系。

### 4.3 调岗与离职

访问来源通过统一查询加载当前直接关系、组成员关系、维护与测试关系，随后使用同一准入规则计算 effective；来源存在不必然表示准入允许。岗位调整复用现有组、分配、成员和测试用户写入口，提交前请求 preview，提交携带目标 revisions。前端分步显示结果，不引入横跨多次点击的伪事务。

停用保留现有 `set_enabled` 的最后管理员、负责人约束和撤销原子边界；从 helper 返回脱敏撤销摘要，并在同一事务写 lifecycle receipt。新预览仅提供参考，提交时重新验证约束。结果页以回执关联本次生成的退出事件和投递，不把该账户全部历史投递算为本次结果。会话/应用明细分页，不将不完整明细误报为完整数量。

## 5. HTTP 接口与错误

以下路径均为现有 API 前缀下的相对路径；不变更 OIDC 公共端点。新增管理读取注册精确 `IdentityOperation`，新写入登记 method + route 模板，并纳入现有管理员权限初始化及升级兼容测试。不能只看前端路由资格。

| 方法与路径 | 输入、输出及授权 |
| --- | --- |
| `GET /account/apps` | 新增 `environment=production/development/all`、after、limit；返回生效启动配置、sources；兼容无新增字段的旧调用 |
| `GET /account/apps/{id}/launch` | 本人准入检查，303 或脱敏 HTML 错误页；不接受目标地址 |
| `POST /admin/onboardings` | 预选计划 + Idempotency-Key；首次签发秘密和回执，其他读取不返回秘密 |
| `GET /admin/onboardings`、`GET /admin/onboardings/{id}` | 状态筛选、稳定分页；元数据及步骤详情 |
| `POST /admin/onboardings/{id}/steps/{step}/apply` | 当前流程/目标 revision + 操作键；单步结果 |
| `POST /admin/onboardings/{id}/reconfirm` | 剩余目标、revision + 操作键；重新验证并确认，显式记录授权者 |
| `POST /admin/onboardings/{id}/revoke` | 只处理未接受邀请；同一事务调用既有撤销 helper |
| `GET /admin/accounts/{id}/access` | 全来源解释、分页、当前准入及版本 |
| `GET /admin/accounts/{id}/offboarding-preview` | 影响计数、约束和明细入口 |
| `PATCH /admin/accounts/{id}/enabled` | 保留原请求，增加安全回执标识；不要求旧客户端理解新字段 |
| `GET /admin/lifecycle-receipts/{id}` | 读取本人有管理权的操作结果及关联投递，分页明细 |
| `GET /admin/audit-events/search` | typed 条件 + cursor，返回 QueryPage；旧 `/admin/audit-events` UUID 分页继续兼容 |
| `GET /admin/todos/summary`、`GET /admin/todos` | 全量计数、kind 条件与分页；各来源部分失败显式标记 |
| `GET /admin/operations/status` | 数据库内当前管理员身份认证 + 内存快照；后端不可认证时返回现有错误，不放松鉴权 |
| `GET /admin/operations/alerts` | 状态、规则、级别筛选 + 分页 |
| `POST /admin/operations/alerts/{id}/acknowledge` | CSRF、revision、操作键；只标已读 |

输入边界：limit 默认 25、范围 1—100；字符串条件最多 256 字符；请求 ID、对象类型、结果按各自类型校验；时间使用 RFC3339 带时区，转换为数据库毫秒，采用 `[from,to)`。无时间条件时查询全部保留期内数据；非法区间、空值歧义和未知条件拒绝，不静默忽略。

保持现有身份错误响应 code/request_id；新增错误码只在需要明确区分时加入：`launch_unconfigured`、`onboarding_blocked`、`cursor_filter_mismatch`、`diagnostic_unavailable`。并发冲突使用既有冲突映射，未登录和无权保持原语义。JSON DTO 类型化，不直接序列化实体或内部诊断错误。

## 6. 审计搜索与待办

新查询按照 `(happened_at DESC,id DESC)` 做 keyset 分页，条件通过绑定参数传入，不拼接原始 SQL。字符串匹配使用精确 action/object_id/request_id 和规范化 result；本期不添加任意 JSON 全文搜索。actor=anonymous 单独表达空操作者。

cursor 为版本化 base64url 内容：查询规范化指纹、首页上界、最后一项毫秒时间及 UUID。长度上限 2048 字节，严格反序列化、上下界检查；它只表示位置，不承担授权，不因其可修改而信任对象范围。每页仍鉴权；不同筛选的 cursor 返回冲突说明。新插入的较旧时间记录可能出现在后续页，明确这是稳定次序遍历而不是跨请求数据库快照；前一页已出现项不会重复。

旧 UUID 游标接口保留原次序，新控制台只调用 `/search`，不把旧 UUID 误当时间游标。应用过滤通过关联表 EXISTS，避免多来源 join 使同一事件重复。历史回填完成度在界面帮助中说明，不把无法归属事件静默挂到某个应用。

待办 summary 在短只读事务中做聚合 SQL，不加载完整工作区或逐用户计算权限。四类来源：pending 生产 approval；待接受/待配置或阻塞的 onboarding（分页可分状态）；retry/permanent_failure/expired 投递；active 告警。expired 邀请不计“可直接继续的入职”，仍可在历史筛选查看。返回分项状态与 sampled_at，数据库查询整体失败返回错误；独立运行采样来源失败只影响告警/状态相应项。

统一待办使用服务端按 kind 查询源表，输出 `kind,id,title,reason_code,created_at,status,route`。route 由受控资源类型生成，不接受数据库内任意跳转 URL。处理后通过现有异步 query 机制失效相关 summary 与列表；跨标签页重新激活时刷新，不对不同请求承诺同一快照。

## 7. 部署诊断与监控

### 7.1 只读部署检查

CLI 增加 `--check-deployment`、`--check-mode offline|online`（默认 online）、`--output text|json`，与运行服务及原 `--check-security-clock` 模式互斥。诊断分支在常规启动迁移、signer 自动生成、NTP fail-fast 之前执行；仍调用纯配置验证，但收集成检查项而非第一处失败立即退出。缺失配置文件在诊断模式直接报告失败，不回退默认配置。

offline 检查格式、issuer/Origin、密钥现存文件与只读解析；online 另做只读 DB 连接、查询迁移历史与编译内嵌迁移清单比对、独立 NTP 测量、configured issuer 的 Discovery 请求并比对 issuer/端点。数据库事务显式只读，缺迁移表判失败；不调用 `init`、`Migrator::up`、`load_identity_signer` 的生成分支或任何 bootstrap helper。

Discovery 只访问配置的固定 issuer，不接受命令行 URL 或页面参数；禁重定向、验证 TLS，单次超时 2 秒、响应上限 64 KiB，允许管理员明示的部署内网地址，避免把诊断变成公开探测代理。数据库连接/查询各 2 秒，NTP 使用既有有界测量；整体上限 15 秒。offline 的在线必需项为未验证，因此退出 2；全部必需项通过退出 0，有失败退出 1，模式或输出参数错误退出 2。JSON 输出单一文档，日志走 stderr，不附带配置原文。

### 7.2 独立状态采样

`RuntimeMonitor` 使用 tokio 单调计时每 30 秒取样，超时 2 秒，MissedTickBehavior::Skip；直接读取数据库 ping、投递聚合和 `ClockHealthSource`，不经 Cookie/CSRF 或 `SecurityBudget::new`。它不绕过身份业务校验：监测只操作独立有限的运维数据。

CLI 的 ClockMonitor 继续在启动时 fail-fast，运行期提供只读健康句柄；嵌入式服务未提供 offset 能力时显示 unknown，不能重建假同步状态。每项带采样时间和来源状态，快照超过两个周期加单次超时（62 秒）标 stale；首次未采样为 unknown。公开就绪探针汇总 DB 与现有安全时钟就绪条件，保持现有字段兼容，详细私有状态另行隔离。

详细机器状态默认关闭；可配置独立仅 loopback 的私有监控监听地址，提供只读 `/status`，不提供业务或管理接口。启用时必须同时配置 `monitor_token_file`，读取部署方预先提供的至少 32 字节随机采集凭据，以常量时间比较 Bearer 值，缺失或错误返回 401；该凭据只被此监听器接受，不能用于身份或管理接口，也不自动生成或打印。启动时拒绝非 loopback 绑定或不完整配置，容器部署需同网络命名空间采集或以本机代理读取，不默认映射到公网。外部监控通过显式配置的受认证代理访问本地采集器；本期不新增通用机器管理 token。公开探针不含账户、应用 ID、连接配置或详细计数。

数据库/时钟失效会使管理员认证按原规则失败；网页保留“无法获取当前状态”的错误事实并指引本机日志和私有监控，不伪造成功读取。进程未启动或退出由外部 HTTP/TCP/进程探测告警。

### 7.3 告警规则与持久化

新增独立 `[operations]` 配置：enabled 默认 true、sample_interval_seconds 默认 30（范围 5—300）、backlog_age_seconds 默认 300（30—86400）、backlog_consecutive_samples 默认 2（1—10）、recovered_retention_days 默认 30（1—365）、monitor_listen 与 monitor_token_file 默认空。state_dir 默认配置目录下的 `.yufu-state/operations`，可由部署方指定独立可写目录。旧配置仍可解析；默认目录不可写时标明告警持久化不可用并输出 critical 日志，不改变原身份服务启动结果，也不宣称已满足重启持久化验收。容器模板显式挂载状态卷。实例配置由部署方持有，网页不修改这些参数。

数据库故障和时钟失信每次采样直接触发；backlog 对 pending/retry/leased 非终态最老 created_at 计算年龄，连续达标才触发；永久失败下次采样触发，转为重试、送达或过期后该永久失败条件恢复，过期另在投递待办中保留。未知来源不能把已有活动事件标恢复。确认不改变触发条件。恢复均需一次成功且条件解除的完整样本。

监测写告警使用独立短事务和唯一索引防止重复活动事件；实例内监测防重入，多进程写同库通过 upsert/行锁合并。业务停用或刷新路径不等待监测采样或告警写入。

为覆盖断库后进程重启，数据库与时钟两类部署级事件同步维护本机有界恢复日志，位置 `state_dir/operations-journal.json`，只存事件 ID、原因码、首次/最近观察和未落库恢复状态。文件以临时文件 + fsync + 原子替换保存，Unix 目录 0700、文件 0600；Windows 使用当前服务账户目录权限。没有可信墙钟时标记 `timestamp_trusted=false`，使用本次进程单调时长计算阈值，不用错误墙钟推进认证。

恢复日志最多 1 MiB，合并相同事件的连续观察，保留未完成写库的状态转换；达到边界或文件不可写时明确输出 critical 日志及 journal_unavailable 状态，不报告持久化成功。容器挂载专用状态卷；不落在只读根文件系统。启动读取并验证有界日志，连接恢复后按稳定事件 ID 幂等补记再删除已确认条目。已经持久化的应用/投递活动告警由数据库恢复，首次采样前保持 unknown，不能一启动就清零。

结构化日志只在首次触发、严重程度变化、恢复或持久化失败时输出事件，持续故障每 5 分钟最多一条汇总。日志不含 URL query、秘密、原始数据库错误、账户和会话明细。外部探测示例只演示监控配置，不自动发送任何消息。

## 8. 前端设计

沿用现有 UiFormField、DataState、entity picker、draft 和 async mutation/query。服务端条件进入 route query，浏览器后退恢复筛选；账号切换清空全部关联缓存，失活页面不继续轮询。

| 页面与文件 | 变化 |
| --- | --- |
| `developer/components/config-editor.vue`、`developer/api/type.ts`、`validation.ts` | 启动地址和说明，包含环境规则、审批 diff 及历史空值 |
| `account/views/apps.vue`、`account/api/*` | 默认生产筛选、来源、缺地址原因、受控启动链接；无前端全量过滤假象 |
| `admin/views/onboardings.vue`（新） | 创建邀请及预选项、流程状态、应用步骤、冲突后重新确认 |
| `admin/views/account-detail.vue`（新） | 来源解释、岗位变更入口、停用预览、操作结果与投递链接 |
| `admin/views/audit.vue`、`admin/api/*` | typed 服务端查询、时区说明、请求 ID、稳定分页 |
| `dashboard/views/dashboard-home.vue` | 仅管理员加载全量待办卡，显示采样时间和失败，不移除成员现有导航 |
| `admin/views/todos.vue`（新） | 四类列表、kind 筛选、处理后刷新 |
| `admin/views/operations.vue`（新） | 状态概览、告警筛选、确认、恢复历史、断连错误和修复指引 |
| `router/routes.ts`、`navigation.ts`、`workspace/page-registry.ts` | 新页面资格、tab identity、导航和可发现入口 |

新增页面路径 `/admin/onboardings`、`/admin/accounts/:id`、`/admin/todos`、`/admin/operations`。按钮无权时不显示，后端仍独立拒绝。人员详情与流程详情分配独立页面身份，不能因标签缓存混入其他人员结果。

## 9. 发布能力与验收证据

新增 `docs/product/capabilities.json` 作为可校验清单：capability ID、public/internal/legacy 范围、四态状态、适用版本、对应 spec AC、证据 ID、限制及中英文文档路径。公开表格引用同一 ID；检查脚本确认对应声明和证据存在，不把文本相同当成功能验证。

新增 `scripts/readiness.py` 运行明确允许的本地检查并生成 evidence index：HEAD SHA、dirty diff/未跟踪实现文件的内容摘要（仅版本受控的实现/测试/配置模板白名单，排除本地配置/密钥/产物）、命令 argv、工具/OS/架构、起止时间、退出码、测试项数量、日志/报告相对路径和 SHA256。命令失败、零测试和过滤后缺少要求场景都不通过；缺依赖标 blocked。

候选源码摘要不包含自身产生的 evidence 文件，避免每写一条证据就使基线变化。CI 使用 clean commit SHA 和 workflow run/job URL；同一版本必须展示三平台实际门禁，构建矩阵与仅 Ubuntu 执行的隔离测试如实分列。准备工作流和提交前本地验证在任务范围内，远端执行/结果缺失保留阻塞，不主动推送或公开发布。

复用 `web/tests/e2e/enterprise-sso-*.spec.ts`、现有真实 fixture 和三引擎 runner 补 E5/E6/邀请 continuation，不重建另一个 mock SSO 验收套件。E7 从文档步骤创建隔离配置，运行 A/B/API 和升级恢复探针，保存脱敏身份/公钥指纹比对。脚本归档不得包含邀请 fragment、浏览器 Cookie、请求认证头或密钥文件。

README、中英文首页、FAQ、接口参考、控制台指南、容器部署和运维说明同步；“不可篡改审计链”等绝对描述须有实现及验证支撑，否则改为精确可证实的只读审计记录说明。能力状态在最终门禁通过后按证据更新，不能为了发布提前改为通过。

## 10. 文件组织

| 路径（仓库相对） | 职责 |
| --- | --- |
| `yufu/src/domain/identity/{application,console,operation}.rs` | 扩展现有启动字段、公开 DTO 与精确管理操作 |
| `yufu/src/domain/identity/{access,lifecycle,audit,operations}.rs`（新） | 来源、流程规则、检索与告警纯规则 |
| `yufu/src/service/identity/{access,lifecycle,audit,todos}.rs`（新） | 身份业务服务 |
| `yufu/src/service/identity/repository_{access,lifecycle,audit,operations}.rs`（新） | typed 数据访问与聚合 |
| `yufu/src/service/identity/{repository,account_actions,account_governance,assignment_change,repository_outbox,workers}.rs` | 事务接口、接受邀请关联、停用回执、复用 helper、审计应用关联与监测生命周期 |
| `yufu/src/service/operations/{mod,diagnostics,monitor,alerts,journal}.rs`（新） | 非身份业务的只读诊断、采样与告警持久化 |
| `yufu/src/route/{identity_console,account,enterprise_admin}.rs` | 扩展列表和现有账户写响应 |
| `yufu/src/route/{lifecycle,audit_search,todos,operations}.rs`（新） | 新增端点 |
| `yufu/src/{config_operations,clock,lib}.rs`、`src/yufu.rs` | 配置、健康句柄、应用状态、CLI 分支与监控 listener |
| `yufu/src/migration/m20261113_00000{1,2,3}_*.rs`（新） | 流程、审计关联/索引、告警迁移 |
| `yufu/src/entity/identity_{onboarding,onboarding_step,lifecycle_receipt,audit_application,operational_alert}.rs`（新） | 持久化模型，Debug 脱敏 |
| `yufu/src/{domain/identity,service/identity,service,route,entity,migration}/mod.rs` | 注册新增模块 |
| `web/src/pages/`、`web/src/router/`、`web/src/workspace/page-registry.ts` | 第 8 节页面与接口 |
| `tests/yufu/routes/{product_access,product_lifecycle,product_audit,product_operations}.rs`（新） | 隔离 PostgreSQL、HTTP 与故障场景 |
| `tests/yufu/routes/{mod,capacity,upgrade_drill}.rs`、`web/tests/e2e/`、`web/tests/` | 测试注册、负载、恢复及真实浏览器 |
| `scripts/readiness.py`、`scripts/tests/test_readiness.py`（新） | 证据采集与归档检查 |
| `docs/product/capabilities.json`（新）、`docs/specs/product-readiness/evidence/` | 能力清单与脱敏证据 |
| `conf/`、`deploy/docker/`、`website/src/content/docs/`、`README.md` | 配置模板、状态卷、监控示例和双语说明 |
| `.github/workflows/rust.yml`、`.github/workflows/release.yml` | 接入验证与上传证据，不主动触发发布 |

## 11. 测试策略与技术决策

先定义 typed 契约与仓储接口，再写领域规则失败/边界测试，随后实现事务与 HTTP；对接隔离数据库和真实浏览器。保留原框架与 SeaORM，不照搬其他项目的 sqlx 测试模板，也不为此次需求迁移 crate 架构。

单元测试覆盖 URL、状态转换、计划去重/上限、cursor 与条件、告警阈值、脱敏和诊断状态。数据库测试覆盖邀请并发消费、流程步骤提交/重试、来源叠加、负责人约束、旧 JSON 兼容、应用关联、分页、全量聚合、唯一活动告警及 journal 回放。HTTP 测试覆盖成功、匿名、无权限、坏请求、对象不存在、CSRF、版本冲突和跨对象路径绑定。

使用可控 ClockHealthSource、单调计时及进程重启 fixture 做断库/失时钟测试；验证监测无需先通过身份 budget。验证本机状态接口缺少/错误凭据拒绝，采集凭据不能用于管理或 OIDC，禁用时没有私有监听器。用临时文件权限和摘要证明诊断未改配置、密钥或数据库。浏览器验证新增页面真实 API，不用静态快照替代撤销/登录行为。

十万审计、一千账户数据集独立生成，预热后至少采样 100 次，分别报告简单/组合/应用关联查询及 summary 的 P50/P95、样本数与环境，不将失败请求从分位数统计中静默删除。目标同 spec：各 P95 ≤1 秒；需要新增索引时以查询计划和实际改善为依据。

| 决策 | 选择与理由 |
| --- | --- |
| 启动地址存放 | 现有配置版本 payload，完整进入生产审批与配置差异 |
| 预选分配执行 | 接受邀请后管理员显式分步应用，复用浏览器认证和事务权限，不保存或伪造委托会话 |
| 审计兼容 | 新 search 端点时间游标，保留旧 UUID 次序，避免破坏既有调用 |
| 审计应用过滤 | 独立关系表与明确回填，避免修改历史事件和猜测 JSON 含义 |
| 待办计数 | 服务端集合聚合，避免逐账户/逐应用加载造成成本与数据遗漏 |
| 故障诊断 | 独立受限运维连接与本机私有状态入口，身份鉴权继续 fail closed |
| 断库告警持久化 | 小型本机恢复日志加数据库幂等补记，容器显式状态卷 |
| 外部通知 | 本期只输出站内与日志、探针，不引入邮件或 IM 凭据和投递链 |
| 发布完成 | 实际最终版本证据齐全才完成，无法执行的远端门禁保持 blocked |

## 12. 覆盖与审批边界

F1—F5 由第 9 节及既有 E5/E6/E7 套件承接；F6—F8 由 2.1、4.1、5、8 承接；F9—F12 由 2.2、3、4.2—4.3 承接；F13—F16 由 2.3、5—6、8 承接；F17—F21 由 2.4、7、8 承接。N1—N7 分别落实到精确授权、追加迁移、脱敏输出、有界查询、分步故障语义、页面状态和隔离验收。

需特别审阅的设计选择：入职接受后由管理员显式应用初始配置；详细机器状态仅启用本机私有监听；断库事件使用独立可写状态目录持久化。三者均为本次设计的具体落地方式，批准后进入 task.md 拆解；尚未开始实现或运行功能门禁。


## 补充：应用申请体验（用户于 2026-11-13 要求检查并直接改进）

保留 app_key 后端及权限资源语义；控制台表单在创建时生成 UUID 标识，并在当前草稿、失败重试期间保持不变。自定义输入移至折叠高级设置，空值使用原自动标识。两个环境的 Audience 使用独立自动默认值，与自定义标识编辑解耦，避免改标识时隐式改动 API 契约。

生产草稿回调可留空，Audience 不再要求初次创建手填。创建回执区分各环境 Client ID、生效状态、一次性秘密和后续生产审批/分配入口。第三方仅为归属属性，管理方式由开发者/管理员流程决定。仅 local_only 要求创建时确认限制。详情有未保存编辑时禁止提交/激活旧版本；配置保存失败不发成功通知。生产深链使用 environment=production，查询值仅用于选取已获授权详情中存在的环境。
