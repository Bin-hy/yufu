# 鱼符产品可用性与发布可信度 Tasks

日期：2026-11-13，Asia/Shanghai。状态：用户已于 2026-11-13 明确“确认”批准；checklist.md 亦已批准，进入实现。依据：[已批准需求](spec.md)、[已批准技术设计](plan.md)。

本文件保留批准时的任务契约；实际执行进度及测试记录见 [verification.md](verification.md)。任务定义和文档审批本身不构成功能、测试或发布通过证据。

## 执行约定

每项定位到文件、依赖、具体步骤和可观测验证。实现或补测任务以一个聚焦改动为单位，目标主动操作时间 2—5 分钟；编译、容器启动和完整门禁的机器运行时间单独记录，不承诺在五分钟内结束。若发现单项包含无法在此粒度内完成的多个行为，先拆子项并继承本项的依赖和验收，不将半完成项标通过。

任务表“步骤”按 1 → 2 顺序执行；“文件”中的路径均相对仓库根目录。新增测试的名称是计划验证契约，实施时先创建该名称的测试，再运行对应筛选；零测试不算通过。领域规则先写能观察错误行为的测试，再实现，不为让测试通过放松安全条件。

表内命令缩写如下，执行日志必须保存展开后的完整 argv：

| 缩写 | 实际命令与通过条件 |
| --- | --- |
| `U <filter>` | `cargo test -p yufu --lib <filter> -- --test-threads=1`；对应用例数大于零且全部通过 |
| `I <filter>` | `cargo test -p yufu-root --test mod <filter> -- --test-threads=1`；隔离 PostgreSQL，筛选用例数大于零且全部通过 |
| `C <filter>` | `cargo test -p yufu-root --bin yufu <filter> -- --test-threads=1`；CLI 单元测试大于零且全部通过 |
| `W <file>` | `pnpm --filter @yufu/web test -- <file>`；Vitest 实际选择目标文件且用例数大于零 |
| `P <filter>` | `python3 -m unittest discover -s scripts/tests -p 'test_readiness.py' -k <filter> -v`；实际执行目标用例且通过 |
| `B <title>` | 在 `web` 目录执行 `node tests/e2e/enterprise-sso-runner.mjs`，以 runner 现有参数机制传递测试标题筛选；报告三个引擎均执行目标真实场景，不接受仅一个引擎或零用例成功 |

写测试但尚未实现的任务允许记录预期 RED，要求失败精确发生在新增契约而非无效夹具；对应实现任务必须转 GREEN。占位 panic、生产分支 todo 和“临时允许所有访问”均不允许进入提交。

数据库集成任务串行，测试使用可销毁数据库、临时目录/密钥、独立端口和虚构人员。禁止读取用户本地秘密制作证据、改业务库、操作用户服务器或自动推送/发版。测试失败先修复并重跑受影响范围，不把失败换成 skip。

## 文件清单

### 领域、服务与路由

| 操作 | 文件 | 职责 |
| --- | --- | --- |
| 修改 | `yufu/src/domain/identity/application.rs`、`console.rs`、`operation.rs`、`error.rs` | 环境启动字段、DTO、精确权限和错误 |
| 新建 | `yufu/src/domain/identity/access.rs`、`lifecycle.rs`、`audit.rs`、`operations.rs` | 来源、流程、审计条件/游标、告警规则 |
| 新建 | `yufu/src/service/identity/access.rs`、`lifecycle.rs`、`audit.rs`、`todos.rs` | 业务编排 |
| 新建 | `yufu/src/service/identity/repository_access.rs`、`repository_lifecycle.rs`、`repository_audit.rs`、`repository_operations.rs` | 集合查询、流程写入、审计关联及运维聚合 |
| 修改 | `yufu/src/service/identity/repository.rs`、`repository_console.rs`、`repository_outbox.rs` | typed 仓储接口、旧查询兼容、审计事务关联 |
| 修改 | `yufu/src/service/identity/account_actions.rs`、`account_governance.rs`、`assignment_change.rs`、`workers.rs` | 接受邀请关联、停用回执、复用事务 helper、worker 生命周期 |
| 新建 | `yufu/src/service/operations/mod.rs`、`diagnostics.rs`、`monitor.rs`、`alerts.rs`、`journal.rs` | 只读诊断、独立采样、告警与本机恢复日志 |
| 新建 | `yufu/src/route/lifecycle.rs`、`audit_search.rs`、`todos.rs`、`operations.rs` | 新增 HTTP 接口 |
| 修改 | `yufu/src/route/identity_console.rs`、`account.rs`、`enterprise_admin.rs` | 现有入口兼容和同源启动跳转 |
| 新建/修改 | `yufu/src/config_operations.rs`（新）、`config.rs`、`clock.rs`、`lib.rs`、`src/yufu.rs` | 配置、时钟健康句柄、状态、监听器与 CLI |
| 修改 | `yufu/src/domain/identity/mod.rs`、`service/identity/mod.rs`、`service/mod.rs`、`route/mod.rs` | 注册新增模块 |

### 持久化

| 操作 | 文件 | 职责 |
| --- | --- | --- |
| 新建 | `yufu/src/migration/m20261113_000001_product_lifecycle.rs` | 入职流程、步骤、结果回执 |
| 新建 | `yufu/src/migration/m20261113_000002_product_audit.rs` | 审计应用关联和检索索引 |
| 新建 | `yufu/src/migration/m20261113_000003_product_operations.rs` | 告警及唯一活动事件索引 |
| 新建 | `yufu/src/entity/identity_onboarding.rs`、`identity_onboarding_step.rs`、`identity_lifecycle_receipt.rs`、`identity_audit_application.rs`、`identity_operational_alert.rs` | typed 模型、关系及脱敏 Debug |
| 修改 | `yufu/src/migration/mod.rs`、`yufu/src/entity/mod.rs` | 追加注册，不修改历史迁移 |

迁移名按 plan 的冲突规则实施；开工时若名称已被占用，先同步三份设计与任务引用，再创建新迁移，不覆盖已有文件。

### 控制台

| 操作 | 文件 | 职责 |
| --- | --- | --- |
| 修改 | `web/src/pages/developer/api/type.ts`、`index.ts`、`validation.ts`、`components/config-editor.vue`、`views/app-detail.vue` | 启动配置、校验、审批差异与服务端待办筛选 |
| 修改 | `web/src/pages/account/api/type.ts`、`index.ts`、`views/apps.vue` | 成员应用筛选、来源、启动入口 |
| 修改 | `web/src/pages/admin/api/type.ts`、`index.ts`、`views/accounts.vue`、`views/audit.vue`、`views/deliveries.vue` | 新接口、详情入口、服务端查询与待办深链 |
| 新建 | `web/src/pages/admin/views/onboardings.vue`、`account-detail.vue`、`todos.vue`、`operations.vue` | 人员流程、详情、待办和运维页面 |
| 修改 | `web/src/pages/dashboard/views/dashboard-home.vue`、`web/src/pages/developer/views/apps.vue` | 管理员待办、审批深链 |
| 修改 | `web/src/router/routes.ts`、`navigation.ts`、`web/src/workspace/page-registry.ts` | 资格、页面身份、导航与草稿恢复 |

### 测试、工具、文档与交付

