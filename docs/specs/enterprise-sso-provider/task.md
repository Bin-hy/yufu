# 企业统一身份与受控应用接入 Tasks

日期：2026-11-09，Asia/Shanghai。

状态：四份文档已于 2026-11-09 经用户确认，继续自主实现。账户、应用注册/版本/审批、管理员接入、组/分配、中心 SSO、标准授权/token/refresh/UserInfo/内省/撤销、RP 发起退出与持久投递 worker 已接入服务及 HTTP，并取得局部证据。T44 Reader 新增幂等安全回执及管理员精确操作；T66 旧管理写入共用注册凭据、Policy barrier、统一锁序和事务，仍需影响图性能及兼容流程竞态核验；T67 显式历史扫描/纳管已落盘并通过六项局部测试。本轮新增账户中心/管理控制台十项读取、撤回与投递重试接口，以及三入口 Vue 页面；前端 79 项单测、类型检查、lint、构建和 Rust fmt/clippy 已通过。第一次完整 workspace 测试 228 通过、1 失败，失败为新增种子迁移后的测试回滚位置假设，已按迁移名动态计算并单项复跑通过，完整复跑待追加。标准参考客户端、真实 HTTPS/三浏览器、真实后通道发送、UI 全场景和全部 94 项验收仍未完成。依据已确认的 [spec.md](spec.md)、[plan.md](plan.md) 和 [checklist.md](checklist.md)；实际命令、时点与限制见 [verification.md](verification.md)。

## 1. 执行范围与规则

交付定位是企业统一身份与单点登录平台，提供受控应用接入和开发者自助管理；同一产品提供账户中心、开发者控制台和企业管理控制台。沿用单企业独立部署、固定 issuer、现有 Rust/PostgreSQL/Vue 工程，覆盖获准第三方接入。公开应用市场、多租户、联邦协议和强认证按 Spec 留给后续。

实现按本文件的直接依赖推进；每项以一个可独立检验的变更为单位。规则先写测试再实现，仓储用隔离 PostgreSQL 验证；HTTP 接入前的集成测试直接调用注入依赖的服务/仓储，HTTP 接入后增加黑盒请求。新规则留在纯 domain，安全模块接收 managed 事实而不反向依赖身份编排。

本文件列出的命令和观察结果是未来实现的验证要求。每个筛选命令须执行对应新增用例，零个测试、环境缺失或静默跳过不算通过。共享迁移、实体、Router、lockfile 和 CI 的改动依次合入；只有依赖已通过且文件不冲突的任务可并行。

验收设计见 [checklist.md](checklist.md)；清单获确认后按本任务依赖顺序实现，将实际结果记录到 verification.md。用户已确认的范围和授权优先于技能中的默认流程。

## 2. 文件清单

以下为实现阶段的文件清单；“新建/修改”按当前工作区判断，同一文件可由多项任务继续完善。spec.md 作为输入保留，plan.md 本轮仅更新确认状态，task.md 本轮创建并在实现阶段记录进度。