| 操作 | 文件 | 职责 |
| --- | --- | --- |
| 新建 | `tests/yufu/routes/product_access.rs`、`product_lifecycle.rs`、`product_audit.rs`、`product_operations.rs` | 数据库、HTTP、并发和故障回归 |
| 修改 | `tests/yufu/routes/mod.rs`、`capacity.rs`、`upgrade_drill.rs` | 注册、性能数据、旧库升级/恢复 |
| 新建 | `web/tests/unit/product-access.test.ts`、`product-lifecycle.test.ts`、`product-audit.test.ts`、`product-todos.test.ts`、`product-operations.test.ts` | 用户可见状态和交互契约 |
| 修改 | `web/tests/unit/developer-validation.test.ts`、`approval-snapshot.test.ts`、`workspace.test.ts`、`identity-session.test.ts` | 配置、审批、标签与换号回归 |
| 新建 | `web/tests/e2e/enterprise-sso-product-readiness.spec.ts` | 新增真实 API 页面流程 |
| 修改 | `web/tests/e2e/enterprise-sso-admission.spec.ts`、`enterprise-sso-logout.spec.ts`、`enterprise-sso-login.spec.ts`、`enterprise-sso-enterprise-onboarding.spec.ts`、`enterprise-sso-fixture.ts`、`enterprise-sso-webkit.mjs`、`enterprise-sso-runner.mjs` | E5/E6、邀请 continuation、三引擎与隔离资源 |
| 新建 | `scripts/readiness.py`、`scripts/tests/test_readiness.py` | 采集器、完整性检查、历史审计关联回填命令的运行入口记录 |
| 新建 | `docs/product/capabilities.json`、`docs/specs/product-readiness/evidence/README.md`、`verification.md` | 能力状态、证据约定与实际结果 |
| 修改 | `README.md`、`website/src/content/docs/index.mdx`、`en/index.mdx`、`faq.mdx`、`en/faq.mdx`、`reference/endpoints.mdx`、`en/reference/endpoints.mdx`、`guides/console.mdx`、`en/guides/console.mdx`、`getting-started.mdx`、`en/getting-started.mdx` | 中英文能力说明、操作指南及公开复现步骤 |
| 修改 | `website/src/content/docs/reference/configuration.mdx`、`en/reference/configuration.mdx`、`operations/deployment.mdx`、`en/operations/deployment.mdx`、`operations/docker.mdx`、`en/operations/docker.mdx`、`operations/security.mdx`、`en/operations/security.mdx` | 诊断、运维配置及备份恢复 |
| 修改 | `conf/yufu.example.toml`、`deploy/docker/yufu.toml.example`、`compose.yaml`、`README.md` | 运维参数、只读根文件系统下的状态卷 |
| 修改 | `.github/workflows/rust.yml`、`.github/workflows/release.yml` | 门禁和证据附件；不主动触发远端运行 |
| 修改 | `docs/specs/enterprise-sso-provider/checklist.md`、`gaps.md`、`verification.md` | 仅按最终实测证据更新历史缺口引用 |

## A. 能力与证据基础（T001—T008）

| 任务 | 文件 | 依赖 | 步骤 | 验证 |
| --- | --- | --- | --- | --- |
| T001：记录实施基线 | `docs/specs/product-readiness/evidence/README.md`、`verification.md` | 无 | 1. 记录 HEAD、dirty 状态与待验收范围。2. 定义 pending/pass/fail/blocked、脱敏附件和源码变化失效规则 | 核对当前 git 状态与记录一致；全部功能状态为未执行，无旧结果冒充本次通过 |
| T002：建立能力清单 | `docs/product/capabilities.json`、`scripts/tests/test_readiness.py` | T001 | 1. 按公开/内部/兼容接口列出稳定能力 ID、四态、限制、适用版本和 AC。2. 先写清单缺字段及非法状态的契约测试 | `P capabilities`：记录校验器尚未实现的预期 RED，PDP 不被列为可直接接入的公开可用 API；T007 必须转 GREEN |
| T003：修正当前能力描述 | `README.md`、`website/src/content/docs/index.mdx`、`en/index.mdx`、`faq.mdx`、`en/faq.mdx`、`reference/endpoints.mdx`、`en/reference/endpoints.mdx`、`web/src/pages/admin/views/audit.vue` | T002 | 1. 逐处对应能力 ID 和实际支持边界。2. 更正公开 PDP 与无证据支持的不可篡改链描述；新增能力仍标待验收 | 搜索 PDP、enforce、审计链相关字面描述；中英文状态一致，未提前标通过；运行官网检查 |
| T004：实现证据元数据契约 | `scripts/readiness.py`、`scripts/tests/test_readiness.py` | T001 | 1. 定义版本、命令 argv、环境、时间、数量、退出码、证据摘要。2. 加载/保存时严格校验字段与路径 | `P metadata`：缺版本、无测试数、越界证据路径和坏 JSON 被拒绝 |
| T005：限定源码摘要范围 | `scripts/readiness.py`、`scripts/tests/test_readiness.py` | T004 | 1. 对版本受控实现/测试/模板及同目录未跟踪源码取摘要，纳入删除状态。2. 排除本地配置、密钥、构建产物及 evidence 自身 | `P baseline`：实现改动使摘要变，生成证据不变；秘密哨兵不进入摘要输入清单或输出 |
| T006：采集检查实际结果 | `scripts/readiness.py`、`scripts/tests/test_readiness.py` | T004 | 1. 仅执行允许的本地 argv，捕获状态、时间与脱敏日志。2. 区分失败、缺依赖、零用例、场景缺失和成功 | `P execution`：模拟退出非零、缺命令、零测试、漏引擎均不通过；成功例记录实际数量 |
| T007：检查证据与能力引用 | `scripts/readiness.py`、`scripts/tests/test_readiness.py` | T002、T005、T006 | 1. 校验证据摘要、源码基线、AC 与能力引用。2. 提供只检查不执行的 verify 模式 | `P integrity`、`P capabilities` 全绿：篡改附件、旧源码证据、未知 AC 和缺文件全部拒绝 |
| T008：整理 CI 证据入口 | `.github/workflows/rust.yml`、`.github/workflows/release.yml`、`docs/specs/product-readiness/evidence/README.md` | T007 | 1. 定义三平台构建和 Ubuntu 测试的独立 job/结果标识。2. 设计按 SHA/run/job 上传的脱敏证据附件，保持原发布条件 | 工作流静态核对覆盖三平台且测试范围如实标注；没有 push、tag、Release 或部署执行记录 |

## B. 应用入口与访问解释（T009—T022）