| 操作 | 文件 | 主要职责 | 关联任务 |
| --- | --- | --- | --- |
| 修改 | `.github/workflows/rust.yml` | 接入专用验证命令和 CI | T101、T102 |
| 修改 | `Cargo.lock` | 执行完整构建与质量检查 | T102 |
| 修改 | `Cargo.toml` | 执行完整构建与质量检查 | T102 |
| 修改 | `cobotusr/Cargo.toml` | 执行完整构建与质量检查 | T102 |
| 修改 | `cobotusr/src/config.rs` | 增加部署及寿命配置 | T25、T60 |
| 新建 | `cobotusr/src/domain/identity/account.rs` | 定义账户、资格与秘密类型 | T2 |
| 新建 | `cobotusr/src/domain/identity/admission.rs` | 定义统一准入与撤销代次规则 | T4 |
| 新建 | `cobotusr/src/domain/identity/application.rs` | 定义环境配置与审批规则 | T3 |
| 新建 | `cobotusr/src/domain/identity/error.rs` | 定义账户、资格与秘密类型 | T2 |
| 新建 | `cobotusr/src/domain/identity/mod.rs` | 定义账户、资格与秘密类型 | T2 |
| 新建 | `cobotusr/src/domain/identity/session.rs` | 定义中心、流程和协议结果类型 | T5 |
| 新建 | `cobotusr/src/domain/mod.rs` | 定义账户、资格与秘密类型 | T2 |
| 修改 | `cobotusr/src/entity/application.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_access_generation.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_action_flow.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_admin_activation.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_app_approval.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_app_assignment.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_app_config_version.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_app_environment.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_app_member.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_app_test_user.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_audit_event.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_authorization_flow.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_collaboration_candidate.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_group_member.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_introspection_reader.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_invitation.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_logout_delivery.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_migration_conflict.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_oidc_rp_session.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_password_reset.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_provider.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_request_operation.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_sso_session.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_subject.rs` | 映射全部身份实体 | T11 |
| 新建 | `cobotusr/src/entity/identity_user_group.rs` | 映射全部身份实体 | T11 |
| 修改 | `cobotusr/src/entity/mod.rs` | 映射全部身份实体 | T11 |
| 修改 | `cobotusr/src/entity/oauth_authorization_code.rs` | 映射全部身份实体 | T11 |
| 修改 | `cobotusr/src/entity/oauth_client.rs` | 映射全部身份实体 | T11 |
| 修改 | `cobotusr/src/entity/oauth_consent.rs` | 映射全部身份实体 | T11 |
| 修改 | `cobotusr/src/entity/security_session.rs` | 映射全部身份实体 | T11 |
| 修改 | `cobotusr/src/entity/user.rs` | 映射全部身份实体 | T11 |
| 修改 | `cobotusr/src/error.rs` | 接入错误和日志脱敏 | T7 |
| 修改 | `cobotusr/src/lib.rs` | 定义账户、资格与秘密类型 | T2、T12、T25、T65、T67 |
| 新建 | `cobotusr/src/migration/m20261109_000001_identity_accounts.rs` | 增加账户准入迁移 | T8 |
| 新建 | `cobotusr/src/migration/m20261109_000002_identity_applications.rs` | 增加应用治理迁移 | T9 |
| 新建 | `cobotusr/src/migration/m20261109_000003_identity_sessions.rs` | 增加会话、审计与通知迁移 | T10 |
| 修改 | `cobotusr/src/migration/mod.rs` | 增加账户准入迁移 | T8、T9、T10 |
| 修改 | `cobotusr/src/recording_middleware.rs` | 接入错误和日志脱敏 | T7 |
| 新建 | `cobotusr/src/route/account.rs` | 实现账户中心 HTTP 接口 | T69 |
| 修改 | `cobotusr/src/route/application.rs` | 封闭旧账户、客户端和撤销旁路 | T66 |
| 修改 | `cobotusr/src/route/auth.rs` | 适配既有管理登录和刷新边界 | T51、T66 |
| 修改 | `cobotusr/src/route/authority.rs` | 封闭旧账户、客户端和撤销旁路 | T66 |
| 新建 | `cobotusr/src/route/developer.rs` | 实现开发者控制台 HTTP | T71 |
| 新建 | `cobotusr/src/route/enterprise_admin.rs` | 实现企业管理 HTTP | T72 |
| 新建 | `cobotusr/src/route/identity_browser.rs` | 实现登录、邀请、重置及 flow HTTP | T70 |
| 修改 | `cobotusr/src/route/mod.rs` | 完成 discovery、CORS 和协议响应保护 | T60、T68、T69、T70、T71、T72、T73 |
| 修改 | `cobotusr/src/route/oidc_admin.rs` | 封闭旧账户、客户端和撤销旁路 | T66 |
| 修改 | `cobotusr/src/route/oidc.rs` | 实现授权请求严格解析 | T53、T56、T58、T59、T60、T62、T73 |
| 修改 | `cobotusr/src/route/permission.rs` | 封闭旧账户、客户端和撤销旁路 | T66 |
| 修改 | `cobotusr/src/route/rbac.rs` | 封闭旧账户、客户端和撤销旁路 | T66 |
| 修改 | `cobotusr/src/route/role_permission.rs` | 封闭旧账户、客户端和撤销旁路 | T66 |
| 修改 | `cobotusr/src/route/role.rs` | 封闭旧账户、客户端和撤销旁路 | T66 |
| 修改 | `cobotusr/src/route/user_authority.rs` | 封闭旧账户、客户端和撤销旁路 | T66 |
| 修改 | `cobotusr/src/route/user_permission.rs` | 封闭旧账户、客户端和撤销旁路 | T66 |
| 修改 | `cobotusr/src/route/user.rs` | 封闭旧账户、客户端和撤销旁路 | T66 |
| 新建 | `cobotusr/src/seed/m20261109_000004_identity_operations.rs` | 注册管理操作并装配权限分组 | T68 |
| 修改 | `cobotusr/src/seed/mod.rs` | 注册管理操作并装配权限分组 | T68 |
| 新建 | `cobotusr/src/service/identity/account.rs` | 固定 issuer 并回填主体 | T12、T27、T28、T29、T30、T31、T32、T33、T48、T50、T61 |
| 新建 | `cobotusr/src/service/identity/application.rs` | 实现候选隐私和应用成员管理 | T33、T34、T36、T37、T38、T39、T40、T41、T42、T43、T44、T47、T67 |
| 新建 | `cobotusr/src/service/identity/assignment.rs` | 实现扁平用户组管理 | T35、T45、T46 |
| 新建 | `cobotusr/src/service/identity/authorization.rs` | 实现批准范围和版本化同意 | T48、T52、T53、T54、T55、T56 |
| 新建 | `cobotusr/src/service/identity/logout.rs` | 实现 RP 发起退出确认 | T62、T63、T64 |
| 新建 | `cobotusr/src/service/identity/mod.rs` | 定义身份仓储与共享事务契约 | T6、T65 |
| 新建 | `cobotusr/src/service/identity/protocol.rs` | 实现后端内省查询凭据 | T44、T58、T59 |
| 新建 | `cobotusr/src/service/identity/repository_account.rs` | 固定 issuer 并回填主体 | T12、T15、T30 |
| 新建 | `cobotusr/src/service/identity/repository_admission.rs` | 实现准入、分配和 generation 仓储 | T17、T46 |
| 新建 | `cobotusr/src/service/identity/repository_application.rs` | 实现应用与不可变版本仓储 | T16、T37、T42、T67 |
| 新建 | `cobotusr/src/service/identity/repository_outbox.rs` | 实现提交一致的身份审计 | T19、T20、T26、T64 |
| 新建 | `cobotusr/src/service/identity/repository_session.rs` | 实现 SSO、RP 与授权码仓储 | T18、T26 |
| 新建 | `cobotusr/src/service/identity/repository.rs` | 定义身份仓储与共享事务契约 | T6、T13、T14、T19 |
| 新建 | `cobotusr/src/service/identity/session.rs` | 实现明确范围的身份撤销 | T22、T61 |
| 新建 | `cobotusr/src/service/identity/sso.rs` | 实现管理员邀请、重置签发和秘密交换 | T27、T49、T50、T52 |
| 修改 | `cobotusr/src/service/mod.rs` | 定义身份仓储与共享事务契约 | T6、T65 |
| 修改 | `cobotusr/src/service/oidc.rs` | 实现标准授权码兑换 | T56、T57、T58、T59、T66 |
| 修改 | `cobotusr/src/service/role.rs` | 封闭旧账户、客户端和撤销旁路 | T66 |
| 修改 | `cobotusr/src/service/security/browser.rs` | 增加中心与匿名 CSRF 绑定 | T24、T49、T51 |
| 修改 | `cobotusr/src/service/security/lifecycle.rs` | 扩展安全快照与 managed 状态判断 | T21、T23、T51、T56、T57 |
| 修改 | `cobotusr/src/service/security/repository_bounds.rs` | 扩展安全快照与 managed 状态判断 | T21 |
| 修改 | `cobotusr/src/service/security/repository_revocation.rs` | 实现统一锁规划 | T13、T22 |
| 修改 | `cobotusr/src/service/security/repository.rs` | 定义身份仓储与共享事务契约 | T6、T13、T14、T21、T26、T57 |
| 修改 | `cobotusr/src/service/security/revocation.rs` | 实现明确范围的身份撤销 | T22、T66 |
| 修改 | `cobotusr/src/service/user.rs` | 实现密码修改和管理员重置完成 | T32、T66 |
| 修改 | `cobotusr/src/swagger_docs.rs` | 完成协议 Router 和 OpenAPI 契约 | T73 |
| 修改 | `cobotweb/apps/cobotusr/package.json` | 接入专用验证命令和 CI | T101 |
| 新建 | `cobotweb/apps/cobotusr/playwright.enterprise-sso.config.ts` | 建立真实跨站 TLS 浏览器 fixture | T95 |
| 修改 | `cobotweb/apps/cobotusr/src/http/axios.ts` | 改造中心前端会话恢复 | T74 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/account/api/index.ts` | 定义三入口 API 和结果类型 | T76 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/account/api/type.ts` | 定义三入口 API 和结果类型 | T76 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/account/index.vue` | 调整导航权限和页面缓存 | T75 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/account/views/apps.vue` | 实现账户资料、应用和会话页面 | T77 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/account/views/consents.vue` | 实现账户资料、应用和会话页面 | T77 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/account/views/password-reset.vue` | 实现邀请、重置和重新认证页面 | T78 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/account/views/profile.vue` | 实现账户资料、应用和会话页面 | T77 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/account/views/sessions.vue` | 实现账户资料、应用和会话页面 | T77 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/applications/components/application-editor.vue` | 实现开发者应用注册和环境页面 | T80 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/applications/components/secret-result.vue` | 实现成员、秘密和接入指导页面 | T81 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/applications/views/application-list.vue` | 实现开发者应用注册和环境页面 | T80 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/authorized-apps/views/authorized-apps-list.vue` | 实现账户资料、应用和会话页面 | T77 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/developer/api/index.ts` | 定义三入口 API 和结果类型 | T76 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/developer/api/type.ts` | 定义三入口 API 和结果类型 | T76 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/developer/components/config-diff.vue` | 实现生产申请和版本差异页面 | T82 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/developer/components/secret-result.vue` | 实现成员、秘密和接入指导页面 | T81 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/developer/index.vue` | 调整导航权限和页面缓存 | T75 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/developer/views/app-create.vue` | 实现开发者应用注册和环境页面 | T80 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/developer/views/approvals.vue` | 实现生产申请和版本差异页面 | T82 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/developer/views/apps.vue` | 实现开发者应用注册和环境页面 | T80 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/developer/views/environment.vue` | 实现开发者应用注册和环境页面 | T80、T82 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/developer/views/integration.vue` | 实现成员、秘密和接入指导页面 | T81 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/developer/views/members.vue` | 实现成员、秘密和接入指导页面 | T81 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/enterprise-admin/api/index.ts` | 定义三入口 API 和结果类型 | T76 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/enterprise-admin/api/type.ts` | 定义三入口 API 和结果类型 | T76 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/enterprise-admin/index.vue` | 调整导航权限和页面缓存 | T75 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/accounts.vue` | 实现企业账户、邀请和用户组页面 | T83 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/applications.vue` | 实现企业审批、分配和第三方页面 | T84 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/approvals.vue` | 实现企业审批、分配和第三方页面 | T84 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/assignments.vue` | 实现企业审批、分配和第三方页面 | T84 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/audit-events.vue` | 实现历史纳管、审计和投递页面 | T85 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/groups.vue` | 实现企业账户、邀请和用户组页面 | T83 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/invitations.vue` | 实现企业账户、邀请和用户组页面 | T83 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/legacy-apps.vue` | 实现历史纳管、审计和投递页面 | T85 |
| 新建 | `cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/logout-deliveries.vue` | 实现历史纳管、审计和投递页面 | T85 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/login/api/index.ts` | 定义三入口 API 和结果类型 | T76 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/login/api/type.ts` | 定义三入口 API 和结果类型 | T76 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/login/index.vue` | 实现邀请、重置和重新认证页面 | T78 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/login/register.vue` | 实现邀请、重置和重新认证页面 | T78 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/oauth/views/oauth-create.vue` | 实现历史纳管、审计和投递页面 | T85 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/oauth/views/oauth-detail.vue` | 实现历史纳管、审计和投递页面 | T85 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/oauth/views/oauth-list.vue` | 实现历史纳管、审计和投递页面 | T85 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/oidc-consent/api/index.ts` | 定义三入口 API 和结果类型 | T76 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/oidc-consent/api/type.ts` | 定义三入口 API 和结果类型 | T76 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/oidc-consent/index.vue` | 实现同意和无交互结果页面 | T79 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/oidc-login/index.vue` | 实现邀请、重置和重新认证页面 | T78、T79 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/profile/index.vue` | 实现账户资料、应用和会话页面 | T77 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/recordings/views/recordings-list.vue` | 实现历史纳管、审计和投递页面 | T85 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/redirect/index.vue` | 实现同意和无交互结果页面 | T79 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/users/views/user-create.vue` | 实现企业账户、邀请和用户组页面 | T83 |
| 修改 | `cobotweb/apps/cobotusr/src/pages/users/views/user-list.vue` | 实现企业账户、邀请和用户组页面 | T83 |
| 修改 | `cobotweb/apps/cobotusr/src/pinia/stores/tabs.ts` | 调整导航权限和页面缓存 | T75 |
| 修改 | `cobotweb/apps/cobotusr/src/pinia/stores/user.ts` | 改造中心前端会话恢复 | T74 |
| 修改 | `cobotweb/apps/cobotusr/src/router/guard.ts` | 调整导航权限和页面缓存 | T75 |
| 修改 | `cobotweb/apps/cobotusr/src/router/index.ts` | 调整导航权限和页面缓存 | T75 |
| 修改 | `cobotweb/apps/cobotusr/src/router/navigation.ts` | 调整导航权限和页面缓存 | T75 |
| 修改 | `cobotweb/apps/cobotusr/src/router/routes.ts` | 调整导航权限和页面缓存 | T75、T77、T78、T79、T80、T81、T82、T83、T84、T85 |
| 修改 | `cobotweb/apps/cobotusr/src/workspace/coordinator.ts` | 改造中心前端会话恢复 | T74 |
| 修改 | `cobotweb/apps/cobotusr/src/workspace/draft-registry.ts` | 调整导航权限和页面缓存 | T75 |
| 修改 | `cobotweb/apps/cobotusr/src/workspace/page-registry.ts` | 调整导航权限和页面缓存 | T75 |
| 修改 | `cobotweb/apps/cobotusr/src/workspace/session.ts` | 改造中心前端会话恢复 | T74 |
| 新建 | `cobotweb/apps/cobotusr/tests/e2e/enterprise-sso-admission.spec.ts` | 验证治理拒绝和接口绕过 | T97 |
| 新建 | `cobotweb/apps/cobotusr/tests/e2e/enterprise-sso-fixture.ts` | 建立真实跨站 TLS 浏览器 fixture | T95 |
| 新建 | `cobotweb/apps/cobotusr/tests/e2e/enterprise-sso-login.spec.ts` | 验证跨应用登录和流程恢复 | T96 |
| 新建 | `cobotweb/apps/cobotusr/tests/e2e/enterprise-sso-logout.spec.ts` | 验证退出联动及超过 5 秒的拒绝 | T98 |
| 新建 | `cobotweb/apps/cobotusr/tests/e2e/enterprise-sso-runner.mjs` | 建立真实跨站 TLS 浏览器 fixture | T95 |
| 新建 | `cobotweb/apps/cobotusr/tests/unit/identity-account.test.ts` | 实现账户资料、应用和会话页面 | T77 |
| 新建 | `cobotweb/apps/cobotusr/tests/unit/identity-actions.test.ts` | 实现邀请、重置和重新认证页面 | T78 |
| 新建 | `cobotweb/apps/cobotusr/tests/unit/identity-admin.test.ts` | 实现企业账户、邀请和用户组页面 | T83、T84、T85 |
| 新建 | `cobotweb/apps/cobotusr/tests/unit/identity-approvals.test.ts` | 实现生产申请和版本差异页面 | T82 |
| 新建 | `cobotweb/apps/cobotusr/tests/unit/identity-consent.test.ts` | 实现同意和无交互结果页面 | T79 |
| 新建 | `cobotweb/apps/cobotusr/tests/unit/identity-developer.test.ts` | 实现开发者应用注册和环境页面 | T80、T81 |
| 新建 | `cobotweb/apps/cobotusr/tests/unit/identity-navigation.test.ts` | 调整导航权限和页面缓存 | T75 |
| 新建 | `cobotweb/apps/cobotusr/tests/unit/identity-session.test.ts` | 改造中心前端会话恢复 | T74 |
| 新建 | `docs/integration/enterprise-oidc.md` | 编写标准接入说明和示例运行指南 | T99 |
| 新建 | `docs/operations/enterprise-identity-upgrade.md` | 编写升级、纳管与故障恢复指南 | T100 |
| 修改 | `docs/specs/enterprise-sso-provider/task.md` | 整理需求覆盖和本轮验收证据 | T103 |
| 新建 | `docs/specs/enterprise-sso-provider/verification.md` | 整理需求覆盖和本轮验收证据 | T103 |
| 新建 | `examples/oidc-spa/.env.example` | 建立独立示例工程与依赖锁定 | T86 |
| 新建 | `examples/oidc-spa/index.html` | 建立独立示例工程与依赖锁定 | T86 |
| 新建 | `examples/oidc-spa/package.json` | 建立独立示例工程与依赖锁定 | T86 |
| 新建 | `examples/oidc-spa/README.md` | 编写标准接入说明和示例运行指南 | T99 |
| 新建 | `examples/oidc-spa/server/config.ts` | 建立独立示例工程与依赖锁定 | T86 |
| 新建 | `examples/oidc-spa/server/logout.ts` | 实现 SPA 配套保护 API | T90 |
| 新建 | `examples/oidc-spa/server/protection.ts` | 实现 SPA 配套保护 API | T90、T91 |
| 新建 | `examples/oidc-spa/server/server.ts` | 实现 SPA 配套保护 API | T90 |
| 新建 | `examples/oidc-spa/src/api.ts` | 实现 SPA 示例标准登录 | T89 |
| 新建 | `examples/oidc-spa/src/config.ts` | 建立独立示例工程与依赖锁定 | T86 |
| 新建 | `examples/oidc-spa/src/main.ts` | 实现 SPA 示例标准登录 | T89 |
| 新建 | `examples/oidc-spa/src/oidc.ts` | 实现 SPA 示例标准登录 | T89 |
| 新建 | `examples/oidc-spa/tests/api.test.ts` | 实现 SPA 配套保护 API | T90 |
| 新建 | `examples/oidc-spa/tests/config.test.ts` | 建立独立示例工程与依赖锁定 | T86 |
| 新建 | `examples/oidc-spa/tests/oidc.test.ts` | 实现 SPA 示例标准登录 | T89 |
| 新建 | `examples/oidc-spa/tests/protection.test.ts` | 落实两个保护 API 的缓存时限 | T91 |
| 新建 | `examples/oidc-spa/tsconfig.json` | 建立独立示例工程与依赖锁定 | T86 |
| 新建 | `examples/oidc-spa/vite.config.ts` | 建立独立示例工程与依赖锁定 | T86 |
| 新建 | `examples/oidc-web/.env.example` | 建立独立示例工程与依赖锁定 | T86 |
| 新建 | `examples/oidc-web/package.json` | 建立独立示例工程与依赖锁定 | T86 |
| 新建 | `examples/oidc-web/README.md` | 编写标准接入说明和示例运行指南 | T99 |
| 新建 | `examples/oidc-web/src/config.ts` | 建立独立示例工程与依赖锁定 | T86 |
| 新建 | `examples/oidc-web/src/logout.ts` | 实现 Web 示例退出和保护操作 | T88 |
| 新建 | `examples/oidc-web/src/oidc.ts` | 实现 Web 示例标准登录 | T87 |
| 新建 | `examples/oidc-web/src/protection.ts` | 实现 Web 示例退出和保护操作 | T88、T91 |
| 新建 | `examples/oidc-web/src/server.ts` | 实现 Web 示例标准登录 | T87、T88 |
| 新建 | `examples/oidc-web/src/session.ts` | 实现 Web 示例标准登录 | T87 |
| 新建 | `examples/oidc-web/tests/config.test.ts` | 建立独立示例工程与依赖锁定 | T86 |
| 新建 | `examples/oidc-web/tests/logout.test.ts` | 实现 Web 示例退出和保护操作 | T88 |
| 新建 | `examples/oidc-web/tests/oidc.test.ts` | 实现 Web 示例标准登录 | T87 |
| 新建 | `examples/oidc-web/tests/protection.test.ts` | 落实两个保护 API 的缓存时限 | T91 |
| 新建 | `examples/oidc-web/tsconfig.json` | 建立独立示例工程与依赖锁定 | T86 |
| 修改 | `libs/auth/Cargo.toml` | 执行完整构建与质量检查 | T102 |
| 修改 | `libs/auth/src/claims.rs` | 扩展标准身份签名上下文 | T23、T63 |
| 修改 | `libs/auth/src/security/csrf.rs` | 增加中心与匿名 CSRF 绑定 | T24 |
| 修改 | `libs/auth/src/token_manager.rs` | 扩展标准身份签名上下文 | T23、T63 |
| 修改 | `package.json` | 建立独立示例工程与依赖锁定 | T86、T101、T102 |
| 修改 | `pnpm-lock.yaml` | 建立独立示例工程与依赖锁定 | T86、T101、T102 |
| 修改 | `pnpm-workspace.yaml` | 建立独立示例工程与依赖锁定 | T86、T101 |
| 修改 | `src/cobotusr.rs` | 增加部署及寿命配置 | T25、T65 |
| 修改 | `tests/cobotusr/common.rs` | 建立隔离身份测试入口 | T1、T95 |
| 修改 | `tests/cobotusr/routes/auth_center.rs` | 完成全新与历史数据库升级回归 | T94 |
| 修改 | `tests/cobotusr/routes/auth_revocation.rs` | 完成全新与历史数据库升级回归 | T94 |
| 修改 | `tests/cobotusr/routes/auth.rs` | 适配既有管理登录和刷新边界 | T51 |
| 新建 | `tests/cobotusr/routes/identity_account.rs` | 建立隔离身份测试入口 | T1、T15、T19、T27、T28、T29、T30、T31、T32、T68、T69、T70、T72、T92、T93 |
| 新建 | `tests/cobotusr/routes/identity_application.rs` | 建立隔离身份测试入口 | T1、T16、T19、T33、T34、T36、T37、T38、T39、T40、T41、T42、T43、T47、T68、T71、T72、T92、T93 |
| 新建 | `tests/cobotusr/routes/identity_assignment.rs` | 建立隔离身份测试入口 | T1、T17、T35、T45、T46、T72、T92、T93 |
| 新建 | `tests/cobotusr/routes/identity_browser.rs` | 建立隔离身份测试入口 | T1、T95、T96、T97、T98 |
| 新建 | `tests/cobotusr/routes/identity_migration.rs` | 建立隔离身份测试入口 | T1、T8、T9、T10、T12、T25、T66、T67、T94 |
| 新建 | `tests/cobotusr/routes/identity_protocol.rs` | 建立隔离身份测试入口 | T1、T7、T18、T23、T44、T48、T52、T53、T54、T55、T56、T57、T58、T59、T60、T62、T63、T70、T73、T92、T93 |
| 新建 | `tests/cobotusr/routes/identity_session.rs` | 建立隔离身份测试入口 | T1、T13、T14、T18、T20、T21、T22、T24、T26、T49、T50、T51、T61、T64、T65、T69、T93、T98 |
| 修改 | `tests/cobotusr/routes/mod.rs` | 建立隔离身份测试入口 | T1 |
| 修改 | `tests/cobotusr/routes/oidc.rs` | 完成全新与历史数据库升级回归 | T94 |
| 修改 | `tests/cobotusr/routes/security_mcp.rs` | 完成全新与历史数据库升级回归 | T94 |
| 修改 | `tests/cobotusr/routes/security_revocation.rs` | 完成全新与历史数据库升级回归 | T94 |
| 新建 | `tests/fixtures/enterprise-identity-legacy.sql` | 完成全新与历史数据库升级回归 | T94 |

## 3. 执行顺序

表中的范围形成一个合法的顺序；单个任务仍以它明确列出的直接依赖为准。阶段内的后端实现完成验证后再接入对应 HTTP 和 UI。

| 阶段 | 任务 | 完成结果 |
| --- | --- | --- |
| A：测试入口与纯规则 | T1—T7 | 先确定可测试接口与资格、环境、准入、流程类型 |
| B：加法迁移与共享仓储 | T8—T20 | 保留旧标识，建立共同事务、锁和持久事实 |
| C：安全生命周期基础 | T21—T26 | 让根/版本/代次进入凭据有效性与撤销条件 |
| D：账户与应用治理 | T27—T48 | 受控开户、协作、注册、审批、分配和同意 |
| E：中心认证与 OIDC | T49—T60 | 稳定 Cookie、独立 flow、标准签发/刷新/身份查询 |
| F：退出、兼容与后端入口 | T61—T73 | 退出通知、显式纳管、旧旁路封闭和权限分组 |
| G：三个前端入口 | T74—T85 | 恢复认证并呈现真实权限、配置和操作状态 |
| H：两个独立标准示例 | T86—T91 | Web 与 SPA/API 分别建立业务会话和在线保护 |
| I：集成、迁移和真实浏览器 | T92—T98 | 用并发/故障/跨站行为证明整个闭环 |
| J：交付说明与完整核验 | T99—T103 | 文档复现、CI、完整检查和可追溯证据 |

生产审批切换、准入变化和撤销都沿用同一主库事务与原 2 秒安全预算。遇到冲突最多三次整事务重试且不重置期限；中心失效先同步提交，网络退出通知通过持久队列投递。所有标签页恢复只读取稳定中心根，正常恢复与真正 refresh 重放分别验证。

## 4. 有序任务

### A：测试入口与纯规则

#### T1: 建立隔离身份测试入口

**文件：** `tests/cobotusr/common.rs`、`tests/cobotusr/routes/mod.rs`、`tests/cobotusr/routes/identity_account.rs`、`tests/cobotusr/routes/identity_application.rs`、`tests/cobotusr/routes/identity_assignment.rs`、`tests/cobotusr/routes/identity_session.rs`、`tests/cobotusr/routes/identity_protocol.rs`、`tests/cobotusr/routes/identity_migration.rs`、`tests/cobotusr/routes/identity_browser.rs`

**依赖：** 无

**步骤：**

1. 注册七个 identity 测试模块；扩展现有 testcontainers fixture，分配独立数据库和随机监听端口。
2. 为身份测试生成临时 RSA key、可信测试时钟和请求客户端，不复用既有 HMAC 管理测试令牌作为协议成功证据。
3. 加入 fixture 启停与数据库隔离测试；故障注入器默认关闭，测试结束释放子进程、容器和临时文件。

**验证：** 运行 `cargo test -p cobotrix-root identity_migration -- --test-threads=1`，两个 fixture 的数据库及密钥独立，启动/关闭成功且不读取业务数据库连接配置；新增用例必须实际被执行。

#### T2: 定义账户、资格与秘密类型

**文件：** `cobotusr/src/domain/mod.rs`、`cobotusr/src/domain/identity/mod.rs`、`cobotusr/src/domain/identity/account.rs`、`cobotusr/src/domain/identity/error.rs`、`cobotusr/src/lib.rs`

**依赖：** 无

**步骤：**

1. 定义员工/合作方、开发者资格、主体、邀请/重置用途、已验证 actor 和一次性秘密类型，沿用 i32 用户 ID。
2. 先写规则测试，再实现账户启用、owner 必要资格与最后管理员保护的纯决策。
3. 为秘密类型禁止普通 Debug/Display 和默认 DTO 序列化；错误使用固定原因码。

**验证：** 运行 `cargo test -p cobotusr domain::identity`，身份与资格独立、最后管理员保护及秘密脱敏的规则测试通过。

#### T3: 定义环境配置与审批规则

**文件：** `cobotusr/src/domain/identity/application.rs`

**依赖：** T2

**步骤：**

1. 落实 AppEnvironmentKind、ClientKind、ConsentPolicy、LogoutCapability、EnvironmentConfig 和 RegistrationReceipt 的边界。
2. 先测试精确登录/退出 URI、origin、开发 loopback、生产 HTTPS、S256 和身份 scope 子集，再实现校验。
3. 定义不可变版本、审批快照、owner/访问范围版本及客户端类型切换决策；生产草案与可运行配置区分验证。

**验证：** 运行 `cargo test -p cobotusr domain::identity`，回调通配、非法 origin、未完成生产提交、SPA 密钥及自批决策被拒绝，合法开发配置可用。

#### T4: 定义统一准入与撤销代次规则

**文件：** `cobotusr/src/domain/identity/admission.rs`

**依赖：** T2、T3

**步骤：**

1. 定义 AdmissionInput、ManagedAdmission 及用户/环境/版本/epoch/generation 事实，保持计划中的字段含义。
2. 先测试直接/组分配、开发成员/测试用户、无分配管理员、失去最后来源、重新授予，再实现纯准入决策。
3. 身份 scopes 与业务边界分别表示，身份 session_bounds 固定为空，不自动附加角色或 MCP 权限。

**验证：** 运行 `cargo test -p cobotusr domain::identity`，多来源保留访问、最后来源丢失递增 generation、重新授予不使旧代次有效。

#### T5: 定义中心、流程和协议结果类型

**文件：** `cobotusr/src/domain/identity/session.rs`

**依赖：** T2、T3、T4

**步骤：**

1. 定义中心根、RP 关联、FlowHandle、BrowserSessionSnapshot、IdentityTokenContext、AuthorizationOutcome 和明确撤销 scope。
2. 先写 prompt/max_age、流程过期、根过期/撤销和 auth_time 不延长的规则测试。
3. 把邀请 action、OIDC flow 和退出确认分别建模；请求 DTO 不能直接构造已验证上下文。

**验证：** 运行 `cargo test -p cobotusr domain::identity`，无交互请求产生标准错误、flow 证据不可混用、恢复和刷新不改变认证时间。

#### T6: 定义身份仓储与共享事务契约

**文件：** `cobotusr/src/service/identity/mod.rs`、`cobotusr/src/service/identity/repository.rs`、`cobotusr/src/service/mod.rs`、`cobotusr/src/service/security/repository.rs`

**依赖：** T4、T5

**步骤：**

1. 按 plan 第 3 节定义 IdentityRepository、IdentityTransaction: SecurityTransaction、IdentityLockScope、IdentityWrite/IdentityWriteResult。
2. 逐项定义账户、配置、分配、根/RP/flow/code、审计和 outbox 的强类型命令；commit/rollback 仅使用父接口。
3. 编写契约测试替身，覆盖错误传播和单次提交；安全服务只接收共享事实，不依赖身份编排服务。

**验证：** 运行 `cargo check --workspace --all-targets` 和契约单元测试，接口可编译，提交次数为一次且模块依赖无环。

#### T7: 接入错误和日志脱敏

**文件：** `cobotusr/src/error.rs`、`cobotusr/src/recording_middleware.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T2、T5