| 任务 | 文件 | 依赖 | 步骤 | 验证 |
| --- | --- | --- | --- | --- |
| T009：定义启动地址规则测试 | `yufu/src/domain/identity/application.rs` | 无 | 1. 写 `product_launch_url` 表驱动用例。2. 覆盖合法 HTTPS、本机开发 HTTP、含凭据/fragment/控制符/反斜杠/相对地址/超长输入 | `U product_launch_url`：先记录新增契约 RED，失败必须来自待实现规则 |
| T010：实现 LaunchUrl | `yufu/src/domain/identity/application.rs` | T009 | 1. 定义 `LaunchUrl::parse` 与只读值访问。2. 实现环境约束及说明规范化，不复用认证回调字符串承担其他语义 | `U product_launch_url` 全绿；生产 HTTP 和非允许协议均被拒绝 |
| T011：兼容旧环境 payload | `yufu/src/domain/identity/application.rs`、`tests/yufu/routes/product_access.rs`、`mod.rs` | T010 | 1. 给输入及领域配置补可选字段和转换。2. 补旧 JSON 往返及当前配置序列化测试并注册集成模块 | `U product_launch_payload`、`I product_launch_payload`：旧 payload 读写不失败，缺字段为 None |
| T012：将启动配置纳入审批 | `yufu/src/service/identity/repository_application.rs`、`web/src/pages/developer/views/app-detail.vue`、`web/tests/unit/approval-snapshot.test.ts` | T011 | 1. 确认启动字段参与版本 payload/hash/固定申请快照。2. 更新配置差异展示与相关测试 | `I product_launch_approval`、`W tests/unit/approval-snapshot.test.ts`：未批地址不生效，申请快照不随后续编辑变化 |
| T013：定义访问查询契约 | `yufu/src/domain/identity/access.rs`、`console.rs`、`mod.rs`、`yufu/src/service/identity/repository.rs` | T011 | 1. 定义 AccessSource、EffectiveEnvironmentAccess、PersonalAppFilter 和 LaunchTarget。2. 声明分页/解释/生效配置仓储接口及环境筛选校验 | `U product_access_filter`；`cargo check -p yufu`，接口无依赖数据库的领域类型 |
| T014：查询本人可访问环境 | `yufu/src/service/identity/repository_access.rs`、`repository_console.rs`、`mod.rs`、`tests/yufu/routes/product_access.rs` | T013 | 1. 在候选 SQL 中过滤环境并读取 active payload。2. 保留发行路径的准入 evaluator，限制分页大小 | `I product_personal_apps`：首屏外生产记录可达，未批准/无分配环境不出现，旧列表调用仍可用 |
| T015：集合读取全部访问来源 | `yufu/src/service/identity/repository_access.rs`、`tests/yufu/routes/product_access.rs` | T013 | 1. 查询 direct/group/maintainer/test_user 来源并去重。2. 返回来源版本和标签，不把存在来源直接等同有效准入 | `I product_access_sources`：多条来源同时展示、无重复，暂停环境即使有来源仍不可准入 |
| T016：实现管理员访问解释 | `yufu/src/service/identity/access.rs`、`yufu/src/domain/identity/operation.rs`、`tests/yufu/routes/product_access.rs` | T014、T015 | 1. 当前管理员鉴权及精确操作授权后读取来源。2. 对每个分页环境给出真实准入、理由与 revisions | `I product_access_explain`：普通账户拒绝；移除一条仍有来源时明确仍可用，移除全部后拒绝 |
| T017：实现启动目标重检 | `yufu/src/service/identity/access.rs`、`tests/yufu/routes/product_access.rs` | T014、T015 | 1. 实现 resolve_launch，重新读取当前中心身份及 active 配置。2. 对失权、停用、缺地址返回明确错误 | `I product_launch_resolve`：列表读取后撤权仍无法取得启动目标，草稿地址不泄露 |
| T018：接入同源启动路由 | `yufu/src/route/account.rs`、`identity_console.rs`、`yufu/src/domain/identity/error.rs`、`tests/yufu/routes/product_access.rs` | T017 | 1. 注册 `GET /account/apps/{id}/launch`。2. 成功 303/no-store/no-referrer；失败显示脱敏页，拒绝目标 URL 参数 | `I product_launch_http`：Location 只取生效配置；未知/跨对象/匿名请求不错误重定向 |
| T019：接入环境列表筛选 | `yufu/src/route/identity_console.rs`、`tests/yufu/routes/product_access.rs` | T014、T015 | 1. 扩展本人 apps typed query，支持 production/development/all。2. 维持旧调用兼容并返回启动配置和 sources | `I product_apps_http`：合法筛选、非法值、边界分页及旧请求均符合契约 |
| T020：编辑器增加启动配置 | `web/src/pages/developer/api/type.ts`、`validation.ts`、`components/config-editor.vue`、`web/tests/unit/developer-validation.test.ts` | T010、T012 | 1. 增加可选字段、缺省值、规则说明与字段错误。2. 启动字段纳入编辑草稿和差异 | `W tests/unit/developer-validation.test.ts`、前端类型检查：旧数据空值可编辑，生产坏地址不能提交 |
| T021：成员卡片提供应用入口 | `web/src/pages/account/api/type.ts`、`index.ts`、`views/apps.vue`、`web/tests/unit/product-access.test.ts` | T018、T019 | 1. 默认服务端生产筛选，展示来源和缺地址原因。2. 链接到同源 launch，不在前端拼 OIDC URL | `W tests/unit/product-access.test.ts`：筛选重置分页、缺地址无打开按钮、失败不显示空列表假象 |
| T022：验证实际应用打开流程 | `web/tests/e2e/enterprise-sso-product-readiness.spec.ts`、`enterprise-sso-fixture.ts` | T020、T021 | 1. 在隔离真实 A/B fixture 配置启动地址并完成生产审批。2. 验证卡片→应用→OIDC 登录，撤权后再次打开拒绝 | `B product app launch`：三引擎真实登录/拒绝均通过，未向公网或真实业务系统发写请求 |

## C. 人员生命周期（T023—T046）