**步骤：**

1. 先测试登录、授权、邀请、重置、secret reset 请求的日志与错误输出。
2. 统一返回固定原因码和 request_id，屏蔽 Cookie、Authorization、完整 query/body、code/token/action secret。
3. 保留可检索事件和失败原因，不把实体或 IssuedSecret 直接格式化到 tracing。

**验证：** 运行 `cargo test -p cobotrix-root identity_protocol -- --test-threads=1`，错误和日志样本可定位请求，但不含夹带的测试秘密；新增用例必须实际被执行。

### B：加法迁移与共享仓储

#### T8: 增加账户准入迁移

**文件：** `cobotusr/src/migration/m20261109_000001_identity_accounts.rs`、`cobotusr/src/migration/mod.rs`、`tests/cobotusr/routes/identity_migration.rs`

**依赖：** T1、T2

**步骤：**

1. 新增 user 启用状态、person_kind、developer_enabled、auth_epoch、真实可空邮箱字段；不改旧 ID/username/role/password。
2. 创建 identity_provider、identity_subject、identity_invitation、identity_password_reset、identity_collaboration_candidate、identity_user_group、identity_group_member。
3. 添加主体/tombstone、秘密摘要、成员唯一性及状态 CHECK；迁移不填猜测 issuer，先写新/旧数据库结构测试。

**验证：** 运行 `cargo test -p cobotrix-root identity_migration -- --test-threads=1`，账户字段默认值和唯一约束正确，历史账户记录原值保留，重复迁移无重复表或记录；新增用例必须实际被执行。

#### T9: 增加应用治理迁移

**文件：** `cobotusr/src/migration/m20261109_000002_identity_applications.rs`、`cobotusr/src/migration/mod.rs`、`tests/cobotusr/routes/identity_migration.rs`

**依赖：** T8、T3、T4

**步骤：**

1. 扩展 application/oauth_client 管理状态，旧记录统一标记 legacy_unmanaged；保留 app_key/client_id/secret hash。
2. 创建 identity_app_member、identity_app_assignment、identity_app_test_user、identity_access_generation、identity_app_environment、identity_app_config_version、identity_app_approval、identity_admin_activation。
3. 创建 identity_introspection_reader、identity_request_operation、identity_migration_conflict；设置单 owner、单环境绑定、恰一分配目标和幂等唯一约束。

**验证：** 运行 `cargo test -p cobotrix-root identity_migration -- --test-threads=1`，孤立旧客户端仍有明确历史状态，无自动信任绑定；非法双目标分配、重复 owner/环境/幂等键被数据库约束拒绝；新增用例必须实际被执行。

#### T10: 增加会话、审计与通知迁移

**文件：** `cobotusr/src/migration/m20261109_000003_identity_sessions.rs`、`cobotusr/src/migration/mod.rs`、`tests/cobotusr/routes/identity_migration.rs`

**依赖：** T9、T5

**步骤：**

1. 创建 identity_sso_session、identity_authorization_flow、identity_action_flow、identity_oidc_rp_session、identity_audit_event、identity_logout_delivery。
2. 扩展 security_session、oauth_authorization_code、oauth_consent 的可空 managed 关联；新关联成组完整，历史 NULL 保留兼容。
3. 授权码新增 digest 唯一列并保留旧 code 非空唯一列；新增消费、lease、投递事件/RP 唯一约束及按查询条件的索引。

**验证：** 运行 `cargo test -p cobotrix-root identity_migration -- --test-threads=1`，新旧会话可并存，缺少 managed 根/代次的记录被拒绝，代码和通知唯一性生效；新增用例必须实际被执行。

#### T11: 映射全部身份实体

**文件：** `cobotusr/src/entity/mod.rs`、`cobotusr/src/entity/user.rs`、`cobotusr/src/entity/application.rs`、`cobotusr/src/entity/oauth_client.rs`、`cobotusr/src/entity/security_session.rs`、`cobotusr/src/entity/oauth_authorization_code.rs`、`cobotusr/src/entity/oauth_consent.rs`、`cobotusr/src/entity/identity_provider.rs`、`cobotusr/src/entity/identity_subject.rs`、`cobotusr/src/entity/identity_invitation.rs`、`cobotusr/src/entity/identity_password_reset.rs`、`cobotusr/src/entity/identity_collaboration_candidate.rs`、`cobotusr/src/entity/identity_user_group.rs`、`cobotusr/src/entity/identity_group_member.rs`、`cobotusr/src/entity/identity_app_member.rs`、`cobotusr/src/entity/identity_app_assignment.rs`、`cobotusr/src/entity/identity_app_test_user.rs`、`cobotusr/src/entity/identity_access_generation.rs`、`cobotusr/src/entity/identity_app_environment.rs`、`cobotusr/src/entity/identity_app_config_version.rs`、`cobotusr/src/entity/identity_app_approval.rs`、`cobotusr/src/entity/identity_admin_activation.rs`、`cobotusr/src/entity/identity_introspection_reader.rs`、`cobotusr/src/entity/identity_request_operation.rs`、`cobotusr/src/entity/identity_migration_conflict.rs`、`cobotusr/src/entity/identity_sso_session.rs`、`cobotusr/src/entity/identity_authorization_flow.rs`、`cobotusr/src/entity/identity_action_flow.rs`、`cobotusr/src/entity/identity_oidc_rp_session.rs`、`cobotusr/src/entity/identity_audit_event.rs`、`cobotusr/src/entity/identity_logout_delivery.rs`

**依赖：** T8、T9、T10

**步骤：**

1. 每张新表独立映射 SeaORM Model、ActiveModel、Relation，保留迁移中的 UTC 毫秒/UUID/i32 类型。
2. 修改旧实体的新增字段并修复构造点；敏感实体不直接用作 HTTP 输出。
3. 增加字段往返与外键关联测试，检查数据库 CHECK 与枚举转换一致。

**验证：** 运行 `cargo check --workspace --all-targets` 和 identity_migration 的实体往返测试，编译通过且无字段类型/空值规则偏差。

#### T12: 固定 issuer 并回填主体

**文件：** `cobotusr/src/service/identity/account.rs`、`cobotusr/src/service/identity/repository_account.rs`、`cobotusr/src/lib.rs`、`tests/cobotusr/routes/identity_migration.rs`

**依赖：** T11

**步骤：**

1. 在启动初始化事务中核对/首次写入固定 identity_provider，禁止从请求 Host 推导 issuer。
2. 幂等回填现有 user.id.to_string() 的主体；保留 tombstone，并将不一致 issuer 记录为冲突。
3. 重复启动只核对数据；不得新建第二身份或静默归一化历史 issuer。

**验证：** 运行 `cargo test -p cobotrix-root identity_migration -- --test-threads=1`，重复启动主体数量稳定，用户名/角色变化不改 sub，issuer 不一致给出冲突；新增用例必须实际被执行。

#### T13: 实现统一锁规划

**文件：** `cobotusr/src/service/identity/repository.rs`、`cobotusr/src/service/security/repository.rs`、`cobotusr/src/service/security/repository_revocation.rs`、`tests/cobotusr/routes/identity_session.rs`

**依赖：** T6、T11

**步骤：**

1. 先测试 application/environment → group → 排序 user → SSO → flow/action/code → RP → grant → security_session → family → credential 的锁序。
2. 实现稳定 ID 定位、同层排序及锁定后重载；禁止事务途中补入逆序锁。
3. 让旧 serialize_user_roots 与新用户/环境/group/generation 屏障共用规划入口。

**验证：** 运行 `cargo test -p cobotrix-root identity_session -- --test-threads=1`，两种相反输入顺序生成同一锁计划，签发/撤销不读取 HTTP guard 旧快照；新增用例必须实际被执行。

#### T14: 实现共享 PostgreSQL 事务执行器

**文件：** `cobotusr/src/service/identity/repository.rs`、`cobotusr/src/service/security/repository.rs`、`tests/cobotusr/routes/identity_session.rs`

**依赖：** T6、T11、T13

**步骤：**

1. 复用 PgSecurityRepository 的连接及 SecurityTransaction 操作实现 PgIdentityRepository，业务与凭据写入同一 REPEATABLE READ 事务。
2. 沿用原 2 秒 SecurityBudget，序列化/死锁最多三次整事务重试，每次使用原剩余预算。
3. 注入 commit/签名/时间失败，验证 rollback、预算耗尽和不确定提交不能返回普通成功。

**验证：** 运行 `cargo test -p cobotrix-root identity_session -- --test-threads=1`，回滚同时撤掉身份与安全写入，重试不重置期限，超预算无半提交；新增用例必须实际被执行。

#### T15: 实现账户及动作仓储

**文件：** `cobotusr/src/service/identity/repository_account.rs`、`tests/cobotusr/routes/identity_account.rs`

**依赖：** T11、T14

**步骤：**

1. 实现邀请/重置摘要查找、条件消费、username/用户屏障、真实 profile、资格和候选可见性读写。
2. 加入 action flow 的短期浏览器证明摘要和一次性消费，明确 target_user_id 与 issued auth_epoch 校验。
3. 先测过期、撤销、撞名、并发消费、目标不符，再实现约束 SQL。

**验证：** 运行 `cargo test -p cobotrix-root identity_account -- --test-threads=1`，两个消费者仅一次成功，失败不留账户/资格/消费记录；新增用例必须实际被执行。

#### T16: 实现应用与不可变版本仓储

**文件：** `cobotusr/src/service/identity/repository_application.rs`、`tests/cobotusr/routes/identity_application.rs`

**依赖：** T11、T14

**步骤：**

1. 实现应用/owner/两环境/client/信任关联的原子写入和可安全查询 operation。
2. 实现只插入版本、审批快照、active pointer/client 镜像/epoch 原子切换与 expected_revision 冲突检查。
3. 先测版本不允许覆盖、故障回滚和幂等键 request_hash 不同，再实现事务命令。

**验证：** 运行 `cargo test -p cobotrix-root identity_application -- --test-threads=1`，版本不可变，失败保留旧 pointer/binding，operation 摘要不含秘密；新增用例必须实际被执行。

#### T17: 实现准入、分配和 generation 仓储

**文件：** `cobotusr/src/service/identity/repository_admission.rs`、`tests/cobotusr/routes/identity_assignment.rs`

**依赖：** T11、T14

**步骤：**

1. 以一致主库快照加载 managed app、active 配置、账户/根状态、直接/组/开发成员/测试来源。
2. 实现首次 generation 创建、最终失权递增和多来源集合重算；分配修改触达共同屏障。
3. 先测多组重叠和不同环境/用户隔离，再实现 evaluate_admission 的事实装配。

**验证：** 运行 `cargo test -p cobotrix-root identity_assignment -- --test-threads=1`，保留其他来源不递增 generation，最终失权仅变更目标 user/environment；新增用例必须实际被执行。

#### T18: 实现 SSO、RP 与授权码仓储

**文件：** `cobotusr/src/service/identity/repository_session.rs`、`tests/cobotusr/routes/identity_session.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T11、T14

**步骤：**

1. 实现根摘要查询、固定期限、flow/action/RP 关联和一次性完成，中心 Cookie 不写 revocation_parent_jti。
2. 新代码仅以 digest 查找；旧 code 列写带类型前缀的摘要表示，禁止明文副本。
3. 先测同一 code/flow 并发消费、根到期、关联不完整，再实现与安全凭据登记共事务接口。

**验证：** 运行 `cargo test -p cobotrix-root identity_protocol -- --test-threads=1`，一次性代码只消费一次，数据库不含原代码秘密，失败不形成孤立 RP/凭据；新增用例必须实际被执行。

#### T19: 实现提交一致的身份审计

**文件：** `cobotusr/src/service/identity/repository.rs`、`cobotusr/src/service/identity/repository_outbox.rs`、`tests/cobotusr/routes/identity_application.rs`、`tests/cobotusr/routes/identity_account.rs`

**依赖：** T7、T14

**步骤：**

1. 实现 append_audit，成功事件与账户/应用/分配写入同一 commit。
2. 拒绝事务回滚后独立记失败事件，记录固定原因和脱敏摘要；历史 recording 可按 request_id 合并读取。
3. 先测审批 commit 失败与拒绝事件，禁止有成功 audit 却无生效状态。

**验证：** 运行 `cargo test -p cobotrix-root identity_application -- --test-threads=1`，成功事件有对应提交状态，失败只能记录拒绝/失败，敏感载荷已脱敏；新增用例必须实际被执行。

#### T20: 实现持久退出 outbox

**文件：** `cobotusr/src/service/identity/repository_outbox.rs`、`tests/cobotusr/routes/identity_session.rs`

**依赖：** T10、T14、T19

**步骤：**

1. 实现 enqueue_logout、事件/RP 幂等、SKIP LOCKED lease、过期租约回收和投递状态写入。
2. 保存批准 endpoint 快照、短错误和 token_jti，不落完整 Logout Token。
3. 先测多 worker 领取及重启回收，保证中心撤销事务不等待外部请求。

**验证：** 运行 `cargo test -p cobotrix-root identity_session -- --test-threads=1`，同一事件/RP 只入队一次，租约中不被重复领取，崩溃后任务可回收；新增用例必须实际被执行。

### C：安全生命周期基础

#### T21: 扩展安全快照与 managed 状态判断

**文件：** `cobotusr/src/service/security/repository.rs`、`cobotusr/src/service/security/lifecycle.rs`、`cobotusr/src/service/security/repository_bounds.rs`、`tests/cobotusr/routes/identity_session.rs`

**依赖：** T4、T17、T18

**步骤：**

1. 为 CredentialSnapshot 增加可空根/RP/managed facts，status_for_snapshot 检查根、auth/environment epoch、generation 和配置。
2. 已纳管上下文强制完整；仅数据库标记的 legacy 路径接受历史 NULL，请求参数不能选择旧分支。
3. 验证父会话的 managed 失效传递到已有派生凭据；身份 bounds 为空且不放大 MCP 权限。

**验证：** 运行 `cargo test -p cobotrix-root identity_session -- --test-threads=1`，撤销根、失权或配置切换使旧凭据无效，legacy 合法用例保留原边界；新增用例必须实际被执行。

#### T22: 实现明确范围的身份撤销

**文件：** `cobotusr/src/service/identity/session.rs`、`cobotusr/src/service/security/revocation.rs`、`cobotusr/src/service/security/repository_revocation.rs`、`tests/cobotusr/routes/identity_session.rs`

**依赖：** T20、T21

**步骤：**

1. 实现 SessionService::revoke 的当前根、用户全部、user/environment、具体授权、应用两环境范围。
2. 同事务更新 epoch/generation 或 revoked 状态、取消待用代码、撤销关联安全授权并记录 audit/outbox。
3. 先测两个浏览器、两应用、两用户及重新授予/启用，避免扩大范围或恢复旧令牌。

**验证：** 运行 `cargo test -p cobotrix-root identity_session -- --test-threads=1`，当前/全局/单应用撤销范围准确，重新启用/授予不复活历史授权；新增用例必须实际被执行。

#### T23: 扩展标准身份签名上下文

**文件：** `libs/auth/src/claims.rs`、`libs/auth/src/token_manager.rs`、`cobotusr/src/service/security/lifecycle.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T5、T21

**步骤：**

1. IdTokenClaims 增加受 scope 控制的真实 profile、auth_time、sid；SessionTokenSigner 接收独立 IdentityTokenContext。
2. 保留 strict access JWT 格式，身份 scopes 在 registry/session 及协议响应中读取。
3. 先测两个 client_id 的 aud/sid/nonce、缺真实邮箱与刷新 auth_time，再实现 RSA 签名。

**验证：** 运行 `cargo test -p auth` 和 identity_protocol 测试，标准 ID Token 可用 JWKS 验证且不泄漏业务角色/虚构邮箱。

#### T24: 增加中心与匿名 CSRF 绑定