| 任务 | 文件 | 依赖 | 步骤 | 验证 |
| --- | --- | --- | --- | --- |
| T023：定义人员流程规则测试 | `yufu/src/domain/identity/lifecycle.rs`、`mod.rs` | 无 | 1. 写状态、重复目标、100 步上限、重确认和已接受撤销的用例。2. 使用纯输入模拟角色/目标变更 | `U product_lifecycle_rules`：预期 RED 对准缺失规则，不依赖真实数据库 |
| T024：实现人员领域契约 | `yufu/src/domain/identity/lifecycle.rs`、`yufu/src/service/identity/repository.rs` | T023 | 1. 定义 OnboardingPlan、LifecycleRevisions、步骤/流程状态、OffboardingPreview 和结果类型。2. 实现规则并声明 typed 事务方法 | `U product_lifecycle_rules` 全绿；未知状态、空目标和超限均拒绝 |
| T025：追加人员流程迁移 | `yufu/src/migration/m20261113_000001_product_lifecycle.rs`、`mod.rs`、`tests/yufu/routes/product_lifecycle.rs`、`mod.rs` | T024 | 1. 建流程、步骤、回执表及唯一/FK/状态约束，包含 accepted_at。2. 仅追加注册并补新库/旧库升级测试 | `I product_lifecycle_migration`：两种路径通过，历史账户及迁移标识不变 |
| T026：定义人员实体模型 | `yufu/src/entity/identity_onboarding.rs`、`identity_onboarding_step.rs`、`identity_lifecycle_receipt.rs`、`mod.rs` | T025 | 1. 定义 typed 模型及 invitation/user 关联。2. Debug 只输出脱敏类型名，不存链接秘密 | `U product_lifecycle_entity`、`cargo check -p yufu`：模型可用且序列化 API 不直接暴露实体 |
| T027：扩展统一锁与仓储 | `yufu/src/service/identity/repository.rs`、`repository_lifecycle.rs`、`mod.rs`、`tests/yufu/routes/product_lifecycle.rs` | T026 | 1. 增加流程/步骤锁定及版本读取。2. 将新增锁放入统一排序，验证锁后重载 | `I product_lifecycle_lock`：相同流程并发串行，版本失效不产生部分修改 |
| T028：提取可复用事务内写入 | `yufu/src/service/identity/account_actions.rs`、`assignment_change.rs`、`account_governance.rs` | T027 | 1. 提取邀请签发/撤销、成员及分配、停用的事务内 helper。2. 旧服务继续调用相同规则，不嵌套独立提交 | `I identity_actions`、`I identity_assignment`、`I identity_governance`：用例非零且旧规则回归通过 |
| T029：原子创建邀请和流程 | `yufu/src/service/identity/lifecycle.rs`、`repository_lifecycle.rs`、`mod.rs`、`tests/yufu/routes/product_lifecycle.rs` | T028 | 1. 创建前验证权限、去重目标、版本及初始范围。2. 同一事务写邀请、预选步骤、审计与安全回执 | `I product_onboarding_create`：失败无半流程，首次才返回秘密，同键重试不取回秘密 |
| T030：关联邀请接受账户 | `yufu/src/service/identity/account_actions.rs`、`repository_lifecycle.rs`、`tests/yufu/routes/product_lifecycle.rs` | T029 | 1. 在原接受事务内关联 accepted_user_id/accepted_at。2. 进入 pending_configuration，保留原 continuation，不执行预选配权 | `I product_onboarding_accept`：账户唯一；接受前后未应用步骤均无额外分配；原 flow_id 不变 |
| T031：读取流程与步骤状态 | `yufu/src/service/identity/lifecycle.rs`、`repository_lifecycle.rs`、`tests/yufu/routes/product_lifecycle.rs` | T030 | 1. 管理员精确授权后分页读取状态。2. 推导未接受过期/撤销，返回安全详情及可处理原因 | `I product_onboarding_read`：跨页无遗漏，返回无秘密，普通账户和无权管理员拒绝 |
| T032：应用用户组步骤 | `yufu/src/service/identity/lifecycle.rs`、`repository_lifecycle.rs`、`tests/yufu/routes/product_lifecycle.rs` | T028、T030 | 1. 锁定流程、原授权者、当前操作者、目标账户和组。2. 重检权限/版本后原子写成员关系、步骤状态及回执 | `I product_onboarding_group`：已有关系为 already_present；创建者失权、组删除或改版均不新增权限 |
| T033：应用直接分配步骤 | `yufu/src/service/identity/lifecycle.rs`、`repository_lifecycle.rs`、`tests/yufu/routes/product_lifecycle.rs` | T032 | 1. 复用应用级生产分配 helper。2. 在事务内检查应用状态、版本、两方授权并记录 applied/blocked | `I product_onboarding_assignment`：合法分配生效，未批生产仍不能登录，错误目标不部分写入 |
| T034：验证单步重试与提交边界 | `tests/yufu/routes/product_lifecycle.rs`、`yufu/src/service/identity/lifecycle.rs` | T032、T033 | 1. 注入同键并发、目标版本冲突、提交失败和不确定提交。2. 保留成功步骤回执，不重复写关系或签发秘密 | `I product_onboarding_retry`：关系/回执唯一；失败无半提交，无法确定提交时安全读取回执 |
| T035：重新确认剩余配置 | `yufu/src/service/identity/lifecycle.rs`、`repository_lifecycle.rs`、`tests/yufu/routes/product_lifecycle.rs` | T034 | 1. 实现 reconfirm，只允许重新审阅未完成目标并记录新授权者和版本。2. 取消目标为 skipped，保留已应用事实 | `I product_onboarding_reconfirm`：失权流程可由当前有权管理员重新确认；不删除已完成授权 |
| T036：撤销未接受流程 | `yufu/src/service/identity/lifecycle.rs`、`repository_lifecycle.rs`、`tests/yufu/routes/product_lifecycle.rs` | T031、T035 | 1. 在事务中同步撤销原邀请和流程。2. 对已接受流程拒绝此撤销，给出停用/移除关系入口 | `I product_onboarding_revoke`：撤销与接受竞态只有合法结果，不掩盖已存在账户或分配 |
| T037：提供停用影响预览 | `yufu/src/service/identity/lifecycle.rs`、`repository_access.rs`、`repository_lifecycle.rs`、`tests/yufu/routes/product_lifecycle.rs` | T016、T027 | 1. 集合查询访问、会话、负责人/维护关系与限制。2. 数量和分页明细分别输出，包含当前版本 | `I product_offboarding_preview`：首屏外关系计入数量；最后管理员、负责人限制明确；读取无副作用 |
| T038：停用回执关联本次撤销 | `yufu/src/service/identity/account_governance.rs`、`repository_lifecycle.rs`、`tests/yufu/routes/product_lifecycle.rs` | T028、T037 | 1. 原停用事务追加安全回执和新事件 ID。2. 结果查询仅关联本次投递；恢复账户继续原会话失效语义 | `I product_offboarding_receipt`：历史投递不混入；停用成功不受通知故障影响；旧会话不因启用复活 |
| T039：注册人员流程 HTTP | `yufu/src/route/lifecycle.rs`、`enterprise_admin.rs`、`mod.rs`、`yufu/src/domain/identity/operation.rs`、`yufu/src/lib.rs` | T031、T035、T036、T038 | 1. 注册 plan 所列人员创建/查询/步骤/重确认/撤销/预览/回执端点。2. 每条登记精确操作及管理员初始化兼容 | `I product_lifecycle_routes`：每条有真实调用，旧 enabled 请求仍兼容，未知字段和路径类型错误拒绝 |
| T040：覆盖人员 HTTP 权限边界 | `tests/yufu/routes/product_lifecycle.rs`、`yufu/src/route/lifecycle.rs` | T039 | 1. 按端点补匿名、无权、CSRF、对象不存在、跨流程 step 和过期 revision。2. 对比拒绝前后状态 | `I product_lifecycle_http_security`：无泄露或越权；无效请求不产生关系、步骤或回执 |
| T041：接入人员前端契约 | `web/src/pages/admin/api/type.ts`、`index.ts`、`web/tests/unit/product-lifecycle.test.ts` | T039 | 1. 定义状态、步骤、安全回执和预览 DTO。2. 每次写携带操作键及正确版本，错误映射为可恢复状态 | `W tests/unit/product-lifecycle.test.ts`：payload 正确，不将失权/冲突当成功 |
| T042：建立入职创建表单 | `web/src/pages/admin/views/onboardings.vue`、`web/tests/unit/product-lifecycle.test.ts` | T041 | 1. 选择人员类别、开发资格、组及直接应用并确认间接访问。2. 提交后一次显示链接及安全回执，离开清除秘密 | `W tests/unit/product-lifecycle.test.ts`：必填/上限/重复提交、草稿取消和秘密清理正确 |
| T043：建立入职步骤界面 | `web/src/pages/admin/views/onboardings.vue`、`web/tests/unit/product-lifecycle.test.ts` | T042、T035、T036 | 1. 展示待接受/待配置/完成/过期/撤销与每步结果。2. 按顺序单步应用，冲突后重新确认，保留此前成功 | `W tests/unit/product-lifecycle.test.ts`：部分失败可继续、blocked 不伪装完成、接受后无撤销邀请替代停用 |
| T044：建立账户详情流程 | `web/src/pages/admin/views/account-detail.vue`、`accounts.vue`、`web/tests/unit/product-lifecycle.test.ts` | T041、T016、T038 | 1. 显示访问来源和调岗前预览，跳到既有关系编辑并带 revision。2. 展示停用影响、约束、本次回执与投递结果 | `W tests/unit/product-lifecycle.test.ts`：多来源解释、先转交约束、部分明细与接收/清理区别正确 |
| T045：接入人员页面身份 | `web/src/router/routes.ts`、`navigation.ts`、`web/src/workspace/page-registry.ts`、`web/tests/unit/workspace.test.ts`、`identity-session.test.ts` | T043、T044 | 1. 注册流程列表及带对象标识的详情 tab identity。2. 后退恢复筛选，换号清缓存，非管理员无入口 | `W tests/unit/workspace.test.ts`、`W tests/unit/identity-session.test.ts`：不同人员不串状态，旧响应不能覆盖新账户 |
| T046：验证入职到停用闭环 | `web/tests/e2e/enterprise-sso-product-readiness.spec.ts`、`enterprise-sso-fixture.ts` | T045、T040、T022 | 1. 真实邀请接受→管理员应用预选→来源叠加/移除。2. 停用→A/B 保护请求拒绝→查看本次回执 | `B product personnel lifecycle`：三引擎完成流程，无关账户可用，秘密不入截图/附件 |

## D. 审计与管理员待办（T047—T062）

| 任务 | 文件 | 依赖 | 步骤 | 验证 |
| --- | --- | --- | --- | --- |
| T047：定义审计条件规则 | `yufu/src/domain/identity/audit.rs`、`mod.rs` | 无 | 1. 定义 AuditFilter、RFC3339 [from,to)、anonymous actor 和长度/分页边界。2. 对非法区间、未知结果及空值写测试 | `U product_audit_filter`：先 RED 后实现转 GREEN，不忽略错误条件 |
| T048：实现版本化审计 cursor | `yufu/src/domain/identity/audit.rs` | T047 | 1. 编解码版本、规范条件指纹、首页上界和最后时间/UUID。2. 校验长度、未知字段和上下界，不承担授权 | `U product_audit_cursor`：坏 base64、超长、指纹不匹配及越界拒绝；相同条件往返稳定 |
| T049：追加关联与查询索引 | `yufu/src/migration/m20261113_000002_product_audit.rs`、`mod.rs`、`yufu/src/entity/identity_audit_application.rs`、`mod.rs`、`tests/yufu/routes/product_audit.rs`、`mod.rs` | T047 | 1. 建关联表与 plan 指定索引。2. 注册迁移/实体和旧库升级测试，不改历史正文 | `I product_audit_migration`：关联唯一、新旧库可用、审计正文摘要升级前后相同 |
| T050：同事务写应用关联 | `yufu/src/service/identity/repository_outbox.rs`、`repository_audit.rs`、`repository.rs`、`mod.rs`、`tests/yufu/routes/product_audit.rs` | T049 | 1. 对应用/环境/审批/会话和组变更提供明确应用上下文。2. 与事件正文共事务写关联，校验摘要白名单 | `I product_audit_relation`：失败全回滚，多应用组事件可查，同一事件多关联不重复返回 |
| T051：提供受限历史回填 | `yufu/src/service/identity/repository_audit.rs`、`src/yufu.rs`、`tests/yufu/routes/product_audit.rs` | T050 | 1. 独立维护模式按有界批次仅回填可确定对象，输出处理/跳过/剩余数。2. 仅使用显式配置和维护模式，不进入启动或诊断分支 | `I product_audit_backfill`：重复运行幂等，数字碰撞/未知对象不误归属，正文和秘密均不改不输出 |
| T052：实现服务端审计查询 | `yufu/src/service/identity/repository_audit.rs`、`repository.rs`、`tests/yufu/routes/product_audit.rs` | T048、T050 | 1. 绑定结构化条件，使用 EXISTS 应用关联和时间/UUID keyset。2. 保留旧 UUID 查询入口不改次序 | `I product_audit_search`：首屏之外目标可检索，交集/同刻/跨页/并发新增无重复，旧接口结果顺序不变 |
| T053：实现审计服务授权 | `yufu/src/service/identity/audit.rs`、`mod.rs`、`yufu/src/domain/identity/operation.rs`、`tests/yufu/routes/product_audit.rs` | T052 | 1. 当前身份和精确操作授权后执行 search。2. 每页重授权并显式传递错误，不能失败回空结果 | `I product_audit_service`：翻页前撤权立即拒绝，坏条件/数据库故障分别返回正确错误 |
| T054：接入审计 search HTTP | `yufu/src/route/audit_search.rs`、`mod.rs`、`yufu/src/lib.rs`、`tests/yufu/routes/product_audit.rs` | T053 | 1. 注册新 search 端点、typed query 和错误。2. 验证 route 不与旧 UUID 列表冲突，返回 sampled_at | `I product_audit_http`：成功、匿名、无权、坏参数、时区边界、旧接口兼容全部通过 |
| T055：控制台使用全量审计搜索 | `web/src/pages/admin/api/type.ts`、`index.ts`、`views/audit.vue`、`web/tests/unit/product-audit.test.ts` | T054 | 1. 字段条件下发 search，route query 保存筛选。2. 变化回首页、展示已加载量和时区，错误与空结果分开 | `W tests/unit/product-audit.test.ts`：请求含全部交集条件，后退恢复、分页绑定和失败重试正确 |
| T056：真实审计跨页查询 | `web/tests/e2e/enterprise-sso-product-readiness.spec.ts`、`enterprise-sso-fixture.ts` | T055、T051 | 1. 写入超过一页的可归属/不可归属事件。2. 通过 UI 搜索目标、切换时区边界和回到旧筛选 | `B product audit search`：三引擎从首屏外找到目标，未知应用归属不误展示 |
| T057：实现全量待办聚合 | `yufu/src/service/identity/todos.rs`、`repository_operations.rs`、`repository.rs`、`mod.rs`、`tests/yufu/routes/product_audit.rs` | T031、T050、T065 | 1. 短只读事务集合聚合四类数据，定义 TodoSummary/CountState。2. 分项来源未知保留 reason，不依赖前端页或逐用户加载 | `I product_todo_summary`：首屏外待办计入，失效邀请规则正确，数据库故障不显示四个零 |
| T058：查询待办列表与受控入口 | `yufu/src/service/identity/todos.rs`、`repository_operations.rs`、`tests/yufu/routes/product_audit.rs` | T057 | 1. 按 kind 查询源表，稳定分页并生成固定资源路由。2. 输出原因、状态、时间，处理后从当前列表消失 | `I product_todo_list`：各 kind 有正确来源，路由不可注入，翻页不重复 |
| T059：接入待办 HTTP 与授权 | `yufu/src/route/todos.rs`、`mod.rs`、`yufu/src/domain/identity/operation.rs`、`yufu/src/lib.rs`、`tests/yufu/routes/product_audit.rs` | T058 | 1. 注册 summary/list 和精确读取权限。2. 添加普通成员/开发者/失权管理员拒绝及 query 验证 | `I product_todo_http`：成功返回 sampled_at，无权用户不读取管理员数量 |
| T060：建立待办页面 | `web/src/pages/admin/views/todos.vue`、`api/type.ts`、`api/index.ts`、`web/tests/unit/product-todos.test.ts` | T059 | 1. 展示 kind、原因和状态，支持筛选分页与重试。2. 点击受控处理入口并在返回激活时刷新 | `W tests/unit/product-todos.test.ts`：错误不显示零，处理后刷新，筛选与后退正确 |
| T061：工作台接入待办卡 | `web/src/pages/dashboard/views/dashboard-home.vue`、`web/tests/unit/product-todos.test.ts` | T060 | 1. 仅管理员发聚合请求，展示分项状态及采样时间。2. 变更后失效相关 query，成员导航保留 | `W tests/unit/product-todos.test.ts`：成员/开发者不发请求，单项失败不使其他项变零或消失 |
| T099：为投递处理页增加服务端筛选 | `yufu/src/route/identity_console.rs`、`yufu/src/service/identity/console.rs`、`repository_console.rs`、`yufu/src/domain/identity/console.rs`、`tests/yufu/routes/product_audit.rs` | T059 | 1. 为管理员投递列表增加状态、应用与环境筛选，绑定参数并重置游标。2. 保留无筛选旧调用与精确读取权限 | `I product_delivery_filter`：首屏之外 retry/permanent_failure/expired 可检索，错误条件拒绝，普通账户无权 |
| T100：为审批处理页增加服务端筛选 | `yufu/src/route/developer.rs`、`yufu/src/service/identity/application.rs`、`repository_workspace.rs`、`repository.rs`、`tests/yufu/routes/product_audit.rs` | T059 | 1. 为管理员应用列表增加待审批/应用目标筛选并在 SQL 层选择候选。2. 开发者查询保持本人成员范围，不能借管理员条件扩展目录 | `I product_approval_filter`：首屏外待审应用可找到，普通开发者无法读取他人应用，旧调用仍可用 |
| T101：消费待办深链条件 | `web/src/pages/admin/api/index.ts`、`views/deliveries.vue`、`web/src/pages/developer/api/index.ts`、`views/apps.vue`、`web/src/router/routes.ts`、`navigation.ts`、`web/src/workspace/page-registry.ts`、`web/tests/unit/product-todos.test.ts` | T099、T100、T060、T045 | 1. 注册待办页，将受控深链的状态/目标条件下发列表 API。2. 条件变化回首页，后退恢复并显示作用范围 | `W tests/unit/product-todos.test.ts`：应用/投递筛选不再只过滤当前页，路由参数只映射受支持字段 |
| T062：验证待办到处理页闭环 | `web/tests/e2e/enterprise-sso-product-readiness.spec.ts`、`enterprise-sso-fixture.ts` | T101、T061 | 1. 从待办进入首屏外应用审批或异常投递目标并处理。2. 返回工作台刷新数量，验证成员没有管理员读取请求 | `B product admin todos`：三引擎计数、筛选和目标一致；不使用当前页本地过滤冒充全量筛选 |

## E. 部署检查、采样与告警（T063—T084）