**文件：** `libs/auth/src/security/csrf.rs`、`cobotusr/src/service/security/browser.rs`、`tests/cobotusr/routes/identity_session.rs`

**依赖：** T5

**步骤：**

1. 区分中心根摘要、匿名浏览器证明和既有 refresh CSRF 的绑定类型。
2. 先测严格 Origin、Cookie/请求头双证据、错误绑定、过期和多标签读取，再实现生成/验证适配。
3. 普通 GET 不每次覆盖稳定 CSRF Cookie，action/flow 证据不能作为另一个写入的 CSRF。

**验证：** 运行 `cargo test -p auth csrf` 和 identity_session 测试，三类绑定不可互换，合法并发恢复不产生证明竞争。

#### T25: 增加部署及寿命配置

**文件：** `cobotusr/src/config.rs`、`cobotusr/src/lib.rs`、`src/cobotusr.rs`、`tests/cobotusr/routes/identity_migration.rs`

**依赖：** T12、T24

**步骤：**

1. 定义 identity 默认根 12 小时、flow 10 分钟、未完成 flow 最多 8、code 60 秒、access 5 分钟、邀请 24 小时和重置 30 分钟。
2. 生产校验 HTTPS、固定 issuer 和持久 RSA key；缺 key 启动失败，不再静默替换；显式 loopback 开发使用非 __Host 名称。
3. 定义退出发送 timeout/重试/私网 allowlist 及保留期配置，边界和错误不泄漏敏感路径内容。

**验证：** 运行 `cargo test -p cobotrix-root identity_migration -- --test-threads=1`，生产缺密钥或 HTTP 配置被拒绝，已有 JWKS 不变，显式本地开发配置可启动；新增用例必须实际被执行。

#### T26: 实现过期数据清理边界

**文件：** `cobotusr/src/service/identity/repository_session.rs`、`cobotusr/src/service/identity/repository_outbox.rs`、`cobotusr/src/service/security/repository.rs`、`tests/cobotusr/routes/identity_session.rs`

**依赖：** T20、T21、T25

**步骤：**

1. 增加到期 flow/action 和已超过保留期安全 operation 摘要清理查询。
2. 根/RP tombstone 与邀请/重置消费证据按关联有效性和审计期限保留；仍有有效 family/后代的已消费 refresh 摘要禁止删除。
3. 先测试活跃后代、已全部过期、清理重试，再实现分批非治理清理；不把清理作为撤销生效条件。

**验证：** 运行 `cargo test -p cobotrix-root identity_session -- --test-threads=1`，清理后真正 refresh 重放仍可被识别，过期临时流程可删且活跃授权证据保留；新增用例必须实际被执行。

### D：账户与应用治理

#### T27: 实现管理员邀请、重置签发和秘密交换

**文件：** `cobotusr/src/service/identity/account.rs`、`cobotusr/src/service/identity/sso.rs`、`tests/cobotusr/routes/identity_account.rs`

**依赖：** T15、T19、T24、T25

**步骤：**

1. 先测试只有管理员可签发、默认期限、提前撤销以及只展示一次秘密。
2. 签发邀请/重置时固定用途、目标、person_kind/资格及 auth_epoch；生成 action flow 后跳到无秘密页面。
3. 入口交换只加载中心资源并保护 Referrer/缓存，保留独立 OIDC flow 关联，不接受自选 return_to。

**验证：** 运行 `cargo test -p cobotrix-root identity_account -- --test-threads=1`，无权/过期/篡改失败，有效交换后页面地址不含秘密且原 OIDC 请求未变；新增用例必须实际被执行。

#### T28: 实现邀请建立新账户

**文件：** `cobotusr/src/service/identity/account.rs`、`tests/cobotusr/routes/identity_account.rs`

**依赖：** T12、T15、T19、T27

**步骤：**

1. 先写正常开户、撞名、消费竞态和账户插入失败测试。
2. AccountService::accept_invitation 原子创建账户/主体、授予明确资格并消费 invitation/action，不读取请求自选角色或分配。
3. 返回安全 continuation 和后续认证动作；初始 profile 只取真实资料，不给任意应用访问。

**验证：** 运行 `cargo test -p cobotrix-root identity_account -- --test-threads=1`，一次邀请最多一个主体，撞名不合并，开户后无未授予资格和应用分配；新增用例必须实际被执行。

#### T29: 实现已有账户接受邀请

**文件：** `cobotusr/src/service/identity/account.rs`、`tests/cobotusr/routes/identity_account.rs`

**依赖：** T28

**步骤：**

1. 先测试未认证、认证账户与目标不符、撞名登录后接受、重复提交。
2. 复用认证后的现有 user/sub，只附加邀请中明确授予的资格并消费动作。
3. 保存原授权 continuation；新旧账户路径都不依赖浏览器填写 sub 或任意客户端。

**验证：** 运行 `cargo test -p cobotrix-root identity_account -- --test-threads=1`，已有账户接受不创建第二身份，错误账户拒绝且邀请未被消费；新增用例必须实际被执行。

#### T30: 实现账户启用和资格保护

**文件：** `cobotusr/src/service/identity/account.rs`、`cobotusr/src/service/identity/repository_account.rs`、`tests/cobotusr/routes/identity_account.rs`

**依赖：** T2、T16、T22、T29

**步骤：**

1. 先测试唯一负责人资格撤销、最后管理员停用、有效交接/应用停用后的允许更新。
2. AccountService::set_enabled/set_developer 在锁后重检 actor、expected_revision 与 owner 列表。
3. 停用递增 auth_epoch 并同事务撤销全部身份授权；重新启用保持最新 epoch，主体不变。
4. 撤销开发者资格时重算其开发成员准入来源，最终失权才递增 generation 并撤销相应授权。

**验证：** 运行 `cargo test -p cobotrix-root identity_account -- --test-threads=1`，无负责人活跃应用与最后管理员被保护，停用拒绝旧身份获取且启用不恢复旧会话；新增用例必须实际被执行。

#### T31: 实现本人真实资料管理

**文件：** `cobotusr/src/service/identity/account.rs`、`tests/cobotusr/routes/identity_account.rs`

**依赖：** T12、T15、T19

**步骤：**

1. 先写本人/他人资料更新和邮箱真实性测试。
2. 只允许修改认可 profile 字段，保留 username 唯一语义；不可更改主体、资格、角色或验证标记。
3. 无邮件验证能力时不给 email_verified=true，返回最小本人 DTO 和审计摘要。

**验证：** 运行 `cargo test -p cobotrix-root identity_account -- --test-threads=1`，资料修改不改 iss/sub，缺邮箱不合成，越权字段更新拒绝；新增用例必须实际被执行。

#### T32: 实现密码修改和管理员重置完成

**文件：** `cobotusr/src/service/identity/account.rs`、`cobotusr/src/service/user.rs`、`tests/cobotusr/routes/identity_account.rs`

**依赖：** T15、T22、T27、T30

**步骤：**

1. 先测试旧密码错误、过期/旧 auth_epoch reset、并发完成和事务失败。
2. change_password/reset_password 原子替换散列、递增 auth_epoch、撤销全部根/授权/旧代码与旧 reset，并写 audit/outbox。
3. 重置后要求新密码重新认证，保留但不完成 OIDC flow；旧 user 改密方法委托该编排。

**验证：** 运行 `cargo test -p cobotrix-root identity_account -- --test-threads=1`，仅一次重置成功，旧密码/旧 reset/旧 refresh 不可用，失败不变更密码或撤销范围；新增用例必须实际被执行。

#### T33: 实现候选隐私和应用成员管理

**文件：** `cobotusr/src/service/identity/account.rs`、`cobotusr/src/service/identity/application.rs`、`tests/cobotusr/routes/identity_application.rs`

**依赖：** T15、T16、T17、T22、T30

**步骤：**

1. 先测试负责人/维护者权限、他应用查询、合作方目录和明确候选授权。
2. 只读取本人、既有成员及管理员授权候选；添加维护者要求目标启用且具备必要资格。
3. 移除维护者重算开发环境来源，仅最终失去来源时撤销；生产分配不从成员关系推导。

**验证：** 运行 `cargo test -p cobotrix-root identity_application -- --test-threads=1`，合作方不能枚举全目录，移除成员不错误撤销另有来源的访问；新增用例必须实际被执行。

#### T34: 实现负责人原子交接

**文件：** `cobotusr/src/service/identity/application.rs`、`tests/cobotusr/routes/identity_application.rs`

**依赖：** T16、T19、T30、T33

**步骤：**

1. 先测两个并发交接、目标无资格、admin_configured 第三方负责人和审批中的交接。
2. 锁定应用及排序用户后更新单 owner，原维护关系和访问来源按明确请求处理。
3. 变更 owner 使待审批快照 obsolete；任何成功提交都保留唯一合法责任人。
4. 重算交接影响的开发准入来源；保留维护者资格时不错误撤销，最终失权则更新代次与通知。

**验证：** 运行 `cargo test -p cobotrix-root identity_application -- --test-threads=1`，只有一名 owner，资格不足拒绝，交接不自动授予生产访问且旧申请失效；新增用例必须实际被执行。

#### T35: 实现扁平用户组管理

**文件：** `cobotusr/src/service/identity/assignment.rs`、`tests/cobotusr/routes/identity_assignment.rs`

**依赖：** T15、T17、T19

**步骤：**

1. 先测试管理员组创建、命名/成员重复、revision 冲突和合作方默认不入员工组。
2. 实现组基本管理及新增成员，不支持嵌套/继承；成员变化触达 group/user 屏障。
3. 加入成员时只重算已明确分配应用，不创建隐式企业全员准入。

**验证：** 运行 `cargo test -p cobotrix-root identity_assignment -- --test-threads=1`，扁平组唯一性成立，新成员仅获得既有明确分配，普通开发者修改拒绝；新增用例必须实际被执行。

#### T36: 实现应用和两环境一次注册

**进度（2026-11-09）：** 自助注册服务与 HTTP 已接入，owner 强制本人、需要开发者资格、两环境/client/trust/operation/audit 同事务、生产 disabled。3 项服务和 2 项开发者 HTTP 筛选通过；管理员直接接入仍属 T41，完整公开旧注册旁路仍属 T66。

**文件：** `cobotusr/src/service/identity/application.rs`、`tests/cobotusr/routes/identity_application.rs`

**依赖：** T16、T19、T25、T30、T34

**步骤：**

1. 先写完整注册、错误 URI/owner/资格、信任绑定插入失败用例。
2. ApplicationService::register 在一个事务创建应用、owner、开发/生产环境、独立 client 和 security_session_client。
3. 开发配置有效，生产 active pointer 为空且 trust disabled；Web 返回新秘密，SPA 无 secret/机器 grant，身份 bounds 为空。

**验证：** 运行 `cargo test -p cobotrix-root identity_application -- --test-threads=1`，一次注册获得完整开发接入关系，提交失败无半成品，生产批准前不能签发；新增用例必须实际被执行。

#### T37: 实现幂等注册和安全操作回执

**进度（2026-11-09）：** 自助注册同 actor/键/规范请求 hash 的并发与重试、安全操作回执已实现并局部核验；响应丢失后的单环境密钥重置和注册未知提交专用用例尚待补齐。

**文件：** `cobotusr/src/service/identity/application.rs`、`cobotusr/src/service/identity/repository_application.rs`、`tests/cobotusr/routes/identity_application.rs`

**依赖：** T36

**步骤：**

1. 先测同键同请求、同键不同请求、并发创建、commit 后回包丢失。
2. 对规范化 request_hash 写 identity_request_operation 并返回 RegistrationReceipt；查询仅返回资源和提交摘要。
3. 秘密仅首次成功响应交付，重试不重新创建或恢复明文；丢失响应引导查询资源后单环境 reset。

**验证：** 运行 `cargo test -p cobotrix-root identity_application -- --test-threads=1`，同一操作只有一个应用，冲突返回安全结果，重试与查询不含旧秘密；新增用例必须实际被执行。

#### T38: 实现开发配置不可变更新

**文件：** `cobotusr/src/service/identity/application.rs`、`tests/cobotusr/routes/identity_application.rs`

**依赖：** T16、T22、T36

**步骤：**

1. 先测非成员更新、非法范围/预授权、版本冲突和 epoch 切换失败。
2. propose 为开发环境插入新版本，安全配置变更原子切 pointer/client mirror/trust 并撤销旧环境授权。
3. 显示名称/说明更新不改变安全版本；普通开发者不能增加测试用户或企业预授权。

**验证：** 运行 `cargo test -p cobotrix-root identity_application -- --test-threads=1`，新开发安全配置提交后才生效，失败保留旧值，旧代次令牌不复活；新增用例必须实际被执行。

#### T39: 实现生产提交和固定快照

**文件：** `cobotusr/src/service/identity/application.rs`、`tests/cobotusr/routes/identity_application.rs`

**依赖：** T16、T34、T35、T38

**步骤：**

1. 先写未完成配置、重复提交、owner/分配变化和 suspended 状态用例。
2. submit 固定 version、owner、当前访问集合、退出能力及限制确认记录，创建 pending 并递增 revision。
3. 运行时继续使用旧 active 配置；安全字段、owner 或分配变化使受影响申请 obsolete，不能变更旧快照。

**验证：** 运行 `cargo test -p cobotrix-root identity_application -- --test-threads=1`，待批准配置不可运行，已有 active 保持可用，敏感变更使旧申请不可批准；新增用例必须实际被执行。

#### T40: 实现独立审核和生产原子切换

**文件：** `cobotusr/src/service/identity/application.rs`、`tests/cobotusr/routes/identity_application.rs`

**依赖：** T19、T22、T39

**步骤：**

1. 先测自批、旧 snapshot/version、重复批准、批准与编辑交错、commit/签名失败。
2. review 验证管理员、requested_by 差异和锁后快照，写 approved/rejected 结果。
3. 批准同事务切 active pointer、client mirror、trust、lifecycle_epoch 并撤销旧代码/授权、写 audit/outbox。

**验证：** 运行 `cargo test -p cobotrix-root identity_application -- --test-threads=1`，自批和过时申请拒绝，切换失败不改旧配置或生成成功审计，重复批准不重复生效；新增用例必须实际被执行。

#### T41: 实现管理员直接接入第三方

**文件：** `cobotusr/src/service/identity/application.rs`、`tests/cobotusr/routes/identity_application.rs`

**依赖：** T36、T40

**步骤：**

1. 先测试无管理资格、LocalOnly 未确认限制和 admin owner 无开发者资格。
2. 通过 admin_configured/admin_activation 建立第三方完整环境、配置/访问快照与独立操作审计。
3. LocalOnly/Online/BackchannelAndOnline 按实际能力登记，有限退出承诺必须由管理员明确接受。

**验证：** 运行 `cargo test -p cobotrix-root identity_application -- --test-threads=1`，合法管理员可直接配置且不被自动授予开发者资格，未确认退出限制不能启用；新增用例必须实际被执行。

#### T42: 实现生产客户端类型切换

**文件：** `cobotusr/src/service/identity/application.rs`、`cobotusr/src/service/identity/repository_application.rs`、`tests/cobotusr/routes/identity_application.rs`

**依赖：** T40

**步骤：**

1. 先测试 Web→SPA、SPA→Web、候选未批准、批准失败和旧 secret 使用。
2. 类型变化建立新 client 候选，批准后原子切环境主绑定、禁用旧 client 并撤销旧授权。
3. 保留旧 ID 历史；SPA 不沿用 Web secret，回执明确新的 client_id 和重新接入需求。

**验证：** 运行 `cargo test -p cobotrix-root identity_application -- --test-threads=1`，批准前候选不能签发，批准后只有新绑定有效，Web 密钥不能作为 SPA 凭据；新增用例必须实际被执行。

#### T43: 实现单环境密钥重置

**文件：** `cobotusr/src/service/identity/application.rs`、`tests/cobotusr/routes/identity_application.rs`

**依赖：** T19、T37、T40

**步骤：**

1. 先测无权 reset、同幂等键、旧/新密钥、跨环境与回包丢失。
2. rotate_secret 同事务替换散列和 secret_revision，记录 operation/audit，首次响应仅显示当前环境新秘密。
3. public SPA 拒绝 client secret reset；只读/重复查询不能取回已交付明文。

**验证：** 运行 `cargo test -p cobotrix-root identity_application -- --test-threads=1`，提交后旧 secret 失效，新 secret 可用，另一环境和无权失败状态不变；新增用例必须实际被执行。

#### T44: 实现后端内省查询凭据

**进度（2026-11-09）：** 独立 reader 创建、轮换/停用、精确 audience、应用/环境路径绑定、revision 和审计服务/HTTP 已接入；内省、跨目标及 token/revoke 用途隔离已有证据。生产审批、完整接入页、秘密重置幂等契约仍待补齐，保持局部状态。