| 任务 | 文件 | 依赖 | 步骤 | 验证 |
| --- | --- | --- | --- | --- |
| T063：定义运维领域契约 | `yufu/src/domain/identity/operations.rs`、`mod.rs` | 无 | 1. 定义 CheckResult、RuntimeSnapshot、AlertObservation、AlertState/Transition。2. 区分失败、未知、陈旧及不可信时间 | `U product_operations_types`：缺值不等于零/健康，未知来源不能恢复活动告警 |
| T064：增加有界运维配置 | `yufu/src/config_operations.rs`、`config.rs`、`lib.rs` | T063 | 1. 定义 [operations] 缺省、区间和 state_dir 相对配置目录解析。2. 拒绝非 loopback 监听及 listener/token 不完整组合 | `U product_operations_config`：旧配置可解析，边界/缺项拒绝，默认无私有 listener |
| T065：追加告警持久化结构 | `yufu/src/migration/m20261113_000003_product_operations.rs`、`mod.rs`、`yufu/src/entity/identity_operational_alert.rs`、`mod.rs`、`tests/yufu/routes/product_operations.rs`、`mod.rs` | T063 | 1. 建告警表和仅 active 的部分唯一索引。2. 注册实体/迁移，保留已确认与恢复字段独立 | `I product_alert_migration`：两个活动 scope 冲突，恢复后可产生新事件，Debug 不输出敏感内容 |
| T066：定义受限运维仓储 | `yufu/src/service/identity/repository_operations.rs`、`yufu/src/service/operations/mod.rs`、`yufu/src/service/mod.rs`、`tests/yufu/routes/product_operations.rs` | T065 | 1. 声明并实现 OperationalRepository 的 ping、投递聚合、告警操作。2. 直接使用短数据库超时，不要求认证 SecurityBudget | `I product_operations_repository`：失信时钟下仍可 ping/聚合；接口无账户、令牌或权限写能力 |
| T067：提供时钟健康只读句柄 | `yufu/src/clock.rs`、`lib.rs` | T063 | 1. 提供 ClockHealthSource，输出实际 offset、不确定度及原健康字段。2. 嵌入调用缺来源时返回 unknown，保留启动 fail-fast | `U product_clock_health` 及原 clock 测试：上界不冒充偏差，过期/跳时仍失信 |
| T068：实现离线只读诊断 | `yufu/src/service/operations/diagnostics.rs`、`mod.rs` | T064 | 1. 收集配置、issuer/Origin、密钥存在/可读/格式的 CheckResult。2. 使用纯验证和只读解析，不调用自动生成或 bootstrap | `U product_diagnostics_offline`：坏配置可返回多项原因；缺密钥不创建文件，输出不含秘密哨兵 |
| T069：检查数据库与迁移状态 | `yufu/src/service/operations/diagnostics.rs`、`tests/yufu/routes/product_operations.rs` | T068、T066 | 1. 有界连接并显式只读查询迁移历史。2. 与编译内嵌迁移清单比对，缺表/待迁移明确失败 | `I product_diagnostics_database`：诊断前后 schema/数据摘要不变，不运行迁移及管理员初始化 |
| T070：检查 Discovery 与时钟 | `yufu/src/service/operations/diagnostics.rs`、`yufu/src/clock.rs`、`tests/yufu/routes/product_operations.rs` | T068、T067 | 1. 只测配置 issuer，验证 TLS、禁重定向、限制 64 KiB 和 2 秒。2. 复用有界 NTP 测量，整体限制 15 秒 | `I product_diagnostics_network`：重定向、超大体、超时、错误 issuer/证书不通过；未访问任意用户 URL |
| T071：接入诊断 CLI 分支 | `src/yufu.rs`、`yufu/src/service/operations/diagnostics.rs` | T069、T070 | 1. 注册 check-deployment/mode/output，启动分支前返回报告。2. 缺配置不回退；与原时钟检查/维护模式互斥，定义 0/1/2 退出码 | `C product_diagnostics_args`：非法参数拒绝，offline 必需在线项为未验证且返回 2 |
| T072：验证诊断可复现与无写入 | `tests/yufu/routes/product_operations.rs`、`src/yufu.rs` | T071 | 1. 子进程使用临时配置逐项注入故障，解析唯一 JSON 文档。2. 比较配置、密钥、数据库摘要与监听端口 | `I product_diagnostics_process`：仅 stderr 有日志，无新业务 listener/DB 写入，退出码与报告一致 |
| T073：实现告警纯规则 | `yufu/src/domain/identity/operations.rs`、`yufu/src/service/operations/alerts.rs` | T063、T064 | 1. 用可控单调时间定义立即故障、连续积压及恢复条件。2. 实现确认不恢复、unknown 不恢复、5 分钟重复日志节流 | `U product_alert_rules`：阈值前后、重置计数、时钟跳变、失信时间与重复故障状态正确 |
| T074：写入与恢复告警事件 | `yufu/src/service/operations/alerts.rs`、`yufu/src/service/identity/repository_operations.rs`、`tests/yufu/routes/product_operations.rs` | T066、T073 | 1. 短事务 upsert/行锁合并相同 active scope。2. 恢复后保留记录；只清理过期 recovered，不删 active | `I product_alert_persistence`：并发只一活动事件，确认不恢复，恢复后重触发新事件，清理不丢活动事件 |
| T075：实现有界恢复日志文件 | `yufu/src/service/operations/journal.rs`、`mod.rs` | T064、T073 | 1. 定义 1 MiB 日志格式、同事件合并、权限与原子替换/fsync。2. 缺权限、损坏和超限显式降级不报持久成功 | `U product_operations_journal`：临时目录权限、损坏/截断、失败原子性及无秘密内容通过 |
| T076：恢复后幂等回放 | `yufu/src/service/operations/journal.rs`、`alerts.rs`、`tests/yufu/routes/product_operations.rs` | T074、T075 | 1. 启动读取未落库状态转换，使用稳定事件 ID 补记。2. 只有确定提交后清除条目，重启/重放保留首次发生事实 | `I product_journal_replay`：回放中崩溃不重复事件，断库重启后仍有活动/待恢复记录 |
| T077：实现独立周期采样 | `yufu/src/service/operations/monitor.rs`、`mod.rs`、`tests/yufu/routes/product_operations.rs` | T066、T067、T073、T076 | 1. 每配置周期 ping/取投递聚合/读时钟，单次超时且 Skip、不重入。2. 维护快照、新鲜度及独立告警，不依赖 Cookie 或认证 budget | `I product_runtime_monitor`：断库/失时钟持续采样；62 秒缺样标 stale；未知数据不沿用绿色 |
| T078：提供本机私有监控 | `yufu/src/service/operations/monitor.rs`、`yufu/src/route/operations.rs`、`src/yufu.rs`、`tests/yufu/routes/product_operations.rs` | T064、T077 | 1. 显式启用 loopback listener，从 token_file 读取专用凭据并常量时间比较。2. 仅暴露 /status，无管理路由，不生成/打印凭据 | `I product_monitor_listener`：禁用无监听；缺/错凭据 401，非 loopback 拒绝；正确凭据不能访问业务/管理端点 |
| T079：接入管理员运行读取 | `yufu/src/route/operations.rs`、`mod.rs`、`yufu/src/domain/identity/operation.rs`、`yufu/src/lib.rs`、`tests/yufu/routes/product_operations.rs` | T074、T077 | 1. 注册 status/alerts，保持原管理员鉴权和精确读取权限。2. 公开 ready 只汇总必要信息，原字段兼容 | `I product_operations_http`：普通账户拒绝；故障不绕过认证；公开探针无详细计数/配置/对象 ID |
| T080：接入告警确认写操作 | `yufu/src/route/operations.rs`、`yufu/src/service/operations/alerts.rs`、`yufu/src/domain/identity/operation.rs`、`tests/yufu/routes/product_operations.rs` | T079 | 1. CSRF、当前管理员、版本、操作键校验后只写 acknowledged 字段。2. 记录脱敏审计，不改变 active 条件 | `I product_alert_acknowledge`：坏 CSRF/过期版本拒绝，同键重试安全，确认后持续故障仍 active |
| T081：管理采样与监听器生命周期 | `yufu/src/service/identity/workers.rs`、`yufu/src/lib.rs`、`src/yufu.rs`、`tests/yufu/routes/product_operations.rs` | T078、T080 | 1. 服务启动持有 worker/monitor handles，关停有界等待并关闭 listener。2. 禁用监测不影响原清理/投递，原认证时钟启动检查仍生效 | `I product_operations_shutdown`：测试结束释放端口/任务；只读目录明确降级，身份行为未放宽 |
| T082：建立运行与告警页面 | `web/src/pages/admin/views/operations.vue`、`api/type.ts`、`api/index.ts`、`web/src/router/routes.ts`、`navigation.ts`、`web/src/workspace/page-registry.ts`、`web/tests/unit/product-operations.test.ts` | T079、T080 | 1. 展示版本、启动时间、偏差、数据库与投递状态及采样时间。2. 告警筛选/确认/历史，失活停轮询、断连显示错误与本机指引 | `W tests/unit/product-operations.test.ts`：unknown/stale 不绿，确认不恢复，换号清理和非管理员无请求 |
| T083：更新运维模板与状态卷 | `conf/yufu.example.toml`、`deploy/docker/yufu.toml.example`、`compose.yaml`、`README.md` | T064、T075、T078、T081 | 1. 显式配置可写 state_dir、专用卷及服务账户权限，保持根文件系统只读。2. 详细监听默认关闭，凭据文件只读挂载；给外部探测配置示例 | `docker compose config` 使用临时 env/config 验证；隔离栈重启保留 journal，无公网监控端口和明文秘密输出 |
| T084：验证故障到恢复闭环 | `tests/yufu/routes/product_operations.rs`、`web/tests/e2e/enterprise-sso-product-readiness.spec.ts`、`enterprise-sso-fixture.ts` | T081、T082、T083 | 1. 控制临时 DB/clock/投递故障、重启及恢复。2. 对比日志、私有快照、站内事件和恢复时间；身份认证保持拒绝 | `I product_operations_fault_cycle`、`B product operations recovery`：三引擎显示正确状态，断库不要求先成功登录 |