**文件：** `cobotusr/src/service/identity/application.rs`、`cobotusr/src/service/identity/protocol.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T16、T19、T43

**步骤：**

1. 先测 SPA backend reader 注册、跨环境目标、非批准 audience、停用和重置。
2. 为明确保护 API 创建独立 reader_id/hash/revision，仅供服务端保存并一次性交付。
3. 将 reader 身份与 client token 认证分别建模；它不能用于授权码兑换、client_credentials、业务 API 或 MCP。

**验证：** 运行 `cargo test -p cobotrix-root identity_protocol -- --test-threads=1`，合法 reader 绑定目标环境/audience，跨环境或 token 端点使用被拒绝；新增用例必须实际被执行。

#### T45: 实现用户/组应用访问分配

**文件：** `cobotusr/src/service/identity/assignment.rs`、`tests/cobotusr/routes/identity_assignment.rs`

**依赖：** T17、T19、T22、T35、T40

**步骤：**

1. 先测试管理员 direct/group 分配、重复操作、开发测试用户与普通开发者越权。
2. assign/unassign 重算 user/environment 的来源与 generation，最终失权时同事务撤销授权并通知。
3. 不以 owner/管理员/同意代替生产分配；pending 审批涉及的分配变化更新 revision 并使快照 obsolete。

**验证：** 运行 `cargo test -p cobotrix-root identity_assignment -- --test-threads=1`，无分配身份拒绝，多来源取消一项仍可用，最后来源撤回只影响目标环境；新增用例必须实际被执行。

#### T46: 实现组成员移除和组删除

**文件：** `cobotusr/src/service/identity/assignment.rs`、`cobotusr/src/service/identity/repository_admission.rs`、`tests/cobotusr/routes/identity_assignment.rs`

**依赖：** T35、T45

**步骤：**

1. 先测组移除、取消组应用分配、删除组、重叠来源和超预算。
2. 集合 SQL 计算受影响用户，按统一锁序更新 group revision、generation、授权撤销及 outbox。
3. 全部状态同一次提交；无法在原 2 秒预算提交时整体回滚并明确可重试，不报告后台已完成。
4. 组成员/组分配变化使相关待审批访问快照 obsolete；批准时重检 group 与访问范围 revision。

**验证：** 运行 `cargo test -p cobotrix-root identity_assignment -- --test-threads=1`，移除后目标用户失权，无关应用/用户可用；超预算不留下部分组变更；新增用例必须实际被执行。

#### T47: 实现应用停用和受控恢复

**文件：** `cobotusr/src/service/identity/application.rs`、`tests/cobotusr/routes/identity_application.rs`

**依赖：** T22、T34、T40、T45

**步骤：**

1. 先测两环境停用、恢复时 owner 无效、未批准配置和并发签发。
2. suspend 递增两环境 epoch、撤销相关授权/代码并写 outbox；中心根可继续用于其他应用。
3. resume 仅启用合法已批准配置和有效 owner，不回退 epoch 或恢复旧 credential。

**验证：** 运行 `cargo test -p cobotrix-root identity_application -- --test-threads=1`，停用阻止签发/刷新，恢复只允许新登录且旧令牌持续无效；新增用例必须实际被执行。

#### T48: 实现批准范围和版本化同意

**文件：** `cobotusr/src/service/identity/authorization.rs`、`cobotusr/src/service/identity/account.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T17、T18、T22、T40、T45

**步骤：**

1. 先测内部企业预授权、第三方首次同意、用户拒绝、新 scope 与 NULL 历史 consent。
2. 记录 user/environment/version 的获准 scopes；取 requested∩approved∩consented，不以同意替代分配。
3. 撤回个人同意仅撤销对应范围授权，单独显示企业预授权政策；旧同意不自动批准新版额外资料。

**验证：** 运行 `cargo test -p cobotrix-root identity_protocol -- --test-threads=1`，新增范围不能沿用旧同意，无分配用户同意后仍拒绝，撤回不影响其他应用；新增用例必须实际被执行。

### E：中心认证与 OIDC

#### T49: 实现稳定中心 Cookie 认证

**文件：** `cobotusr/src/service/identity/sso.rs`、`cobotusr/src/service/security/browser.rs`、`tests/cobotusr/routes/identity_session.rs`

**依赖：** T18、T21、T24、T25

**步骤：**

1. 先测正常读取、新标签读取、过期、撤销、账户/epoch 不符和固定期限。
2. authenticate_browser 按 Cookie digest 从主库验证根，生产写 __Host-cobotusr-sso 的 Secure/HttpOnly/Lax/Path=/ 且无 Domain。
3. 读取不轮换秘密、不延长 authenticated_at/expiry、不依赖 refresh Cookie，返回服务端本人上下文。

**验证：** 运行 `cargo test -p cobotrix-root identity_session -- --test-threads=1`，并发恢复使用同一有效根，过期/撤销不能通过本地 token 恢复，Cookie 属性符合配置；新增用例必须实际被执行。

#### T50: 实现密码认证和强制重新认证

**文件：** `cobotusr/src/service/identity/sso.rs`、`cobotusr/src/service/identity/account.rs`、`tests/cobotusr/routes/identity_session.rs`

**依赖：** T22、T28、T29、T32、T49

**步骤：**

1. 先写正确/错误密码、重复登录 flow、会话固定、prompt=login 与另一个标签已有根测试。
2. authenticate_password 验证匿名或 flow 证据/CSRF，锁后建立新根；已有旧根则原子撤销旧关联授权再替换。
3. 该 flow 记录 reauth_completed_at，不能用其他标签的旧认证满足强制请求；新开户可在同事务建立首次根。

**验证：** 运行 `cargo test -p cobotrix-root identity_session -- --test-threads=1`，重复 flow 不建多个根，强制重新认证确实验证密码并使旧根授权失效；新增用例必须实际被执行。

#### T51: 适配既有管理登录和刷新边界

**文件：** `cobotusr/src/route/auth.rs`、`cobotusr/src/service/security/browser.rs`、`cobotusr/src/service/security/lifecycle.rs`、`tests/cobotusr/routes/auth.rs`、`tests/cobotusr/routes/identity_session.rs`

**依赖：** T21、T23、T24、T50

**步骤：**

1. 先测旧 /login 返回结构、已有管理 bearer 调用及真正 refresh replay。
2. 旧浏览器登录委托同一中心认证服务，管理 token set 与中心根同事务关联，保持原专门业务边界。
3. 旧 /refresh 保留合法兼容调用，不作为中心恢复，不增加重放宽限；旧过期/撤销根仍影响关联凭据。

**验证：** 运行 identity_session、auth 和 security 相关筛选测试，管理返回兼容、合法刷新可用，旧秘密重放仍撤销 family。

#### T52: 实现独立授权 flow 和 continuation

**文件：** `cobotusr/src/service/identity/authorization.rs`、`cobotusr/src/service/identity/sso.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T18、T24、T25、T49

**步骤：**

1. 先测两 app 首次并发、flow 超时/上限、错误浏览器证明、重复完成和邀请 continuation。
2. 每个交互 flow 写独立 __Host-cobotusr-flow-{uuid}，最长 10 分钟、最多 8；URL 仅携带 flow_id。
3. 从服务端已验证 payload 恢复 callback/state/nonce/PKCE，完成或超时清相应证明，不接受任意 return_to。

**验证：** 运行 `cargo test -p cobotrix-root identity_protocol -- --test-threads=1`，两个应用流程不覆盖，错证明不能读取/完成 flow，邀请不能替换原回调；新增用例必须实际被执行。

#### T53: 实现授权请求严格解析

**文件：** `cobotusr/src/service/identity/authorization.rs`、`cobotusr/src/route/oidc.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T3、T5、T40、T52

**步骤：**

1. 先测无效 client/redirect、scope、缺 state/nonce、非 code/query、非 S256、prompt=none 混用。
2. 解析到已注册 active 环境及固定配置，错误 client/回调在中心返回，不重定向陌生地址。
3. 已验证回调才返回标准错误及原 state；客户端参数不能改 audience、owner 或管理状态。

**验证：** 运行 `cargo test -p cobotrix-root identity_protocol -- --test-threads=1`，非法回调不跳转，合法回调的协议拒绝保留 state，未批准生产不能授权；新增用例必须实际被执行。

#### T54: 实现直接 SSO 与 prompt/max_age 决策

**文件：** `cobotusr/src/service/identity/authorization.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T4、T48、T49、T50、T53

**步骤：**

1. 先测试已有根直接授权、缺分配/同意、prompt=login/consent/none、max_age 和认证时间。
2. authorize 输出计划中的 AuthorizationOutcome，每条成功分支都执行最新准入检查。
3. none 缺条件返回 login_required/consent_required/interaction_required 等适用标准错误，不呈现交互页。

**验证：** 运行 `cargo test -p cobotrix-root identity_protocol -- --test-threads=1`，已有有效根无密码复用，强制/超时要求新认证，无交互错误正确且不建交互页面；新增用例必须实际被执行。

#### T55: 实现同意完成与 RP/代码原子建立

**文件：** `cobotusr/src/service/identity/authorization.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T18、T19、T23、T48、T52、T54

**步骤：**

1. 先测同意接受/拒绝、完成前分配撤回、版本切换、并发重复提交和事务失败。
2. complete 校验 flow/中心/CSRF，锁后重检配置、根与分配；同意、RP sid、授权码及 flow 消费同事务落库。
3. 两个 RP 分别生成 sid 和代码，拒绝不更新 consent，代码默认 60 秒且仅持摘要。

**验证：** 运行 `cargo test -p cobotrix-root identity_protocol -- --test-threads=1`，一次完成仅一个有效代码/RP，拒绝不修改同意，撤回或失败无半授权；新增用例必须实际被执行。

#### T56: 实现标准授权码兑换

**文件：** `cobotusr/src/service/identity/authorization.rs`、`cobotusr/src/route/oidc.rs`、`cobotusr/src/service/oidc.rs`、`cobotusr/src/service/security/lifecycle.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T18、T21、T23、T25、T43、T55

**步骤：**

1. 先测 Web basic/post、SPA none、认证冲突、PKCE、过期/重放、跨环境及根/版本/代次变化。
2. 条件消费 code 时重检所有 managed 事实；签名、登记 access/可选 refresh 和消费同一事务提交。
3. access/refresh 不晚于根到期，返回当前 identity scope；commit/签名失败不返回 token，回包丢失不重放旧 token。

**验证：** 运行 `cargo test -p cobotrix-root identity_protocol -- --test-threads=1`，标准兑换成功、重复代码 invalid_grant、失败无凭据登记，身份 grant 无业务/MCP 权限；新增用例必须实际被执行。

#### T57: 接入 managed 刷新生命周期

**文件：** `cobotusr/src/service/security/lifecycle.rs`、`cobotusr/src/service/security/repository.rs`、`cobotusr/src/service/oidc.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T21、T23、T25、T56

**步骤：**

1. 先测合法轮换、同秘密并发/重放、根撤销/到期、scope 缩小和代次变化。
2. 逐次检查根、RP、active 配置、分配；保留 session/grant/family/bounds/scopes/auth_time/sid 与适用 nonce 规则。
3. 根/family 绝对到期不延长，不提供 offline_access；正常中心恢复不使用 refresh 秘密。

**验证：** 运行 `cargo test -p cobotrix-root identity_protocol -- --test-threads=1`，合法刷新时间/范围稳定，真正 replay 仍撤销 family，根或准入失效拒绝刷新；新增用例必须实际被执行。

#### T58: 实现最小身份 userinfo

**文件：** `cobotusr/src/service/identity/protocol.rs`、`cobotusr/src/route/oidc.rs`、`cobotusr/src/service/oidc.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T21、T23、T31、T48、T56

**步骤：**

1. 先测应用 bearer、管理 token、另一个环境 token、未注册签名 JWT、缺 profile/email 和 scope。
2. IdentityProtocolService::userinfo 验证签名、issuer、registry、绑定 audience/client、根/RP/当前准入和 scopes。
3. 保留新标准与旧发布地址，独立于管理 URI-RBAC；真实资料按最小披露返回，legacy 仅明确兼容。

**验证：** 运行 `cargo test -p cobotrix-root identity_protocol -- --test-threads=1`，openid 身份不要求业务权限，错误 token 被拒绝，缺资料省略且不合成邮箱；新增用例必须实际被执行。

#### T59: 实现注册表内省和标准撤销

**文件：** `cobotusr/src/service/identity/protocol.rs`、`cobotusr/src/route/oidc.rs`、`cobotusr/src/service/oidc.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T21、T22、T44、T56、T57

**步骤：**

1. 先测 Web 本 client、SPA backend reader、跨环境、未知/过期/撤销 token 与主库故障。
2. introspect 从 registry 与当前准入输出 active/scope/client_id/token_type/exp/sub/sid；无效 active=false，依赖错误 503。
3. 标准 revoke 只允许绑定目标范围，reader 不因内省资格获得撤销或 token 授权；旧 token 表只供 legacy 分支。

**验证：** 运行 `cargo test -p cobotrix-root identity_protocol -- --test-threads=1`，撤销提交后内省无效，跨 client 不能枚举身份，依赖故障不返回 active=true；新增用例必须实际被执行。

#### T60: 完成 discovery、CORS 和协议响应保护

**文件：** `cobotusr/src/route/oidc.rs`、`cobotusr/src/route/mod.rs`、`cobotusr/src/config.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T7、T25、T53、T56、T58、T59

**步骤：**

1. 先测试 discovery 能力与 endpoint、approved SPA origin、未批准 origin 和 Cookie 跨域请求。
2. 仅宣告实际已实现的 code/S256、认证/刷新/内省/退出能力；配置精确 CORS，不开放中心 Cookie/内省跨域访问。
3. 秘密/身份/flow 响应 no-store，交互页 no-referrer、禁止嵌入及必要 CSP；标准表单/JSON 错误保持兼容。

**验证：** 运行 `cargo test -p cobotrix-root identity_protocol -- --test-threads=1`，标准端点与 CORS 匹配 approved 配置，无跨域 Cookie 或未实现能力宣告；新增用例必须实际被执行。

### F：退出、兼容与后端入口

#### T61: 实现本人会话清单与退出

**文件：** `cobotusr/src/service/identity/session.rs`、`cobotusr/src/service/identity/account.rs`、`tests/cobotusr/routes/identity_session.rs`

**依赖：** T22、T48、T49

**步骤：**

1. 先测本人/他人根及 RP 列表、当前根退出、指定根退出、全部退出和 consent 撤回。
2. 返回脱敏会话与通知概况，清 Cookie 仅在成功退出当前浏览器根后发生。
3. 全局退出递增 auth_epoch，其他用户和无关范围保持有效；拒绝不能提前清理或伪报成功。

**验证：** 运行 `cargo test -p cobotrix-root identity_session -- --test-threads=1`，退出范围与会话归属匹配，当前/所有浏览器退出行为可区分且无他人信息；新增用例必须实际被执行。

#### T62: 实现 RP 发起退出确认

**文件：** `cobotusr/src/service/identity/logout.rs`、`cobotusr/src/route/oidc.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T18、T22、T24、T49、T61

**步骤：**

1. 先测试有效/过期可关联 hint、伪造签名/aud、client_id、专用退出地址、state 及他浏览器根。
2. GET 建立绑定退出确认 flow，不撤销；POST 校验 Cookie/hint 关联、flow、Origin 和 CSRF 后结束当前根。
3. 按专用 post_logout_redirect_uris 返回原 state；无中心根安全返回结果，hint 不能撤销别人的根。

**验证：** 运行 `cargo test -p cobotrix-root identity_protocol -- --test-threads=1`，GET 无状态撤销，合法 POST 正确返回，错证据/陌生地址/其他根请求被拒绝；新增用例必须实际被执行。

#### T63: 实现独立 Logout Token 签名

**文件：** `libs/auth/src/claims.rs`、`libs/auth/src/token_manager.rs`、`cobotusr/src/service/identity/logout.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T20、T23、T62

**步骤：**

1. 先测试 Logout Token 的 RSA/JWKS、iss/aud/iat/jti/sid/sub/events、默认 120 秒和禁 nonce。
2. 使用独立载荷类型生成短时通知，针对每个 RP；重试复用持久化 jti 并保留事件/RP 目标。
3. 数据库仅记录投递 jti/状态，不存完整 token，普通日志不输出签名载荷。

**验证：** 运行 `cargo test -p auth` 和 identity_protocol 测试，标准 JWT 校验器通过合法通知并拒绝 nonce/错误受众。

#### T64: 实现通知发送、重试和目标约束

**文件：** `cobotusr/src/service/identity/logout.rs`、`cobotusr/src/service/identity/repository_outbox.rs`、`tests/cobotusr/routes/identity_session.rs`

**依赖：** T20、T25、T63

**步骤：**

1. 先测 2xx、429/Retry-After、5xx、网络超时、普通 4xx、多 worker 与通知不可达。
2. LogoutDispatcher::dispatch_batch 在租约事务外以 form 发送 logout_token，默认 timeout 2 秒；按 5秒/30秒/2分/10分/1小时至 24 小时重试，普通 4xx 永久失败待手动重试。
3. 生产仅批准 HTTPS，连接解析地址核对显式私网 allowlist，拒绝 loopback/link-local/metadata 和重定向；2xx 只记录对端接受。

**验证：** 运行 `cargo test -p cobotrix-root identity_session -- --test-threads=1`，网络失败不回滚中心撤销，lease 重启可继续，未许可目标/重定向被拒绝，状态无“全部退出”误报；新增用例必须实际被执行。

#### T65: 装配身份服务与可取消 worker

**文件：** `cobotusr/src/lib.rs`、`src/cobotusr.rs`、`cobotusr/src/service/identity/mod.rs`、`cobotusr/src/service/mod.rs`、`tests/cobotusr/routes/identity_session.rs`

**依赖：** T12、T25、T26、T50、T59、T64

**步骤：**

1. 注入同一 repository、signer、可信时钟、预算和策略到账户/应用/SSO/协议/会话服务。
2. 随服务器启动退出 dispatcher 与清理 worker，使用取消信号、短租约和可控批次，关闭时停止领取并释放资源。
3. 先测重复初始化、worker 重启和 shutdown；外部请求不持有治理事务。

**验证：** 运行 `cargo test -p cobotrix-root identity_session -- --test-threads=1`，服务可启动/关闭，worker 不丢持久任务、不阻塞中心请求提交；新增用例必须实际被执行。

#### T66: 封闭旧账户、客户端和撤销旁路

**文件：** `cobotusr/src/route/auth.rs`、`cobotusr/src/route/user.rs`、`cobotusr/src/route/application.rs`、`cobotusr/src/route/oidc_admin.rs`、`cobotusr/src/service/user.rs`、`cobotusr/src/service/oidc.rs`、`cobotusr/src/service/security/revocation.rs`、`tests/cobotusr/routes/identity_migration.rs`、`cobotusr/src/route/user_authority.rs`、`cobotusr/src/route/user_permission.rs`、`cobotusr/src/route/role.rs`、`cobotusr/src/route/role_permission.rs`、`cobotusr/src/route/rbac.rs`、`cobotusr/src/service/role.rs`、`cobotusr/src/route/authority.rs`、`cobotusr/src/route/permission.rs`

**依赖：** T30、T32、T33、T38、T40、T47、T51、T59

**步骤：**

1. 先逐条请求旧 /register、目录、账户删改、角色赋权、OAuth 新建/编辑、kickout/revoke，覆盖普通开发者/合作方/管理员。
2. 无有效邀请 /register 拒绝；旧目录限制可见范围，删除转停用，资格/owner/密码变化走新服务。
3. 旧新建转受控注册或明确迁移说明，managed 配置编辑走版本服务，所有既有撤销路径检查新根；请求不能切 legacy。

**验证：** 运行 `cargo test -p cobotrix-root identity_migration -- --test-threads=1`，旧接口不能绕过准入/审批或泄露目录，旧合法管理行为保持实际授权边界；新增用例必须实际被执行。

#### T67: 实现历史扫描与显式纳管

**文件：** `cobotusr/src/service/identity/application.rs`、`cobotusr/src/service/identity/repository_application.rs`、`cobotusr/src/lib.rs`、`tests/cobotusr/routes/identity_migration.rs`

**依赖：** T12、T16、T22、T40、T41、T66

**步骤：**

1. 先测孤立 client、owner/issuer 冲突、已有关联、bootstrap 重启、纳管失败及重复请求。
2. 扫描只记录可解释关系/冲突；adopt_legacy 由管理员明确 owner、环境、类型、退出能力与分配，复用选定旧 client_id/key，另环境明确补建。
3. 同事务纳管及撤销旧代码/授权/consent，禁止静默降级；bootstrap 不追加 managed 回调或创建新不受控 client。

**验证：** 运行 `cargo test -p cobotrix-root identity_migration -- --test-threads=1`，sub/client/key/历史记录保留，未解冲突不纳管，成功纳管后旧授权失效且重启不扩大配置；新增用例必须实际被执行。

T67 补充核验：`identity_migration_adoption_` 覆盖孤立/关联保留、issuer/多应用/类型/快照冲突、同键并发仅一次秘密、COMMIT 故障无部分纳管、旧授权撤销和 bootstrap 无写入。接口及锁计划见 plan 9.1。

#### T68: 注册管理操作并装配权限分组

**文件：** `cobotusr/src/seed/m20261109_000004_identity_operations.rs`、`cobotusr/src/seed/mod.rs`、`cobotusr/src/route/mod.rs`、`tests/cobotusr/routes/identity_account.rs`、`tests/cobotusr/routes/identity_application.rs`

**依赖：** T30、T40、T45、T65、T66

**步骤：**

1. 先测试本人、开发者成员、企业操作权限三种 guard 和事务重检。
2. 新增管理操作目录并复用已有管理员授权来源；不自动授予普通新用户业务/MCP/管理员权限。
3. 分别装配 account、developer、admin Cookie route 层和独立应用 bearer 协议层，保持旧合法管理 bearer 路径。

**验证：** 运行 `cargo test -p cobotrix-root identity_account -- --test-threads=1`，普通账户可读本人，无资格/非成员/无操作权限直接请求被拒绝且无状态变化；新增用例必须实际被执行。

#### T69: 实现账户中心 HTTP 接口

**文件：** `cobotusr/src/route/account.rs`、`cobotusr/src/route/mod.rs`、`tests/cobotusr/routes/identity_account.rs`、`tests/cobotusr/routes/identity_session.rs`

**依赖：** T31、T32、T48、T61、T68

**步骤：**

1. 先写 session/profile/apps/consents/sessions/logout-all 的参数、401/403/404 和成功请求。
2. 按 plan 第 4 节提供本人 DTO，GET session 返回入口资格与当前 CSRF，写请求验证中心 Cookie/Origin/CSRF。
3. 密码/同意/会话操作调用既有身份编排，实体秘密不进入响应，GET 不轮换根。

**验证：** 运行 `cargo test -p cobotrix-root identity_account -- --test-threads=1`，本人 API 可用，访问他人或缺证据拒绝，中心恢复只用有效 Cookie；新增用例必须实际被执行。

#### T70: 实现登录、邀请、重置及 flow HTTP

**文件：** `cobotusr/src/route/identity_browser.rs`、`cobotusr/src/route/mod.rs`、`tests/cobotusr/routes/identity_account.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T27、T28、T29、T50、T52、T55、T68

**步骤：**

1. 先测试匿名/中心/action/flow 证据的正确与错误组合、参数缺失和超时。
2. 提供 identity/csrf、login/logout、actions、invitations/accept、password-resets/complete、flows/{id}/complete。
3. 成功后仅恢复已验证 continuation，原始秘密换取 action 后离开地址栏；完成请求重检中心与准入。

**验证：** 运行 `cargo test -p cobotrix-root identity_protocol -- --test-threads=1`，完整交互可继续原回调，错证据拒绝且不发生账户/同意/授权变化；新增用例必须实际被执行。

#### T71: 实现开发者控制台 HTTP

**进度（2026-11-09）：** 已接入自助注册、应用详情与成员范围分页、安全操作回执、reader 管理接口；成员/负责人、配置提案、审批、生命周期及秘密重置接口仍待实现。

**文件：** `cobotusr/src/route/developer.rs`、`cobotusr/src/route/mod.rs`、`tests/cobotusr/routes/identity_application.rs`

**依赖：** T33、T34、T37、T38、T39、T42、T43、T44、T68

**步骤：**

1. 先写应用创建/读取/编辑、成员/交接、versions/submit/secret-reset/integration/readers 及候选查询黑盒测试。
2. 按应用成员和环境做后端授权，创建/reset 支持 Idempotency-Key，敏感更新 expected_revision/If-Match。
3. 返回 active/pending/obsolete/suspended 的真实状态，接入摘要只使用 active 配置，秘密仅一次受控响应。

**验证：** 运行 `cargo test -p cobotrix-root identity_application -- --test-threads=1`，成员只管理本人参与的应用，跨应用/环境读取或 reset 拒绝，安全回执可查询；新增用例必须实际被执行。

#### T72: 实现企业管理 HTTP

**文件：** `cobotusr/src/route/enterprise_admin.rs`、`cobotusr/src/route/mod.rs`、`tests/cobotusr/routes/identity_account.rs`、`tests/cobotusr/routes/identity_application.rs`、`tests/cobotusr/routes/identity_assignment.rs`

**依赖：** T30、T35、T40、T41、T45、T46、T47、T64、T67、T68

**步骤：**

1. 先写账户/邀请/重置/组、资格、approvals、assignments/test-users、third-party、suspend/resume、legacy adopt 请求测试。
2. 按实际管理操作 guard + CSRF，并在事务内验证 actor、revision、自批与快照。
3. 提供脱敏 audit-events、logout-deliveries/retry，开发者只能读本人应用概况，手动 retry 仍校验批准目标策略。

**验证：** 运行 `cargo test -p cobotrix-root identity_application -- --test-threads=1`，越权、自批、旧 revision 与未确认 LocalOnly 拒绝；成功状态和审计/投递概况一致；新增用例必须实际被执行。

#### T73: 完成协议 Router 和 OpenAPI 契约