## F. 性能、真实发布场景与最终证据（T085—T098）

| 任务 | 文件 | 依赖 | 步骤 | 验证 |
| --- | --- | --- | --- | --- |
| T085：准备查询性能数据集 | `tests/yufu/routes/capacity.rs`、`product_audit.rs` | T052、T057 | 1. 创建隔离十万审计、一千账户及跨页待办数据，包含真实应用关联和边界时间。2. 记录随机种子、数据规模与清理方式 | `I product_readiness_dataset`：数量/关联正确，可重复生成，未连接业务库；计数查询不遍历前端页 |
| T086：测量搜索与聚合 P95 | `tests/yufu/routes/capacity.rs`、`docs/specs/product-readiness/evidence/`、`verification.md` | T085 | 1. 预热后分别至少 100 次简单/组合/应用关联查询及 summary。2. 保存环境、失败、P50/P95 和慢查询计划，必要索引先记录再修改并回归 | `cargo test --release -p yufu-root --test mod product_readiness_capacity -- --ignored --nocapture --test-threads=1`：用例非零、无失败、各 P95≤1秒，否则任务未通过 |
| T087：补齐 E5 准入拒绝 | `web/tests/e2e/enterprise-sso-admission.spec.ts`、`enterprise-sso-fixture.ts`、`enterprise-sso-webkit.mjs` | T022、T046、T054、T079 | 1. 逐项覆盖无分配、未批准生产、坏回调/scope、无开发资格、跨成员/环境/旧入口。2. 每项有本人合法访问对照及前后状态比较 | `B E5`：三引擎每条目标拒绝且无 token/身份泄露；报告要求场景计数不为零 |
| T088：补齐 E6 撤销退出矩阵 | `web/tests/e2e/enterprise-sso-logout.spec.ts`、`enterprise-sso-fixture.ts`、`enterprise-sso-webkit.mjs` | T046、T084、T087 | 1. 按独立用例覆盖中心/应用/全部退出、组移除、同意撤回、改密/重置、停启、配置切换。2. 覆盖通知不可达/重复/重启/内省故障，记录单调计时及无关会话 | `B E6`：三引擎保护操作超过既有5秒缓存界限拒绝，通知失败不阻止本地撤销；失败分支不跳过 |
| T089：补邀请继续原应用授权 | `web/tests/e2e/enterprise-sso-enterprise-onboarding.spec.ts`、`enterprise-sso-login.spec.ts`、`enterprise-sso-fixture.ts`、`enterprise-sso-webkit.mjs` | T030、T046、T087 | 1. 真实 A 发起→邀请开户/已有账户接受→回 A。2. 验证 state/nonce/PKCE/client/issuer 关联，替换 flow/return_to 拒绝 | `B product invitation continuation`：三引擎 C03/C04 成功与拒绝分支均有证据，账户数无额外增长 |
| T090：执行 E7 文档复现 | `README.md`、`website/src/content/docs/getting-started.mdx`、`en/getting-started.mdx`、`web/tests/e2e/enterprise-sso-fixture.ts`、`docs/specs/product-readiness/verification.md` | T022、T046、T071、T083 | 1. 在全新隔离实例仅用公开步骤启动中心和 A/B/API。2. 完成 SSO、退出和诊断，记录每条步骤与偏差，修正公开文档后重走受影响段 | 独立 fixture 成功，不依赖中心私有库/DB 人工补数据；记录与手册命令一致的真实结果 |
| T091：验证旧库升级和恢复 | `tests/yufu/routes/upgrade_drill.rs`、`identity_migration.rs`、`docs/specs/product-readiness/verification.md` | T025、T049、T065、T083、T090 | 1. 扩展已有旧 schema 备份→追加迁移→登录→新备份恢复流程。2. 比对 issuer、主体、JWKS 指纹、旧配置、审计正文及新表，验证回退边界 | `cargo test --release -p yufu-root --test mod upgrade_drill -- --ignored --nocapture --test-threads=1` 及 `I identity_migration`：用例非零，完整实测，无原密钥覆盖 |
| T092：执行控制台与示例门禁 | `web/`、`examples/`、`docs/specs/product-readiness/evidence/` | T062、T084、T086、T087、T088、T089 | 1. 运行前端 test/check/lint/build。2. 运行现有 Web/SPA 示例 check/test，归档退出状态和数量 | `pnpm --filter @yufu/web test`、`check`、`lint`、`pnpm build:web`、`pnpm check:examples`、`pnpm test:examples` 全通过，失败不可用构建成功替代 |
| T093：执行 Rust 最终门禁 | Rust workspace、`docs/specs/product-readiness/evidence/` | T091、T092 | 1. 前端产物已生成后运行格式、全 feature Clippy/测试/doc。2. 运行依赖检查/审计，记录缺工具/网络阻塞与真实结果 | `cargo fmt --all -- --check`；`cargo clippy --workspace --all-targets --all-features -- -D warnings`；`cargo test --workspace --all-targets --all-features -- --test-threads=1`；`cargo test --workspace --doc --all-features`；`cargo machete`；`cargo audit` 全部通过才完成 |
| T094：同步双语操作与能力说明 | `README.md`、`website/src/content/docs/guides/console.mdx`、`en/guides/console.mdx`、`reference/configuration.mdx`、`en/reference/configuration.mdx`、`operations/deployment.mdx`、`en/operations/deployment.mdx`、`operations/docker.mdx`、`en/operations/docker.mdx`、`operations/security.mdx`、`en/operations/security.mdx`、`docs/product/capabilities.json` | T090、T091、T093 | 1. 描述实际已验收的入口、显式入职步骤、搜索、诊断退出码、告警限制与外部探测。2. 未取得 CI 证据的能力仍保留限制，不推导完整发布通过 | 能力 ID、配置字段、命令及中英文逐项对应实际结果；没有把通知接收写成应用会话已清理 |
| T095：执行官网与证据工具门禁 | `website/`、`scripts/tests/test_readiness.py`、`docs/specs/product-readiness/evidence/` | T007、T094 | 1. 运行 Python 采集器全测与官网检查/构建。2. 检查相对链接、能力引用、脱敏哨兵和附件摘要 | `python3 -m unittest discover -s scripts/tests -v`、`pnpm check:website`、`pnpm build:website` 通过；`readiness.py` 的 verify 模式无错 |
| T096：接入并核验远端 CI 证据 | `.github/workflows/rust.yml`、`.github/workflows/release.yml`、`scripts/readiness.py`、`docs/specs/product-readiness/evidence/` | T008、T092、T093、T095 | 1. 将最终检查与附件接入现有工作流，静态校验发布条件仍要求通过。2. 只读取已存在且对应候选 SHA 的三平台实际 job 结果并归档；未触发或无权限列 blocked | 核对每个 run/job URL、SHA、平台及附件摘要；本机结果不得填入远端 job。无远端结果时此项不能完成，不自行推送/打 tag |
| T097：核对最终候选完整证据 | `scripts/readiness.py`、`docs/product/capabilities.json`、`docs/specs/product-readiness/verification.md`、`docs/specs/enterprise-sso-provider/checklist.md`、`gaps.md`、`verification.md` | T086、T087、T088、T089、T091、T095、T096 | 1. 固定最终实现源码基线，校验全部 AC、三引擎、三平台及门禁附件。2. 仅勾选证据覆盖项，文档变化导致 E7 步骤变化时重跑相应复现 | verify 无过期/缺失/失败记录；有阻塞就保留发布未完成；既有14项历史通过不自动扩大为全部通过 |
| T098：交付实际结果与遗留状态 | `docs/specs/product-readiness/verification.md`、`evidence/README.md`、`docs/product/capabilities.json` | T097 | 1. 按四模块报告完成内容、实际验证和仍未完成项。2. 列出升级、状态卷、监控凭据和恢复操作要求，不主动部署或公开发布 | 与最终 checklist 实际勾选及证据一致；所有未完成项有具体原因，不因任务结束改成通过 |