**文件：** `cobotusr/src/route/mod.rs`、`cobotusr/src/route/oidc.rs`、`cobotusr/src/swagger_docs.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T60、T62、T63、T69、T70、T71、T72

**步骤：**

1. 把 discovery、authorize/token/userinfo/introspect/revoke/logout 按不同认证方式注册，保留旧 userinfo 发布路径。
2. 先测试每个 discovery 声明的端点真实可调用，补齐现已实现的退出能力；userinfo 不受管理 URI-RBAC 拦截。
3. 注册新 DTO/路由的 OpenAPI、安全方案、错误及一次性秘密说明；确保未实现的动态注册/implicit 等能力未宣告。

**验证：** 运行 `cargo test -p cobotrix-root identity_protocol -- --test-threads=1`，标准客户端可发现并调用真实端点，前后端契约一致且未误宣告能力；新增用例必须实际被执行。

### G：三个前端入口

#### T74: 改造中心前端会话恢复

**文件：** `cobotweb/apps/cobotusr/src/workspace/session.ts`、`cobotweb/apps/cobotusr/src/workspace/coordinator.ts`、`cobotweb/apps/cobotusr/src/pinia/stores/user.ts`、`cobotweb/apps/cobotusr/src/http/axios.ts`、`cobotweb/apps/cobotusr/tests/unit/identity-session.test.ts`

**依赖：** T51、T69、T70

**步骤：**

1. 先写没有 sessionStorage token 但中心 Cookie 有效、新标签并发、401/403 与 CSRF 稳定测试。
2. 应用启动/刷新先请求 account/session，再决定登录状态；新管理 API 同源携带 Cookie 和正确 CSRF。
3. 中心恢复不调用旧 refresh，不使用旧本地身份复活会话；401 清身份，403 保留认证并说明缺少权限。

**验证：** 运行 `pnpm --filter @cobotweb/cobotusr test -- identity-session.test.ts` 和 `pnpm --filter @cobotweb/cobotusr check`，无本地 token 可恢复中心，并发不竞争 refresh/CSRF，401/403 行为可区分。

#### T75: 调整导航权限和页面缓存

**文件：** `cobotweb/apps/cobotusr/src/router/guard.ts`、`cobotweb/apps/cobotusr/src/router/routes.ts`、`cobotweb/apps/cobotusr/src/router/navigation.ts`、`cobotweb/apps/cobotusr/src/router/index.ts`、`cobotweb/apps/cobotusr/src/workspace/page-registry.ts`、`cobotweb/apps/cobotusr/src/workspace/draft-registry.ts`、`cobotweb/apps/cobotusr/src/pinia/stores/tabs.ts`、`cobotweb/apps/cobotusr/tests/unit/identity-navigation.test.ts`、`cobotweb/apps/cobotusr/src/pages/account/index.vue`、`cobotweb/apps/cobotusr/src/pages/developer/index.vue`、`cobotweb/apps/cobotusr/src/pages/enterprise-admin/index.vue`

**依赖：** T71、T72、T74

**步骤：**

1. 先测普通用户/开发者/管理员三入口、直接导航、资格撤回和页面缓存失效。
2. 注册 /account、/developer、/admin 的实际入口页，显示当前身份/获准入口概要；根据 session 资格显示入口，直接访问仍依据服务端 403。各子页面在对应页面任务中注册，不提前引用不存在的文件。
3. 失去成员/资格时关闭或清理对应缓存与 secret 结果，复用现有布局/草稿机制，不继续展示过期数据。

**验证：** 运行 `pnpm --filter @cobotweb/cobotusr test -- identity-navigation.test.ts` 和 `pnpm --filter @cobotweb/cobotusr check`，入口显示准确，越权直接导航被拒绝，撤回权限清除相关页面秘密。

#### T76: 定义三入口 API 和结果类型

**文件：** `cobotweb/apps/cobotusr/src/pages/account/api/index.ts`、`cobotweb/apps/cobotusr/src/pages/account/api/type.ts`、`cobotweb/apps/cobotusr/src/pages/developer/api/index.ts`、`cobotweb/apps/cobotusr/src/pages/developer/api/type.ts`、`cobotweb/apps/cobotusr/src/pages/enterprise-admin/api/index.ts`、`cobotweb/apps/cobotusr/src/pages/enterprise-admin/api/type.ts`、`cobotweb/apps/cobotusr/src/pages/login/api/index.ts`、`cobotweb/apps/cobotusr/src/pages/login/api/type.ts`、`cobotweb/apps/cobotusr/src/pages/oidc-consent/api/index.ts`、`cobotweb/apps/cobotusr/src/pages/oidc-consent/api/type.ts`

**依赖：** T69、T70、T71、T72、T73

**步骤：**

1. 按新 HTTP 契约定义 profile/qualification、应用环境/版本、成员/审批、分配、operation/audit/delivery 的类型。
2. 区分 once-only secret 响应、无 secret 查询回执、active 配置和 pending 草案，统一 revision/conflict 处理。
3. 请求函数显式选择 Cookie API 或标准协议，flow/action ID 不作为资格或任意回调参数。

**验证：** 运行 `pnpm --filter @cobotweb/cobotusr check`，API 类型与 OpenAPI 样本一致，无秘密字段被错误要求为可重读字段。

#### T77: 实现账户资料、应用和会话页面

**文件：** `cobotweb/apps/cobotusr/src/pages/account/views/profile.vue`、`cobotweb/apps/cobotusr/src/pages/account/views/apps.vue`、`cobotweb/apps/cobotusr/src/pages/account/views/consents.vue`、`cobotweb/apps/cobotusr/src/pages/account/views/sessions.vue`、`cobotweb/apps/cobotusr/src/pages/profile/index.vue`、`cobotweb/apps/cobotusr/src/pages/authorized-apps/views/authorized-apps-list.vue`、`cobotweb/apps/cobotusr/tests/unit/identity-account.test.ts`、`cobotweb/apps/cobotusr/src/router/routes.ts`

**依赖：** T74、T75、T76

**步骤：**

1. 先测本人 profile/password、可访问应用、同意撤回和当前/全部退出反馈。
2. 使用账户 API 展示最小身份、真实邮箱、可访问应用和根/RP 会话，明确区分应用会话与中心会话。
3. 原 profile/已授权应用入口复用或转到新账户视图；失败只显示真实错误，不提前标记退出成功。

**验证：** 运行 `pnpm --filter @cobotweb/cobotusr test -- identity-account.test.ts` 和 `pnpm --filter @cobotweb/cobotusr check`，资料、可访问应用和会话范围正确，失败不伪报成功。

#### T78: 实现邀请、重置和重新认证页面

**文件：** `cobotweb/apps/cobotusr/src/pages/login/index.vue`、`cobotweb/apps/cobotusr/src/pages/login/register.vue`、`cobotweb/apps/cobotusr/src/pages/account/views/password-reset.vue`、`cobotweb/apps/cobotusr/src/pages/oidc-login/index.vue`、`cobotweb/apps/cobotusr/tests/unit/identity-actions.test.ts`、`cobotweb/apps/cobotusr/src/router/routes.ts`

**依赖：** T70、T74、T76

**步骤：**

1. 先测原 flow 从开户/已有账户邀请/重置后恢复、撞名提示、链接过期和重复提交。
2. 页面只使用 server action/flow 概要，开户无有效邀请禁止提交；已有账户要求认证后接受。
3. 重置后引导新密码认证，prompt=login 明确重新认证；不要在日志、页面草稿或 URL 留秘密。

**验证：** 运行 `pnpm --filter @cobotweb/cobotusr test -- identity-actions.test.ts` 和 `pnpm --filter @cobotweb/cobotusr check`，受控开户/重置可继续既定流程，无法覆盖 callback/state 且无秘密持久缓存。

#### T79: 实现同意和无交互结果页面

**文件：** `cobotweb/apps/cobotusr/src/pages/oidc-consent/index.vue`、`cobotweb/apps/cobotusr/src/pages/oidc-login/index.vue`、`cobotweb/apps/cobotusr/src/pages/redirect/index.vue`、`cobotweb/apps/cobotusr/tests/unit/identity-consent.test.ts`、`cobotweb/apps/cobotusr/src/router/routes.ts`

**依赖：** T70、T74、T76

**步骤：**

1. 先测真实应用/当前账户/批准 scopes、拒绝同意、flow 过期与缺分配。
2. 依据 AuthorizationOutcome 呈现必要交互，显示实际披露范围和企业预授权政策。
3. 拒绝返回原 state 的 access_denied；无交互协议请求不被前端重定向成登录/同意页。

**验证：** 运行 `pnpm --filter @cobotweb/cobotusr test -- identity-consent.test.ts` 和 `pnpm --filter @cobotweb/cobotusr check`，页面只呈现允许范围，拒绝正确回调，none 不显示交互页面。

#### T80: 实现开发者应用注册和环境页面

**文件：** `cobotweb/apps/cobotusr/src/pages/developer/views/apps.vue`、`cobotweb/apps/cobotusr/src/pages/developer/views/app-create.vue`、`cobotweb/apps/cobotusr/src/pages/developer/views/environment.vue`、`cobotweb/apps/cobotusr/src/pages/applications/views/application-list.vue`、`cobotweb/apps/cobotusr/src/pages/applications/components/application-editor.vue`、`cobotweb/apps/cobotusr/tests/unit/identity-developer.test.ts`、`cobotweb/apps/cobotusr/src/router/routes.ts`

**依赖：** T71、T75、T76

**步骤：**

1. 先测 Web/SPA 注册、一次性结果、dev/prod 切换、URI 错误和回包丢失后查询。
2. 展示应用和两环境的独立配置/凭据，生产未启用状态明确；一次注册后可直接使用有效开发配置。
3. 原应用页面走受控注册与版本 API；重复点击复用幂等键，相同键冲突按已提交回执处理。

**验证：** 运行 `pnpm --filter @cobotweb/cobotusr test -- identity-developer.test.ts` 和 `pnpm --filter @cobotweb/cobotusr check`，一次注册不缺接入关系，环境不会混用，生产草案不显示为可运行配置。

#### T81: 实现成员、秘密和接入指导页面

**文件：** `cobotweb/apps/cobotusr/src/pages/developer/views/members.vue`、`cobotweb/apps/cobotusr/src/pages/developer/views/integration.vue`、`cobotweb/apps/cobotusr/src/pages/developer/components/secret-result.vue`、`cobotweb/apps/cobotusr/src/pages/applications/components/secret-result.vue`、`cobotweb/apps/cobotusr/tests/unit/identity-developer.test.ts`、`cobotweb/apps/cobotusr/src/router/routes.ts`

**依赖：** T71、T75、T76、T80

**步骤：**

1. 先测候选查询、负责人交接、无成员权限、Web secret reset 和 SPA reader 说明。
2. 展示成员/候选的最小信息；秘密只在首次受控结果短暂显示，离开/失权即清理。
3. 生成接入参数仅用 active 配置，区分 public SPA 无 secret 与 backend reader 服务端秘密，注明实际退出能力。

**验证：** 运行 `pnpm --filter @cobotweb/cobotusr test -- identity-developer.test.ts` 和 `pnpm --filter @cobotweb/cobotusr check`，候选不暴露全目录，失权清秘密，接入参数与后端 active 配置一致。

#### T82: 实现生产申请和版本差异页面

**文件：** `cobotweb/apps/cobotusr/src/pages/developer/views/approvals.vue`、`cobotweb/apps/cobotusr/src/pages/developer/views/environment.vue`、`cobotweb/apps/cobotusr/src/pages/developer/components/config-diff.vue`、`cobotweb/apps/cobotusr/tests/unit/identity-approvals.test.ts`、`cobotweb/apps/cobotusr/src/router/routes.ts`

**依赖：** T71、T76、T80

**步骤：**

1. 先测 pending/rejected/obsolete/approved/suspended、失败切换和客户端类型变更。
2. 并列展示 active 与申请版本，提交/review 后刷新服务端真实状态，不提前把草案当已上线。
3. 类型切换显示新 client_id 和重新接入，旧申请失效展示原因，允许受控重新提交。

**验证：** 运行 `pnpm --filter @cobotweb/cobotusr test -- identity-approvals.test.ts` 和 `pnpm --filter @cobotweb/cobotusr check`，待审和已生效配置清晰，切换失败仍显示旧 active，类型变化参数准确。

#### T83: 实现企业账户、邀请和用户组页面

**文件：** `cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/accounts.vue`、`cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/invitations.vue`、`cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/groups.vue`、`cobotweb/apps/cobotusr/src/pages/users/views/user-create.vue`、`cobotweb/apps/cobotusr/src/pages/users/views/user-list.vue`、`cobotweb/apps/cobotusr/tests/unit/identity-admin.test.ts`、`cobotweb/apps/cobotusr/src/router/routes.ts`

**依赖：** T72、T75、T76

**步骤：**

1. 先测员工/合作方邀请、资格、密码 reset、最后管理员/owner 约束和组 revision 冲突。
2. 管理员只展示一次邀请/reset 链接，账户停用提示影响范围与需要交接的应用。
3. 旧账户创建/目录页面遵循新准入和隐私；组操作失败不能显示部分成功。

**验证：** 运行 `pnpm --filter @cobotweb/cobotusr test -- identity-admin.test.ts` 和 `pnpm --filter @cobotweb/cobotusr check`，受控账户管理反馈正确，约束/冲突可处理且无链接泄漏到草稿。

#### T84: 实现企业审批、分配和第三方页面

**文件：** `cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/approvals.vue`、`cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/applications.vue`、`cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/assignments.vue`、`cobotweb/apps/cobotusr/tests/unit/identity-admin.test.ts`、`cobotweb/apps/cobotusr/src/router/routes.ts`

**依赖：** T72、T76、T82、T83

**步骤：**

1. 先测自批禁止、snapshot 过时、用户/组分配、开发测试用户与停用/恢复。
2. 展示审批固定配置与范围；LocalOnly 第三方启用要求明确确认退出限制，记录操作结果。
3. 状态由服务器决定，访问分配和维护权分别显示；审批失败/拒绝不显示生产生效。

**验证：** 运行 `pnpm --filter @cobotweb/cobotusr test -- identity-admin.test.ts` 和 `pnpm --filter @cobotweb/cobotusr check`，过时申请不能批准，分配与维护权独立，退出限制确认必需。

#### T85: 实现历史纳管、审计和投递页面

**文件：** `cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/legacy-apps.vue`、`cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/audit-events.vue`、`cobotweb/apps/cobotusr/src/pages/enterprise-admin/views/logout-deliveries.vue`、`cobotweb/apps/cobotusr/src/pages/recordings/views/recordings-list.vue`、`cobotweb/apps/cobotusr/src/pages/oauth/views/oauth-create.vue`、`cobotweb/apps/cobotusr/src/pages/oauth/views/oauth-detail.vue`、`cobotweb/apps/cobotusr/src/pages/oauth/views/oauth-list.vue`、`cobotweb/apps/cobotusr/tests/unit/identity-admin.test.ts`、`cobotweb/apps/cobotusr/src/router/routes.ts`

**依赖：** T67、T72、T76、T84

**步骤：**

1. 先测冲突记录、原 client 选择、纳管失败、通知重试和普通开发者查询限制。
2. 历史页面明确 legacy 状态和需核对的 owner/环境/能力/范围，旧 OAuth 页面不能直接修改 managed 安全配置。
3. 审计/投递展示操作者、提交状态与脱敏错误，2xx 仅显示通知已接受，LocalOnly 及失败不称全部退出。

**验证：** 运行 `pnpm --filter @cobotweb/cobotusr test -- identity-admin.test.ts` 和 `pnpm --filter @cobotweb/cobotusr check`，旧配置无法绕过治理，通知状态表述准确，审计无秘密且可关联请求。

### H：两个独立标准示例

#### T86: 建立独立示例工程与依赖锁定

**文件：** `examples/oidc-web/package.json`、`examples/oidc-web/tsconfig.json`、`examples/oidc-web/.env.example`、`examples/oidc-spa/package.json`、`examples/oidc-spa/tsconfig.json`、`examples/oidc-spa/vite.config.ts`、`examples/oidc-spa/index.html`、`examples/oidc-spa/.env.example`、`pnpm-workspace.yaml`、`pnpm-lock.yaml`、`package.json`、`examples/oidc-web/src/config.ts`、`examples/oidc-web/tests/config.test.ts`、`examples/oidc-spa/src/config.ts`、`examples/oidc-spa/server/config.ts`、`examples/oidc-spa/tests/config.test.ts`

**依赖：** T73

**步骤：**

1. 建立 @cobotrix/oidc-web 与 @cobotrix/oidc-spa，后者包含独立运行的 Node 保护 API；为两个包定义 check/build/test 脚本。
2. 采用 openid-client、oidc-client-ts 与标准 JWT 校验库；实现前核对官方 Node/API 兼容性后锁定实际版本，不引入中心 DB/内部 auth 依赖。
3. 配置样例列 issuer/client/callback/API 变量及用途，生产 secret 从运行环境注入；SPA public 配置禁止 client_secret，reader 仅 API 进程读取。
4. 实现并测试示例运行配置校验：Web secret 与 API reader 仅服务端读取，SPA 仅暴露批准的 public 参数；脚手架 check/test 对实际配置模块执行。

**验证：** 运行 `pnpm install --frozen-lockfile` 和两个包的 check，依赖锁一致、工程可编译，浏览器可见配置不含秘密。

#### T87: 实现 Web 示例标准登录

**文件：** `examples/oidc-web/src/server.ts`、`examples/oidc-web/src/oidc.ts`、`examples/oidc-web/src/session.ts`、`examples/oidc-web/tests/oidc.test.ts`

**依赖：** T56、T57、T58、T73、T86

**步骤：**

1. 先测 discovery/state/nonce/PKCE、错误回调和相同本地会话刷新互斥。
2. openid-client 完成 code 兑换和 ID Token 校验，以 (iss,sub) 建立最小本地业务资料和独立 opaque 会话 Cookie。
3. 服务端保管 token/refresh，用户界面不暴露中心 token；每次新认证的本地 state 与请求对应。

**验证：** 运行 `pnpm --filter @cobotrix/oidc-web check`、`pnpm --filter @cobotrix/oidc-web build`、`pnpm --filter @cobotrix/oidc-web test`，标准回调校验与刷新互斥测试通过；真实 OP 兼容性在 T96 验证。

#### T88: 实现 Web 示例退出和保护操作

**文件：** `examples/oidc-web/src/server.ts`、`examples/oidc-web/src/protection.ts`、`examples/oidc-web/src/logout.ts`、`examples/oidc-web/tests/logout.test.ts`

**依赖：** T59、T62、T63、T87

**步骤：**

1. 先测本地退出、RP logout state、backchannel 签名/aud/events/nonce/jti/sid 和重复通知。
2. 本地退出只清 A 会话，可标准 revoke A 授权；中心退出通过标准 end_session_endpoint 确认。
3. 接收端用标准 JWT 库/JWKS 验证通知并按 sid 幂等清会话/缓存；提供必须在线内省允许的 POST /api/operation，验证本地 CSRF/来源。

**验证：** 运行 `pnpm --filter @cobotrix/oidc-web check`、`pnpm --filter @cobotrix/oidc-web build`、`pnpm --filter @cobotrix/oidc-web test`，伪造通知拒绝、重复通知幂等、本地退出不删除其他 RP；无有效会话保护写入失败。

#### T89: 实现 SPA 示例标准登录

**文件：** `examples/oidc-spa/src/main.ts`、`examples/oidc-spa/src/oidc.ts`、`examples/oidc-spa/src/api.ts`、`examples/oidc-spa/tests/oidc.test.ts`

**依赖：** T56、T57、T58、T60、T73、T86

**步骤：**

1. 先测试 public code+S256、state/nonce、标签页存储独立和本标签 refresh single-flight。
2. oidc-client-ts 通过顶层导航完成授权，禁用依赖 iframe 静默登录的恢复；浏览器无 client_secret。
3. 按 (iss,sub) 呈现自己的最小本地资料；access 只送配套 API，刷新秘密不跨标签页共享，本地/中心退出分开。

**验证：** 运行 `pnpm --filter @cobotrix/oidc-spa check`、`pnpm --filter @cobotrix/oidc-spa build`、`pnpm --filter @cobotrix/oidc-spa test`，public 参数与标签页隔离正确，构建产物无客户端/reader secret。

#### T90: 实现 SPA 配套保护 API

**文件：** `examples/oidc-spa/server/server.ts`、`examples/oidc-spa/server/protection.ts`、`examples/oidc-spa/server/logout.ts`、`examples/oidc-spa/tests/api.test.ts`

**依赖：** T44、T59、T63、T89

**步骤：**

1. 先测 bearer 签名/issuer/audience、过期/未知 token、reader 认证、允许 origin 和 backchannel 通知。
2. 独立 API 仅持后端 reader secret 做内省，提供 POST /api/operation 及 sid 关联/cache 清理；不代 SPA 兑换授权码。
3. 校验 JWT 与当前 registry/准入，拒绝中心管理或 A 的 token；通知签名/events/jti/目标按标准验证。

**验证：** 运行 `pnpm --filter @cobotrix/oidc-spa check`、`pnpm --filter @cobotrix/oidc-spa build`、`pnpm --filter @cobotrix/oidc-spa test`，跨应用/未知 token 拒绝，reader 不进入浏览器 bundle，合法保护操作须有内省允许。

#### T91: 落实两个保护 API 的缓存时限

**文件：** `examples/oidc-web/src/protection.ts`、`examples/oidc-web/tests/protection.test.ts`、`examples/oidc-spa/server/protection.ts`、`examples/oidc-spa/tests/protection.test.ts`

**依赖：** T88、T90

**步骤：**

1. 先以可控单调时钟测试慢内省、跨层读取、expiry、过期缓存/依赖中断、保护写入前过期。
2. 正向缓存默认 2 秒，从内省发起时计龄，总年龄硬上限 5 秒并受 token 到期限制；层间传递原验证时刻而不重算 TTL。
3. 过龄结果不缓存，执行写入前再次检查；过期/clock/内省故障失败关闭，通知可清除对应 sid 缓存。

**验证：** 运行两个示例的 protection 测试，慢查询与多次读取不能延长允许结果，过期后依赖故障/保护写入拒绝。

### I：集成、迁移和真实浏览器

#### T92: 补齐 HTTP 权限与最小披露矩阵

**文件：** `tests/cobotusr/routes/identity_account.rs`、`tests/cobotusr/routes/identity_application.rs`、`tests/cobotusr/routes/identity_assignment.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T69、T70、T71、T72、T73

**步骤：**

1. 按已实现公开操作补参数缺失/错误格式、未认证、无资格、非成员、资源不存在、revision 冲突的黑盒请求。
2. 交叉测试开发者资格、成员、分配、同意、业务权限彼此不能替代；userinfo/introspection 不泄漏未获准资料。
3. 检查秘密一次性交付、no-store/Origin/CSRF/CORS 和日志/audit 脱敏；失败后比较数据库无状态变化。

**验证：** 运行 `cargo test -p cobotrix-root identity_ -- --test-threads=1`，HTTP 正常及拒绝矩阵通过，资料披露最小且失败无越权写入；新增用例必须实际被执行。

#### T93: 补齐并发、提交失败与重放竞态

**文件：** `tests/cobotusr/routes/identity_account.rs`、`tests/cobotusr/routes/identity_application.rs`、`tests/cobotusr/routes/identity_assignment.rs`、`tests/cobotusr/routes/identity_session.rs`、`tests/cobotusr/routes/identity_protocol.rs`

**依赖：** T46、T47、T55、T56、T57、T64、T92

**步骤：**

1. 用 barrier 控制邀请/reset/code 同时消费、两个 app flow、审批与 owner/配置/分配交错、撤销与签发/refresh 交错。
2. 分别注入账户/注册/同意/code 签名/commit/secret reset/分配更新失败与提交后回包丢失。
3. 断言唯一生效和原预算重试、active/binding/epoch/consent/audit/outbox 同提交；真正 refresh replay 保持撤销，正常 root 恢复不误报。

**验证：** 运行 `cargo test -p cobotrix-root identity_ -- --test-threads=1`，竞态按准入/锁序得到唯一合法结果，失败无半提交，回包丢失不重发秘密/令牌；新增用例必须实际被执行。

#### T94: 完成全新与历史数据库升级回归

**文件：** `tests/fixtures/enterprise-identity-legacy.sql`、`tests/cobotusr/routes/identity_migration.rs`、`tests/cobotusr/routes/auth_center.rs`、`tests/cobotusr/routes/auth_revocation.rs`、`tests/cobotusr/routes/security_revocation.rs`、`tests/cobotusr/routes/security_mcp.rs`、`tests/cobotusr/routes/oidc.rs`

**依赖：** T25、T51、T66、T67、T92

**步骤：**

1. 建立旧版本真实字段形状 fixture，保存用户/sub、应用 key、client ID/secret hash、签名 key 和历史记录样本。
2. 测试全新库、重复启动、冲突解除/纳管、未解冲突、bootstrap 旁路和失败恢复，比较升级前后样本。
3. 回归 MCP/通用权限边界、管理刷新重放与既有 OIDC；纳管旧代码/授权/consent 失效但不影响其他 legacy client。

**验证：** 运行 identity_migration、auth_center、oidc、security 相关集成筛选测试，身份/标识/密钥保留、受控准入及已有权限边界均通过。

#### T95: 建立真实跨站 TLS 浏览器 fixture

**文件：** `tests/cobotusr/routes/identity_browser.rs`、`tests/cobotusr/common.rs`、`cobotweb/apps/cobotusr/tests/e2e/enterprise-sso-runner.mjs`、`cobotweb/apps/cobotusr/tests/e2e/enterprise-sso-fixture.ts`、`cobotweb/apps/cobotusr/playwright.enterprise-sso.config.ts`

**依赖：** T1、T65、T73、T85、T87、T89、T90、T91

**步骤：**

1. 启动隔离 PostgreSQL、真实 OP/三入口 UI、Web A、SPA B/API，随机端口并通过 fixture TLS 代理提供不同站点。
2. fixture 使用 127.0.0.1/localhost/127.0.0.2 等地址，断言 site/Cookie 隔离；浏览器上下文只处理 fixture 证书，Node/API 只信 fixture CA。
3. Chromium/Firefox/WebKit 各自独立 context，经真实登录及管理 API 邀请/注册/审批/分配；不拦截 authorize/token 伪造成功，不使用真实业务库。

**验证：** 运行 `cargo test -p cobotrix-root identity_browser -- --test-threads=1`，三引擎能访问不同 TLS 站点，A/B 无共享 Cookie/storage，fixture 启停可靠且标准请求到真实 OP；新增用例必须实际被执行。

#### T96: 验证跨应用登录和流程恢复

**文件：** `cobotweb/apps/cobotusr/tests/e2e/enterprise-sso-login.spec.ts`、`tests/cobotusr/routes/identity_browser.rs`

**依赖：** T78、T79、T81、T82、T95

**步骤：**

1. 在三引擎从 A 发起授权，邀请开户/已有账户接受后完成真实 code/PKCE 回调，再在新标签进入 B。
2. 断言 B 不要求密码、各自 client/aud/sid/token 独立、(iss,sub) 相同；刷新/清中心当前标签 token 保留 Cookie 后能恢复。
3. 验证 A/B 并发首次流程、重新打开标签、prompt=login/consent/none/max_age 与 profile 修改后主体稳定。

**验证：** 运行 `cargo test -p cobotrix-root identity_browser -- --test-threads=1`，三引擎真实 A→B SSO、流程恢复和交互策略通过，证据包含回调状态及独立身份映射；新增用例必须实际被执行。

#### T97: 验证治理拒绝和接口绕过

**文件：** `cobotweb/apps/cobotusr/tests/e2e/enterprise-sso-admission.spec.ts`、`tests/cobotusr/routes/identity_browser.rs`

**依赖：** T83、T84、T85、T92、T95

**步骤：**

1. 在三引擎测试未批准生产、错误回调、无分配、无测试访问、无开发者资格、跨应用成员/secret 查询和自批。
2. 测试合作方旧目录/角色读取限制、旧 register/OAuth 管理不能绕过新规则、pending 与 active 差异。
3. 通过真实 ID Token/userinfo 验证未批准 scope/旧 consent 不披露，管理 token 或 A token 不能替 B 获取身份/保护操作。

**验证：** 运行 `cargo test -p cobotrix-root identity_browser -- --test-threads=1`，拒绝场景无凭据/状态变化或资料泄漏，UI 与直接 API 结果一致；新增用例必须实际被执行。

#### T98: 验证退出联动及超过 5 秒的拒绝

**文件：** `cobotweb/apps/cobotusr/tests/e2e/enterprise-sso-logout.spec.ts`、`tests/cobotusr/routes/identity_browser.rs`、`tests/cobotusr/routes/identity_session.rs`

**依赖：** T61、T62、T64、T91、T93、T95

**步骤：**

1. 真实浏览器依次验证 A 本地退出、当前根退出、所有根退出、移除组成员、撤回同意、密码修改/重置、停用/恢复与配置切换。
2. 记录中心提交结束时刻；分别在 A 和 B/API 发起保护操作，超过 5 秒必须拒绝，同时检查另应用/用户保留访问。
3. 注入后通道不可达、重试/重复通知、内省/主库/clock 故障；缓存过期后保护写入失败，页面/审计只反映真实能力和投递结果。

**验证：** 运行 `cargo test -p cobotrix-root identity_browser -- --test-threads=1`，三引擎退出范围正确，两个真实保护 API 的超过 5 秒拒绝有单调时间证据，通知故障不破坏中心失效；新增用例必须实际被执行。

### J：交付说明与完整核验

#### T99: 编写标准接入说明和示例运行指南

**文件：** `docs/integration/enterprise-oidc.md`、`examples/oidc-web/README.md`、`examples/oidc-spa/README.md`

**依赖：** T73、T81、T87、T88、T89、T90、T91、T96、T98

**步骤：**

1. 说明 Provider 定位、受控开发者接入、dev/prod、审批/分配/同意、准确 discovery/回调/CORS 与身份映射。
2. 提供 Web/SPA+API 从注册配置到启动、code+S256、userinfo/refresh、后端 reader、RP/backchannel logout 和本地退出步骤。
3. 说明根到期/配置变化/重新认证、在线检查默认 2 秒与硬上限 5 秒、LocalOnly 限制；样例不含真实 secret，使用本轮已运行参数。

**验证：** 依照文档在新的隔离 fixture 启动两个示例并完成 T96/T98 场景，参数与服务端契约一致；扫描文档无真实凭据。

#### T100: 编写升级、纳管与故障恢复指南

**文件：** `docs/operations/enterprise-identity-upgrade.md`

**依赖：** T25、T64、T67、T94、T98

**步骤：**

1. 写明主库/配置/key 备份、预演冲突扫描、维护窗口停旧进程、迁移/启动、样本主体和 JWKS 核对步骤。
2. 区分开放新写入前备份恢复和新治理写入后的停服数据恢复，明确不能直接运行忽略 managed 规则的旧二进制。
3. 记录 invitation/reset 交付、owner 交接、审批 obsolete、outbox permanent_failed/retry、issuer/key/数据库故障诊断及证据保留。

**验证：** 在隔离旧库 fixture 演练升级、冲突纳管及备份恢复，样本与 T94 相符，文档未声称新写入可无损退回旧规则。

#### T101: 接入专用验证命令和 CI

**文件：** `package.json`、`pnpm-workspace.yaml`、`pnpm-lock.yaml`、`cobotweb/apps/cobotusr/package.json`、`.github/workflows/rust.yml`

**依赖：** T94、T95、T96、T97、T98、T99、T100

**步骤：**

1. 根脚本增加 test:sso 调用 `cargo test -p cobotrix-root --all-features identity_browser -- --test-threads=1`，Rust fixture 调专用三引擎 runner/config。
2. 把示例 check/build/test 与 frozen lock 检查纳入流程，保留现有 macOS/Windows/Ubuntu 构建，Ubuntu 跑隔离 Pg 和真实 SSO。
3. 保存脱敏测试结果及迁移/时间证据；依赖、Docker 或浏览器缺失返回失败/明确未验证，禁止静默 skip 成功。

**验证：** 运行 `pnpm test:sso`，实际启动三引擎并执行登录/准入/退出场景；核对 CI 保留平台矩阵且无未执行测试的绿色结果。

#### T102: 执行完整构建与质量检查

**文件：** `Cargo.toml`、`Cargo.lock`、`cobotusr/Cargo.toml`、`libs/auth/Cargo.toml`、`package.json`、`pnpm-lock.yaml`、`.github/workflows/rust.yml`

**依赖：** T92、T93、T94、T101

**步骤：**

1. 执行任务文档中的完整命令表；T101 已通过且输入未变更的专用 SSO 结果复用，不因汇总再重复执行。先修当前修改引入的失败，再复跑受影响检查；必要依赖改动同步 lockfile。
2. 覆盖 Rust workspace/all-targets/all-features、doctest、前端 test/check/lint/build、两个示例及真实三引擎 SSO。
3. 执行依赖未使用检查与 Rust 依赖审计，记录实际发现和处理结果；工具/环境缺失不得填通过。

**验证：** 所有完整命令实际通过，无编译/lint/测试失败；检查结果记录执行范围、环境和失败/未验证项，不能以历史结果代替。

#### T103: 整理需求覆盖和本轮验收证据

**文件：** `docs/specs/enterprise-sso-provider/verification.md`、`docs/specs/enterprise-sso-provider/task.md`

**依赖：** T96、T97、T98、T99、T100、T102

**步骤：**

1. 把 checklist.md 的每个验收项对齐实际命令/场景、结果与脱敏证据；按本 task 依赖标记已完成项，仅通过验证才勾选。
2. 记录 commit/工作区、依赖/OS/浏览器版本、临时数据库/站点、真实运行数量、迁移样本、并发结果和 5 秒时间边界。
3. 报告失败/跳过/限制；截图/网络摘要/audit/outbox 在产生时脱敏，不保留完整密码/邀请/reset/code/token/Cookie/secret 或原始未脱敏 trace。

**验证：** 逐条核对 F1—F12、N1—N7、AC1—AC27 和批准的 checklist，所有完成声明都有本轮证据，报告无秘密和未验证却标通过项。

## 5. 方案与需求覆盖

### 5.1 组件覆盖

| 方案组件 | 实现任务 |
| --- | --- |
| plan 1、3：模块依赖、类型、服务和仓储契约 | T2—T6、T13—T18、T65 |
| plan 2.2：账户、资格、组与访问代次 | T8、T11—T12、T15、T17、T27—T35、T45—T46 |
| plan 2.3：环境、版本、审批、reader、operation | T9、T16、T36—T44 |
| plan 2.4：SSO/flow/RP/code/consent、安全关联 | T10、T18、T21—T24、T48—T59 |
| plan 2.5：审计、outbox、迁移冲突 | T7、T10、T19—T20、T64、T67、T85 |
| plan 4：Cookie/CSRF、路由分组、标准错误 | T24—T25、T49—T60、T62、T68—T76 |
| plan 5：受控开户、原子注册、批准、准入、身份输出 | T27—T59、T69—T73 |
| plan 6：退出范围、RP/backchannel、5 秒保护 | T22、T30、T32、T45—T47、T61—T64、T88—T91、T98 |
| plan 7：共享事务、锁、预算、重试、清理 | T13—T14、T21—T22、T26、T46、T93 |
| plan 8：三入口和两个独立示例 | T74—T91、T95—T99 |
| plan 9：新/旧库升级、bootstrap、显式纳管、恢复 | T8—T12、T25、T66—T67、T85、T94、T100 |
| plan 10：规则/HTTP/并发/迁移/真实浏览器/CI 证据 | T1—T5、各任务验证、T92—T103 |
| plan 11—12：技术边界及标准客户端兼容 | T25、T41、T60、T73、T86—T91、T99—T103 |

### 5.2 Spec 覆盖

下表涵盖 F1—F12、N1—N7 和 AC1—AC27。各实现任务中的验证与 T92—T98 的集成/浏览器验证共同提供证据，最终逐项记录到验收报告。

| 需求 | 任务 | 对应验收 |
| --- | --- | --- |
| F1 | T8、T12、T15、T27、T28、T29、T30、T31、T32、T69、T70、T78、T83、T94、T96、T98 | AC1、AC2、AC3、AC4 |
| F2 | T2、T15、T30、T33、T34、T66、T71、T75、T81、T92、T97 | AC5、AC6 |
| F3 | T3、T9、T16、T36、T37、T69、T71、T80、T86、T96 | AC7、AC8 |
| F4 | T3、T16、T38、T39、T40、T41、T42、T47、T71、T72、T82、T84、T93、T97 | AC9、AC10 |
| F5 | T4、T17、T22、T35、T45、T46、T72、T84、T97、T98 | AC11、AC12 |
| F6 | T5、T18、T24、T49、T50、T51、T52、T53、T54、T55、T56、T57、T74、T78、T79、T93、T96 | AC2、AC13、AC14、AC15 |
| F7 | T4、T23、T31、T48、T54、T55、T56、T58、T59、T79、T92、T97 | AC4、AC11、AC16、AC17 |
| F8 | T3、T9、T16、T37、T42、T43、T44、T71、T76、T81、T92、T97 | AC5、AC8、AC10、AC18 |
| F9 | T20、T21、T22、T26、T30、T32、T46、T47、T61、T62、T63、T64、T65、T77、T85、T88、T90、T91、T98 | AC3、AC12、AC19、AC20、AC21、AC22 |
| F10 | T71、T81、T86、T87、T88、T89、T90、T91、T95、T96、T98、T99 | AC23 |
| F11 | T7、T19、T41、T68、T69、T70、T71、T72、T73、T75、T77、T80、T83、T84、T85、T92、T97 | AC22、AC24 |
| F12 | T8、T9、T10、T12、T25、T51、T66、T67、T85、T94、T100 | AC25、AC26 |
| N1 | T2、T3、T4、T13、T14、T17、T21、T22、T30、T33、T40、T45、T48、T57、T58、T59、T68、T92、T93 | AC27 及权限/会话各场景 |
| N2 | T7、T23、T24、T25、T27、T49、T50、T52、T53、T56、T60、T62、T63、T64、T86、T95、T97 | AC27 及传输/凭据各场景 |
| N3 | T14、T15、T16、T18、T19、T20、T37、T40、T43、T46、T55、T56、T57、T93、T94 | AC14、AC26、AC27 |
| N4 | T23、T53、T54、T56、T57、T58、T59、T60、T62、T63、T73、T87、T88、T89、T90、T96 | AC15、AC17、AC20、AC27 |
| N5 | T21、T22、T46、T47、T59、T64、T88、T90、T91、T98 | AC21、AC27 |
| N6 | T1、T92、T93、T94、T95、T96、T97、T98、T101、T102、T103 | AC27 |
| N7 | T25、T65、T67、T94、T99、T100、T101、T102、T103 | AC27 |

## 6. 完整核验命令与证据

下列命令在实现阶段执行。各项局部验证通过后，T102 汇总完整检查；已经通过且输入未变更的结果可直接引用。全 workspace 测试若已执行 identity_browser 的完整三引擎场景，不额外为汇总重复相同 SSO 用例。

| 范围 | 命令 | 通过证据 |
| --- | --- | --- |
| 依赖可重现 | `pnpm install --frozen-lockfile` | 安装使用现有 lockfile，不发生隐式版本改写 |
| 中心前端 | `pnpm --filter @cobotweb/cobotusr test` | 单元/组件测试通过，新增入口、恢复、flow 与权限用例实际执行 |
| 中心类型与静态检查 | `pnpm --filter @cobotweb/cobotusr check`；`pnpm --filter @cobotweb/cobotusr lint` | 类型及 lint 无错误 |
| 中心前端构建 | `pnpm build:cobotusr` | 嵌入 UI 所需构建产物生成成功 |
| Web 示例 | `pnpm --filter @cobotrix/oidc-web check`；`pnpm --filter @cobotrix/oidc-web build`；`pnpm --filter @cobotrix/oidc-web test` | 标准客户端、会话、通知、保护缓存测试和构建通过 |
| SPA/API 示例 | `pnpm --filter @cobotrix/oidc-spa check`；`pnpm --filter @cobotrix/oidc-spa build`；`pnpm --filter @cobotrix/oidc-spa test` | public 客户端、标签隔离和独立 API 测试/构建通过，bundle 不含秘密 |
| Rust 格式 | `cargo fmt --all -- --check` | 格式检查通过 |
| Rust 静态检查 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 所有目标/feature 无警告 |
| Rust 发布构建 | `cargo build --workspace --all-targets --all-features --release` | 保留当前平台构建能力 |
| Rust 完整测试 | `cargo test --workspace --all-targets --all-features -- --test-threads=1` | 规则、隔离 Pg/HTTP、并发、迁移和注册的浏览器测试通过 |
| 文档测试 | `cargo test --workspace --doc --all-features` | 所有 doctest 通过 |
| 依赖使用与审计 | `cargo machete`；`cargo audit` | 未使用依赖与依赖漏洞检查结果及实际处理可追溯，工具缺失不记通过 |
| 专用真实 SSO | `pnpm test:sso` | T101 新增脚本启动真实 OP/Web/SPA/API，Chromium/Firefox/WebKit 执行 T96—T98 |

验收报告记录实际源码状态、命令、执行数量、环境/依赖版本、数据库隔离和不同站点配置。5 秒证据以中心提交结束时刻及参考保护 API 的单调计时为准；迁移证据逐项比较主体/client/key/历史样本。任何未运行场景注明未验证及原因。

日志、截图、网络摘要和通知/audit 证据在生成时脱敏；不保存原始含密 trace，不以页面消失替代服务端保护操作拒绝，不以通知 2xx 声称第三方全部会话已经退出。

## 7. 本轮文档核验

2026-11-09 已执行文档核验：103 个任务编号唯一，395 条依赖均存在、无环且顺序合法，232 个文件全部列入清单；每项文件/步骤/验证字段完整，覆盖 F1—F12、N1—N7 与 AC1—AC27。占位、空章节及空白检查通过。该核验只说明任务文档自洽；产品功能完成情况由后续真实实现与 checklist 验收决定。