## 执行顺序与依赖

编号用于追踪，不是唯一合法顺序；按显式依赖进行拓扑排序，不能直接按数字逐项执行。共 101 个任务；T099—T101 为待办处理页筛选的细化项，列在 D 组，必须在 T062 前完成。另需注意 T057 需要先完成 T065 的告警表，T085 需要待办聚合完成。

1. 文档全部审批后，从 T001、T009、T023、T047、T063 五个无依赖入口开始，先定义契约和测试，再逐个实现。
2. 建好人员迁移与运维迁移后推进访问、生命周期、审计查询及监测；T057—T062 等告警表准备好再接待办。
3. 先完成各模块局部验证，再运行 T085—T091 的规模测试、真实拒绝/撤销、文档复现与恢复。
4. 按 T092—T098 运行最终门禁、同步说明、核验远端结果与交付报告。

依赖中允许独立工作的分支表示顺序可调整，不代表授权自动创建子代理。本次按单代理工作，除非用户另行要求。

每组实现通过局部检查后，按仓库规则形成可审阅的逻辑变更；不得为了提交掩盖失败，不在文档审批阶段提交实现。没有完整执行所有验收前，状态保持进行中或具体阻塞。

## 需求与设计覆盖

| 需求 | 任务 |
| --- | --- |
| F1—F2 对外描述 | T002—T003、T094—T095、T097 |
| F3 证据基线 | T001、T004—T008、T096—T098 |
| F4—F5 关键发布验收 | T087—T093、T096—T098 |
| F6 环境启动配置 | T009—T012、T020 |
| F7—F8 成员入口与来源 | T013—T022 |
| F9 入职预选与状态 | T023—T036、T041—T043、T046 |
| F10 访问解释及调岗 | T015—T016、T037、T044、T046 |
| F11 停用及恢复结果 | T037—T040、T044、T046、T088 |
| F12 可追溯与幂等 | T029—T030、T034—T040、T050 |
| F13—F14 服务端审计检索 | T047—T056、T085—T086 |
| F15—F16 管理员待办 | T057—T062、T099—T101、T085—T086 |
| F17—F18 只读部署检查 | T064、T068—T072、T083、T090 |
| F19 运行状态 | T063—T067、T077—T079、T081—T084 |
| F20—F21 告警与监控 | T073—T084、T094 |
| N1 安全 | T010、T016—T019、T027—T040、T053—T054、T059、T078—T080、T087—T089、T099—T100 |
| N2 兼容 | T011—T012、T025、T028、T049、T052、T064—T065、T091 |
| N3 隐私 | T005—T007、T026、T029—T031、T068、T075、T078、T095 |
| N4 有界性能 | T014—T015、T047—T048、T052、T057—T058、T070、T075、T077、T085—T086、T099—T100 |
| N5 故障语义 | T034—T038、T043—T044、T055、T057、T061、T073—T084 |
| N6 可操作性 | T020—T022、T041—T046、T055—T056、T060—T062、T082、T094—T095、T101 |
| N7 隔离验证 | 执行约定、T001、T022、T046、T072、T083—T093、T096 |

| Plan 组件 | 对应任务 |
| --- | --- |
| §2.1 环境配置、§4.1 启动入口 | T009—T022 |
| §2.2 人员表、§4.2—4.3 生命周期 | T023—T046 |
| §2.3 审计关联、§6 审计/待办 | T047—T062、T099—T101 |
| §2.4 告警、§7 诊断/监控 | T063—T084 |
| §3/§5 typed 服务与 HTTP | T013、T016—T019、T024、T027—T040、T047—T054、T057—T059、T063—T081、T099—T100 |
| §8 前端 | T020—T022、T041—T046、T055—T056、T060—T062、T082、T101 |
| §9 发布能力与证据 | T001—T008、T087—T098 |
| §10 模块注册与文件组织 | 各任务列出的 mod.rs、路由注册、实体及页面文件 |
| §11 测试与性能 | 各项局部验证、T085—T098 |

## 本阶段自检与下一阶段

本文件审批前检查：任务编号唯一、依赖均存在且无环、所有任务都有明确验证、F1—F21/N1—N7 与 Plan 组件有任务归属、相对文档链接有效、无未填占位符。

2026-11-13 已执行文档结构检查：101 个唯一任务、179 条依赖边，拓扑排序包含全部 101 项；每项均有文件、两步操作和验证方式。F1—F21、N1—N7 覆盖完整，三份文档的相对链接无缺失，未发现未填占位符。该结果只验证文档结构与编号覆盖，不代表设计行为或实现正确。

此处自检仅检查任务文档。批准后进入 checklist.md，把 AC1—AC16 和跨模块故障/兼容场景变为可观察验收条目；在 checklist.md 获批前不编写实现代码或把计划测试标为已运行。


## 补充：完整应用申请流程改进（T102—T107）

用户于 2026-11-13 明确要求检查 Key 意义并直接改进完整流程，沿用已批准的应用配置与审批治理边界。

| 任务 | 实现及可观测验证 |
| --- | --- |
| T102 自动标识 | `api/registration.ts` 生成标识与环境 Audience；基础创建无需手填 Key；自定义入口折叠且解释用途 |
| T103 创建边界 | 生产草稿可直接继续；第三方归属不冒充管理员管理方式；只对 local_only 要求限制确认 |
| T104 创建回执 | 列出 Client ID 和生效状态，提供开发接入和生产下一步，秘密按真实签发显示 |
| T105 审批提交边界 | 未保存修改时阻止提交/激活；保存失败保留编辑且不发成功提示；生产深链选中正确环境 |
| T106 流程回归 | `identity-ui.test.ts`、`approval-snapshot.test.ts`；真实浏览器覆盖自动标识创建与生产审批 |
| T107 操作指南 | 中英文说明创建→联调→保存→审批→分配，以及应用标识/Client ID/秘密/Audience 的区别 |


## 补充：官网截图与容器发布排查（T108—T110）

用户要求更新官网截图并分析两小时后取消的容器发布任务，随后确认非人工取消。

| 任务 | 实际工作与验证 |
| --- | --- |
| T108 官网截图 | 重拍八张真实界面图，新增注册/开发配置/应用入口，逐张查看；中英文指南共享图片，官网检查与构建通过 |
| T109 超时修正 | 原生 amd64/arm64 matrix、架构缓存、digest 冒烟、合并前 manifest 校验；Dockerfile --check、actionlint 和标签/manifest 正反例通过 |
| T110 缓存范围及证据 | 排除官网内容和 docs/，保留工作区 manifest；scratch 导出验证；记录推断边界及远端待验证状态 |


## 补充：控制台频繁重新登录排查（T111—T113）

用户最终澄清为本地环境，约两小时需重新登录。

| 任务 | 实施与验证 |
| --- | --- |
| T111 时长区分 | 只读取本地配置的非秘密时长字段；确认 SSO 默认 43200 秒，旧 JWT 7200 秒；不改本地配置、不延长认证策略 |
| T112 故障误跳转 | 临时错误时已登录导航中止并保留原页；初次恢复提供重试原会话；仅确认为 401 才清理身份并展示密码表单 |
| T113 回归与说明 | 路由/UI 正反例、隔离测试时钟跨过两小时及十二小时边界；模板说明 SSO 参数及固定期限，不使用业务库做测试 |
