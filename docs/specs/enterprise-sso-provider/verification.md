# 企业 SSO 实现核验记录

日期：2026-11-09，Asia/Shanghai。工作区包含未提交实现，记录针对命令执行时的代码，不代表后续改动自动通过。

## 已取得的证据

| 命令 | 实际结果 | 覆盖及限制 |
| --- | --- | --- |
| `cargo test -p cobotrix-root security_ -- --test-threads=1` | 81 通过，0 失败 | 旧安全仓储、签发/撤销竞态、预算和提交故障。执行于本轮配置修改及最新 managed 签发 helper 之前，后续需回归。 |
| `cargo test -p cobotrix-root identity_session_uncertain_commit -- --test-threads=1` | 1 通过，0 失败 | COMMIT 中耗尽原预算，操作只执行一次，不返回秘密；主库已提交的安全回执可查询。 |
| `cargo test -p cobotrix-root identity_ -- --test-threads=1` | 32 通过，0 失败 | 最新账户 action/profile、迁移、应用版本、准入、多来源代次、会话、真实 RSA/JWKS/ID Token 和提交故障集成测试。执行于本轮配置修改之前。 |
| `cargo test -p auth` | 49 单元测试、8 doctest 通过；1 个既有 doctest ignored | 包含真实 RSA/JWKS ID Token 与 Logout Token 签名；不代表 HTTP OIDC 闭环。 |
| `cargo test -p cobotusr --lib` | 63 通过，0 失败 | 配置规则与领域单元测试，执行于本轮 SSO/HTTP 编排落盘之前。 |
| `cargo test -p cobotrix-root identity_migration_production_key -- --test-threads=1` | 1 通过，0 失败 | 生产缺失/损坏密钥拒绝启动且不覆盖，显式 loopback 开发生成权限 0600，生产重载 JWKS 不变。 |
| `cargo test -p cobotrix-root identity_session_ -- --test-threads=1` | 11 通过，0 失败 | 并发只读恢复、根到期/auth_epoch 拒绝、个人撤销范围和其他浏览器/用户隔离；执行于后续密码/HTTP 编辑之前。 |
| `cargo check -p cobotusr --all-targets` | 通过，无编译警告 | 包含本轮中心 SSO 服务和 HTTP；既有 proc-macro-error2 future-incompat notice 保留。 |
| `cargo test -p cobotrix-root identity_browser_ -- --test-threads=1` | 2 通过，0 失败 | HTTP 密码登录、Cookie、并发恢复不覆写、不续期、Origin 拒绝、重登录撤销旧 RP 和退出清 Cookie；flow 用例手工 seed，不是完整 authorize/token 闭环。 |
| `cargo test -p cobotrix-root identity_session_center_bound_manager_can_manage_independent_mcp_and_revoke_scopes -- --test-threads=1` | 1 通过，0 失败 | 中心绑定管理凭据创建、改名、轮换、删除独立 MCP，撤销 RP；中心退出使 manager 失效但保留独立 MCP。 |
| `cargo test -p cobotusr --lib identity_browser_ -- --test-threads=1` | 2 通过，0 失败 | 生产 Cookie/Origin/CSRF 上下文与读取复用；执行于 action Cookie 和 write_challenge 编辑之前，需回归。 |
| `cargo test -p cobotrix-root identity_account_service_ -- --test-threads=1` | 4 通过，0 失败 | 管理员签发、邀请开户与撞名、真实资料、并发 reset/change、延迟提交故障与密码空格。执行于精确管理授权及最新 action HTTP 编辑之前。 |
| `cargo test -p cobotrix-root security_ -- --test-threads=1` | 69 通过，12 失败 | 新配置改变了旧 fixture 的 issuer/Cookie 模式，旧 fixture 错误期待生产 HTTP 的 Host Cookie。已区分固定 HTTPS 生产策略与显式 loopback 开发策略，待完整复跑。 |
| `cargo test -p cobotrix-root cobotusr::routes::auth:: -- --test-threads=1` | 17 通过，1 失败 | fixture 修复后仅诊断用例期待旧的无效生产 HTTP 原因；已改为生产 HTTPS 缺失 Cookie 的原因。 |
| `cargo test -p cobotrix-root security_browser_loopback_aliases_and_rejection_diagnostics -- --test-threads=1` | 1 通过，0 失败 | 上述诊断用例复跑通过；仍不能代替完整旧安全回归。 |
| `cargo test -p cobotrix-root identity_actions_http_ -- --test-threads=1` | 3 通过，0 失败 | 邀请/重置精确权限、动作 proof 的 HttpOnly Cookie 和浏览器绑定、并发只读 inspection、未知字段拒绝、密码提交故障不清 Cookie、reset 清本人根。执行于后续资格 HTTP 增补前。 |
| `cargo test -p cobotrix-root identity_ -- --test-threads=1` | 49 通过，0 失败 | 当前账户服务、4 个账户动作/治理 HTTP 用例、迁移升级/回退、应用/分配仓储、会话、RSA/JWKS、并发和提交故障回归。覆盖最后管理员并发保护、developer_managed 负责人保护、多来源代次、停用/启用不复活旧会话，以及迁移继承用户拒绝且不恢复编辑后的角色授权。仍无 authorize/token HTTP 闭环和跨站浏览器证据。 |

## 授权、令牌与内省阶段的补充证据

| 命令/范围 | 实际结果 | 覆盖及限制 |
| --- | --- | --- |
| `cargo test -p cobotrix-root identity_migration -- --test-threads=1` | 11 通过，0 失败 | 账户/应用/会话约束、升级保留历史、显式 Provider 回填、回退与生产密钥；执行于后续协议编辑之前。 |
| 授权服务及 HTTP 筛选 | 6 通过，0 失败 | SSO 复用根、独立 RP、同意原子完成、强制重新认证、合法错误回调、仅 flow_id 的登录/同意恢复。该阶段执行，不作为真实跨站浏览器证据。 |
| 原授权码/刷新筛选 | 11 通过，0 失败 | 标准 Web/SPA 兑换、代码一次消费、真实签名、刷新轮换/重放、根期限及提交故障；执行于内省/CORS 增补之前。 |
| 原 UserInfo 筛选 | 2 通过，0 失败 | 按原 scopes 返回真实资料、注册表与当前准入验证、数据库故障；执行于 CORS 增补之前。 |
| `cargo test -p cobotrix-root identity_protocol_ -- --test-threads=1` | 16 通过，1 失败 | 17 项协议集成；唯一失败为内省测试错误期待 scope 顺序，已按集合的稳定排序改正。执行于后续完整认证格式及 CORS 测试增补之前。 |
| `cargo test -p cobotrix-root identity_protocol_introspection_ -- --test-threads=1` | 1 通过，0 失败 | scope 断言修正后，注册表 access/refresh 及跨客户端隐藏复跑通过；不是完整内省回归。 |
| `cargo test -p cobotrix-root identity_protocol_cors_ -- --test-threads=1` | 1 通过，0 失败 | 精确 SPA Origin/预检、中心与内省不开放、目标环境 Web 跨域兑换拒绝、SPA 兑换/UserInfo/环境停用。执行于显式 wildcard 旧 CORS fixture 和最终认证格式编辑之前。 |
| `cargo test -p cobotusr --lib` | 75 通过，0 失败 | 领域规则、配置及协议响应规则；执行于最后 Basic/Bearer 大小写编辑之前。 |
| `cargo clippy -p cobotusr --all-targets -- -D warnings` | 通过 | 既有 proc-macro-error2 future incompat notice 保留；执行于最后认证格式及 legacy Origin 编辑之前。 |
| `cargo test -p cobotrix-root identity_ -- --test-threads=1` | 71 通过，2 失败，372.20 秒 | 73 项完整 identity 回归覆盖账户、授权/同意、code/refresh/UserInfo、内省、CORS、迁移、会话与故障。失败仅为 token.revoke 的审计字段不在白名单，发生在新增注册服务之前。 |
| `cargo test -p cobotrix-root identity_protocol_revoke_ -- --test-threads=1` | 2 通过，0 失败，8.80 秒 | 将审计摘要 `subject` 改为已有 `target_user_id` 后复跑；验证客户端审计 actor_id 为空、目标用户准确、grant/RP/audit/outbox 原子撤销、跨客户端及中心根隔离、提交失败完整回滚。 |
| `cargo test -p cobotrix-root identity_application_service_registration_ -- --test-threads=1` | 3 通过，0 失败，15.71 秒 | 自助两环境注册、独立密钥与生产 disabled、无开发资格拒绝、SPA 无密钥/机器 grant、并发同键只有一个应用、重复只读回执、不同请求冲突、延迟提交失败无半应用。初版测试浏览器夹具格式错误已修，不算产品服务失败。 |
| `cargo test -p cobotrix-root identity_developer_http_ -- --test-threads=1` | 2 通过，0 失败，13.63 秒 | 注册的 Cookie/CSRF/资格/幂等键/owner 注入拒绝；应用详情、成员范围分页及操作回执不包含密钥；reader 列表/创建/轮换/停用、应用与环境路径绑定、旧 revision 冲突及匿名拒绝。 |

以上已取得的历史结果对应执行时的源码，不自动覆盖之后的编辑。尚未复跑完整旧 security/OIDC 与完整质量门禁。

这些是服务基础、仓储与身份协议 HTTP 证据。标准 RP 发起退出和持久化网络投递器已经实现并获得局部测试；真实 HTTPS 成功发送、三入口 UI、参考 RP 与跨站浏览器仍待验收，不能据此认定跨应用 SSO 已交付。

## 开发中的范围

纯身份规则、加法迁移、共享 PostgreSQL 事务、统一锁序、动作流程、应用版本/审批/分配仓储、中心/RP/code、审计和持久 outbox、managed 凭据状态与签名已落盘，持续补齐行为测试。生产固定 issuer、持久密钥及配置边界已实现并有上述局部证据。中心密码认证、只读恢复、浏览器 CSRF 适配和个人退出 HTTP 已接入，标准协议编排已接入，真实客户端与完整端到端场景仍待核验。

授权、同意、标准 code/token/refresh/UserInfo、注册表内省/标准撤销已接入服务和 HTTP；自助两环境注册、应用详情/成员范围列表、安全操作回执及 reader 管理 HTTP 已接入并取得上述局部证据。reader 管理写事务现在同时固定 application_id/environment_id；列表只选择公开管理元数据，不能取回散列或旧秘密。生产审批、管理员直接接入和组/分配服务及 HTTP 已实现并取得局部证据；Reader 新增管理员精确操作与幂等回执，完整接入配置和 UI 仍待验收，不能标记整个 T44 完成。Discovery 已宣告实现的标准 /oidc/end-session；SPA CORS 按精确 Origin 且在目标签发事务重检。中心及内省不提供浏览器跨域访问。

仍需完成旧管理旁路的全部并发/性能核验、历史显式纳管的完整升级验收、真实 HTTPS 投递、前端三个入口、标准 Web/SPA/API 示例、真实跨站 TLS 三浏览器和完整质量门禁。验收清单只在对应完整行为得到证据后勾选。

## 应用治理阶段的补充证据（2026-11-09）

| 命令/范围 | 实际结果 | 覆盖及限制 |
| --- | --- | --- |
| `cargo test -p cobotrix-root -- --test-threads=1` | 183 通过、5 失败，共 188 项，641.06 秒 | 执行于治理扩展之前；5 个失败涉及固定 discovery 在时钟不可用时读取和冲突客户端认证的错误分类，修复后相关筛选已通过，尚未完整复跑。 |
| `cargo test -p cobotrix-root cobotusr::routes::auth:: -- --test-threads=1` | 18 通过，0 失败，69.62 秒 | 修复后旧认证、注册、退出、CSRF、撤销及故障行为回归。 |
| `cargo test -p cobotrix-root security_t21_http_ -- --test-threads=1` | 3 通过，0 失败，9.30 秒 | 固定部署 discovery/JWKS 可读取，缺失或不可信时钟、错误签名不能消费授权码。 |
| `cargo test -p cobotrix-root identity_protocol_token_http_ -- --test-threads=1` | 1 通过，0 失败，7.73 秒 | 标准 Basic/post/SPA 和冲突认证 HTTP 错误分类。 |
| `cargo test -p cobotusr --lib` | 78 通过，0 失败 | 执行于最新用户组/分配扩展之前。 |
| `cargo test -p cobotrix-root identity_application_governance_ -- --test-threads=1` | 4 通过，0 失败，20.21 秒 | 配置切换/新候选 client、固定审批快照和独立审批/有限退出确认、单环境秘密与独立 secret_revision/故障回滚、候选隐私/owner交接/多来源保留。执行于用户组/分配扩展之前。 |
| `cargo test -p cobotrix-root identity_developer_http_configuration_ -- --test-threads=1` | 1 通过，0 失败，6.35 秒 | 注册、生产草案、提交、自批拒绝、独立批准、精确 deny 优先、停用及缺少 CSRF；执行于 secret_revision 及分配扩展之前。 |
| `cargo test -p cobotrix-root identity_protocol_reader_ -- --test-threads=1` | 1 通过，0 失败，12.00 秒 | reader 在环境信任停用时内省失效，重新启用不绕过当前注册表状态。 |

管理员直接接入、组/分配 HTTP、旧接口治理、标准 RP 退出与网络投递器已经落盘；生命周期全部竞态、旧接口影响图性能、真实 HTTPS 投递、三个 UI 入口与真实浏览器仍未完整验收。以上为局部功能证据。

## 旧接口、Reader、退出与显式纳管补充证据（2026-11-09）

| 命令/范围 | 实际结果 | 覆盖及限制 |
| --- | --- | --- |
| 用户组/分配/管理员服务及 HTTP 筛选（前序已执行结果） | 14 通过、0 失败 | 管理员直接注册、组/分配、精确权限、提交失败；保留前序范围记录，不补造具体命令。 |
| `cargo test -p cobotrix-root identity_protocol_logout_ -- --test-threads=1`（前序） | 5 通过、0 失败 | 标准 /oidc/end-session、hint 关联、确认绑定、匿名与另一浏览器根隔离及故障回滚。 |
| `cargo test -p cobotrix-root identity_session_ -- --test-threads=1`（前序） | 16 通过、0 失败 | 包含 dispatcher/worker/cleanup 的租约、稳定 jti、重试、失败、取消与重启；成功投递采用 mock，不代表真实 TLS 成功发送。 |
| `cargo test -p cobotrix-root identity_protocol_ -- --test-threads=1`（前序） | 24 通过、1 失败 | 失败为 metadata OPTIONS CORS；快速路径已限制为无 Origin 的 GET/HEAD，尚未取得修复后的完整复跑结果。 |
| `cargo test -p cobotrix-root identity_protocol_reader_ -- --test-threads=1`（前序） | 4 通过、0 失败，38.35 秒 | 用途隔离、同键并发与无秘密回执、admin 非成员精确读写权限、延迟 COMMIT 回滚；执行于本轮纳管扩展之前。 |
| `cargo test -p cobotrix-root test_ -- --test-threads=1`（前序） | 17 通过、0 失败，110.90 秒 | 旧 CRUD、recording、认证、完整旧 OIDC、sync_permissions；执行于本轮旧 consent 事务及 bootstrap 封闭之前。 |
| `cargo test -p cobotrix-root identity_migration_legacy_ -- --test-threads=1` | 8 通过、0 失败，54.37 秒 | SQL fixture 修正后，当前管理凭据、有效权限变更撤销、最后管理员/owner、并发管理员降级、旧 client 原子审计与 COMMIT 故障；执行于本轮 consent/bootstrap 扩展之前。 |
| `cargo test -p cobotrix-root identity_developer_http_reader_ -- --test-threads=1` | 1 通过、0 失败，10.02 秒 | Reader HTTP 路径、幂等键、revision、脱敏列表和匿名拒绝。 |
| `cargo test -p cobotusr --lib` | 84 通过、0 失败 | 包含两项纳管输入规则；执行于旧 token 客户端锁增补之前。 |
| `cargo test -p cobotrix-root identity_migration_adoption_ -- --test-threads=1` | 6 通过、0 失败，44.80 秒 | 关联 application/client/hash/key 保留、孤立 SPA 两环境、同键并发首次秘密、issuer/多应用/快照/类型拒绝、COMMIT 失败无半纳管、HTTP 当前权限/CSRF/键、bootstrap 重启无写入。尚未覆盖历史 JWT issuer 解析、派生图专门样本和纳管/签发交错的专项并发。 |

T66 使用单例 Policy barrier、Application/Environment/OAuthClient 等统一锁后重检注册凭据和 method/URI 权限；旧角色、权限、用户覆盖、账户、应用及客户端写入共用原预算和事务。账户删除改为停用，旧开户/密码/managed 编辑拒绝并指向受控流程，recording/人员/权限目录限管理员。有效权限变化才递增账户代次、撤销根并重算环境。全范围发现目前最多 1000 用户且逐用户加载，容量与性能尚未完整核验。

本轮旧 consent 改为共享锁后当前认证与客户端状态校验，同意/代码/审计同事务；旧 token code/refresh 新增 OAuthClient 锁与显式纳管串行。bootstrap 当前只读保留历史条目，新条目报告需显式治理注册，不创建 legacy 客户端或隐式应用。历史兼容测试改为隔离库显式预置旧数据。

T67 新增 MigrationRead/MigrationScan/MigrationAdopt 精确操作、分页扫描、管理员显式纳管和安全回执。已获得六项局部证据，仍继续补充冲突来源及并发核验；生产数据库迁移演练、UI 和运维指南未完成。所有完整平台清单保持未勾选。

## 控制台阶段与平台现状盘点（2026-11-09）

本轮新增 `domain/identity/console.rs`、`service/identity/console.rs`、`repository_console.rs`、`route/identity_console.rs` 及 `m20261109_000012_identity_console_operations`，接入账户中心与管理控制台的读取/撤回/重试接口：`GET /account/apps`、`GET /account/consents`、`POST /account/consents/{id}/revoke`、`GET /account/sessions`、`GET /account/rp-sessions`、`POST /account/rp-sessions/{id}/revoke`、`GET /admin/accounts`、`GET /admin/audit-events`、`GET /admin/logout-deliveries`、`POST /admin/logout-deliveries/{id}/retry`。查询使用闭合 `ConsoleKind`/`ConsoleQuery` 与 typed DTO，分页默认 25、上限 100；账户读取强制使用当前中心身份，管理读取校验实际管理员与精确操作权限；响应不包含摘要、nonce、protocol_sid、token_jti、endpoint_snapshot 或租约秘密。同意撤回在同一事务内删除该环境本人同意、撤销该环境授权并写审计/outbox；RP 撤回复用个人会话撤销；投递重试只接管通知期限内、未送达且无有效租约的原记录，保留原 endpoint/jti/attempt_count。

前端已落盘三个入口及页面：`/account`、`/account/apps`、`/account/consents`、`/account/sessions`、`/developer/apps`、`/developer/apps/:id`、`/admin/accounts`、`/admin/apps`、`/admin/apps/:id`、`/admin/groups`、`/admin/audit`、`/admin/deliveries`、`/admin/migrations`，以及独立 `/account/action`。中心 HTTP 使用同源 `/api/v1`、Cookie、CSRF 与 `X-CSRF-Token`，不附旧 Bearer、不走旧自动刷新、不持久化秘密；资格变化会重置中心身份与查询缓存。

| 命令/范围 | 实际结果 | 覆盖及限制 |
| --- | --- | --- |
| `cargo check -p cobotusr` | 通过，16.32 秒 | 本轮控制台服务/路由编译检查；仅有既有 `proc-macro-error2` future-incompat notice。 |
| `cargo test -p cobotrix-root identity_console_ -- --test-threads=1` | 3 通过、0 失败，19.80 秒 | 本人元数据最小披露与 admin 精确授权、非法分页/停用身份拒绝；同意撤回提交失败不产生半状态；投递重试保留 endpoint/jti/attempt_count、拒绝租约冲突/已送达/过期/未知记录。 |
| `pnpm --filter @cobotweb/cobotusr test` | 8 个文件、79 项通过、0 失败，1.51 秒 | 中心会话恢复、CSRF/401/403、页面入口与 action 秘密处理等前端单测。 |
| `pnpm --filter @cobotweb/cobotusr check` | 通过 | `vue-tsc -b` 类型检查。 |
| `pnpm --filter @cobotweb/cobotusr lint` | 通过 | ESLint 对 `src`、`tests` 与配置执行，无错误。 |
| `pnpm --filter @cobotweb/cobotusr build` | 通过，2.45 秒 | 生成 `assets/cobotusr` 产物；browserslist 数据偏旧与 outDir 常规 warning 不影响构建。 |
| `cargo fmt --all -- --check` | 通过 | 当前工作区格式检查。 |
| `cargo clippy --workspace --all-targets -- -D warnings` | 通过，5.07 秒 | 仅保留既有 `proc-macro-error2` future-incompat notice。 |
| `cargo test --workspace -- --test-threads=1`（第一次完整运行） | root 集成测试 228 通过、1 失败，992.98 秒；auth 另 50 项通过 | 失败为 `identity_migration_managed_session_code_and_logout_links_are_complete`：新增种子迁移后，测试仍以“回滚最后一步”回到 `identity_code_rp` 之前，实际只回滚了 no-op 种子，升级断言得到 0。 |
| 修复后单项复跑 | 1 通过、0 失败，1.91 秒 | 按迁移名动态计算回滚步数；该用例已覆盖回到 `identity_code_rp` 前 schema 再升级的路径。 |
| `cargo test --workspace -- --test-threads=1`（修复后完整复跑） | 全部通过：auth 50、root 集成 229、cobotusr 86；auth doctest 8 通过/1 ignored、cobotusr doctest 1 通过；root 集成 1004.25 秒 | 默认 feature 的完整 workspace 回归；失败用例已在同一源码状态下转绿，无失败、无新增 ignored。尚未加 `--all-features`，不能替代 checklist C82 的最终门禁。 |

第一次完整运行中唯一失败是测试夹具的迁移位置假设，不是产品约束失败；修复只改测试回滚步数计算。修复后的完整 workspace 默认 feature 复跑已全部通过，但 T102/AC27 还要求 `--all-targets --all-features`、参考客户端和真实浏览器/HTTPS 证据，因此当前仍不能宣称整体交付完成。

当前代码盘点：工作区有 94 个未跟踪新文件、81 个已跟踪修改文件；`cobotusr/src` 约 4.1 万行 Rust，`tests` 约 2.1 万行，中心前端约 8.2 千行；路由层 30 个模块、119 处路由注册。核心后端已覆盖账户/邀请/重置、中心 SSO、授权/同意/code/refresh/UserInfo/内省/撤销、标准退出与后通道投递、两环境应用注册/审批/负责人/Reader、组/分配、历史纳管和审计/outbox。

尚未完成的关键交付：标准 `openid-client` Web 示例、`oidc-client-ts` SPA 与独立保护 API 示例尚未创建；没有真实跨站 TLS、Chromium/Firefox/WebKit 的 A→B SSO/新标签页/退出验收，也没有真实 HTTPS 后通道成功发送证据；`tests/e2e` 仍使用 `127.0.0.1:5188` 假接口 server。UI 已补审批时固定快照对比（见下节），仍缺开发者本人应用投递概况、旧隐藏管理页面的中心身份适配，以及 Reader/候选/历史扫描的完整分页遍历。旧 `legacy_client` 仍可能以错误差异暴露 app_key 是否存在，旧全范围治理仍受 1000 用户和逐用户加载限制。生产升级演练、容量/延迟、备份恢复和运维文档未完成。C01—C90 及四项纳管补充检查共 94 个复选框全部保持未勾选。

## 全 feature 门禁、依赖审计与浏览器修复（2026-11-09 续）

本轮把门禁从默认 feature 推进到 `--all-features --all-targets`，并修复了门禁暴露的两个真实产品问题：

1. `service/identity/browser.rs::verify_write` 原先只接受 `requested == issuer.origin()`，导致显式 loopback 开发模式下 `127.0.0.1` 与 `localhost` 的主机别名被拒。现改为复用 `service/security/browser.rs::origin_allowed`，与旧管理面保持同一套显式别名规则。
2. 当浏览器以明文 HTTP 访问 HTTPS issuer 时，Secure CSRF Cookie 不可能存在，但旧路径先做 Origin 判定并返回泛化 `forbidden`。现在身份写路径与旧 `validate_csrf` 都会在同主机同端口、仅 scheme 为 `http` 对 `https` 时返回 `csrf_cookie_requires_https`，前端据此展示可操作提示；请求仍然被拒绝，不放宽任何来源。

同时补齐审批页“固定申请快照对比”：`app-detail.vue` 对比提交时固定的 `approval.data.value.config` 与当前 `env.active_config`，逐字段展示当前/申请差异、固定版本、快照负责人和退出限制确认；新增 `approval-snapshot.test.ts` 覆盖有差异/无差异两种状态。

依赖侧：`testcontainers 0.25 -> 0.28`、`utoipa 5.4 -> 6.0`、`utoipa-swagger-ui 9 -> 10`、`rsa 0.9.10 -> 0.10.0`（改用 `rsa::PrivateKey`、`n_bytes()/e_bytes()` 与 `getrandom::SysRng`），并定向更新 h2/rustls/rustls-webpki/webpki-roots/anyhow/tokio/event-listener/ctutils/spin/chacha20/quinn-proto 等传递依赖；`sea-orm` 保持 `2.0.0-rc.40`（全量 update 会升到 2.0.4 并产生 646 个编译错误）。`libs/auth` 删除未使用的 `headers`、`log`。

| 命令 | 实际结果 | 覆盖及限制 |
| --- | --- | --- |
| `cargo test -p cobotrix-root --all-features security_browser_ -- --test-threads=1` | 5 通过、0 失败，37.39 秒 | 含 Chromium/Firefox/WebKit 的 `127.0.0.1` 与 `localhost` 登录/刷新/退出、WebKit 明文 HTTP 诊断；真实跨站 TLS 仍不在其中。 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 通过，24.68 秒 | 仅保留 `proc-macro-error2` future-incompat notice。 |
| `cargo test --workspace --all-targets --all-features -- --test-threads=1` | auth 50、root 集成 231、cobotusr 86，全部 0 失败；root 集成 734.87 秒 | 本轮依赖升级与浏览器修复后的完整全 feature 回归；`--all-targets` 不含 doctest，doctest 单独执行。 |
| `cargo test --workspace --doc --all-features` | auth 8 通过/1 ignored、cobotusr 1 通过，0 失败 | 文档示例编译与运行。 |
| `cargo build --workspace --all-targets --all-features --release` | 通过，2 分 39 秒 | release 全目标构建。 |
| `cargo fmt --all -- --check` | 通过 | 当前工作区格式。 |
| `cargo machete` | 未发现未使用依赖 | 删除 `libs/auth` 的 `headers`、`log` 后复跑。 |
| `cargo audit` | 退出码 0；剩余 5 条在 `.cargo/audit.toml` 中逐条记录理由 | `libsqlite3-sys`/`rkyv` 仅来自未启用的可选驱动/特性，`arrow-data` 仅来自未启用的 `sea-orm-arrow`，`astral-tokio-tar` 仅 dev（testcontainers），`proc-macro-error2` 仅构建期 proc-macro。 |
| `pnpm --filter @cobotweb/cobotusr test` | 9 个文件、81 项通过、0 失败 | 含新增审批快照对比用例。 |
| `pnpm --filter @cobotweb/cobotusr check` | 通过 | `vue-tsc -b`。 |
| `pnpm --filter @cobotweb/cobotusr lint` | 通过 | ESLint。 |
| `pnpm --filter @cobotweb/cobotusr build` | 通过，2.71 秒 | 生成 `assets/cobotusr` 产物。 |

本轮之后仍未交付的部分保持不变：`examples/` 参考客户端、真实跨站 TLS 与三浏览器 A→B SSO/新标签页/退出验收、真实 HTTPS 后通道成功发送、开发者本人应用投递概况、旧隐藏管理页的中心身份适配，以及 Reader/候选/历史扫描的完整分页遍历。94 个复选框继续保持未勾选。

## 本人应用投递概况与最终门禁复核（2026-11-10）

本节记录上一节列出的四项待办中已关闭的三项，以及在同一源码状态下的完整门禁复跑。

1. 开发者本人应用投递概况：`ConsoleKind::AppDeliveries` 与 `ConsoleQuery::AppDeliveries` 进入闭合查询枚举，通用 `list()` 对该种类返回 `InvalidInput`，只能经 `ConsoleService::list_app_deliveries` 进入——管理员要求 `DeliveryRead` 精确权限，普通用户走 `require_membership`（成员 + 资格启用），并校验环境属于该应用。HTTP 入口为 `GET /developer/apps/{id}/environments/{env}/logout-deliveries` 与 `GET /admin/apps/{id}/environments/{env}/logout-deliveries`，共用同一 handler，由 Dispatcher 按路径前缀区分管理面。开发者页新增“退出通知投递”区块，展示状态中文名、尝试次数、错误码、时间与游标翻页，文案明确 2xx 只表示对端接受通知、本地撤销先于通知生效。
2. 前端分页补齐：`useCursorPage` 支持复合 key 与额外 active 条件；应用详情页的 Reader 列表由草稿首屏改为游标翻页，候选协作者改为首页 + “加载更多候选账户”累积，避免只取第一页造成的漏选。
3. 参考示例与 SPA nonce：SPA 在每次 `signinRedirect` 前生成 32 字节 CSPRNG 的 base64url（43 字符）nonce；浏览器夹具的同意页改成轮询两种结果以消除竞态；中心退出确认页补 `Referrer-Policy: same-origin` 并将 `protect_response()` 改为 `entry().or_insert()`，使处理函数显式设置的响应头优先，修复确认页 POST 403。WebKit 在 macOS 14 以下与 Playwright 1.65 不兼容，改为 Darwin major < 24 时使用 `externalWebkit`，由 `playwright-webkit@1.61` 单独跑四组场景。
4. `legacy_client` 先校验客户端凭据再解析 `app_key`，消除按错误差异判断 app_key 是否存在的侧信道。

| 命令 | 实际结果 | 覆盖及限制 |
| --- | --- | --- |
| `cargo test -p cobotrix-root --all-features identity_console_app_deliveries -- --test-threads=1` | 1 通过、0 失败，3.94 秒 | 同应用同环境的两次撤回产生 ≥2 条投递；limit=1 连翻两页 id 递增；生产环境空列表；非成员 `OwnerRequired`；跨应用环境 `Forbidden`；limit=0 与非法游标 `InvalidInput`；`/developer` 与 `/admin` 均 200、`Cache-Control: no-store`、无敏感键、未登录 401。 |
| `cargo test -p cobotrix-root --all-features identity_legacy -- --test-threads=1` | 8 通过、0 失败 | 含先凭据后 app_key 的 401 行为。 |
| `cargo test -p cobotrix-root --all-features sync_permissions -- --test-threads=1` | 2 通过、0 失败 | 错误 secret 与不存在 app_key 均 401。 |
| `pnpm --filter @cobotweb/cobotusr test` | 10 个文件、91 项通过、0 失败 | 含投递概况映射、虚假候选分页与 `getAppDeliveries` 桩。 |
| `pnpm --filter @cobotweb/cobotusr check` / `lint` / `build` | 均通过，构建 2.47 秒 | `vue-tsc -b`、ESLint、产物落 `assets/cobotusr`。 |
| `cargo test --workspace --all-targets --all-features -- --test-threads=1` | 全部通过：auth 50（3.62 秒）、root 集成 233（776.85 秒）、cobotusr 86（0.21 秒），0 失败、0 新增 ignored，退出码 0 | 含三引擎 SSO 用例（Chromium+Firefox 10 项 17.0 秒、WebKit 四组场景通过）与本人投递概况用例；此为本轮最终门禁，早期被中断的运行不计入。 |
| `cargo test --workspace --doc --all-features` | auth 8 通过 / 1 ignored、cobotusr 1 通过（compile fail 用例），0 失败 | 文档示例编译与运行。 |
| `cargo build --workspace --all-targets --all-features --release` | 通过，1 分 27 秒 | release 全目标构建，独立部署形态未变。 |
| `cargo fmt --all -- --check` | 退出码 0 | 当前工作区格式。 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 通过 | 仅既有 `proc-macro-error2` future-incompat notice。 |
| `pnpm install --frozen-lockfile` | 通过，46 毫秒，`pnpm-lock.yaml` 无改动 | Node v24.21.0、pnpm 12.10.1、Rust 1.99.1；示例标准客户端 `oidc-client-ts ^3.5.0`、`vite ^7.1.7`、`vitest ^5.1.0`。 |
| `pnpm --filter './examples/*' check` | 两个示例 `tsc --noEmit` 通过 | oidc-web 与 oidc-spa。 |
| `pnpm --filter './examples/*' test` | oidc-web 4 文件 54 项、oidc-spa 4 文件 59 项，全部通过 | 标准客户端协议/会话/退出/保护缓存与标签隔离。 |
| `pnpm --filter './examples/*' build` | 均通过 | SPA 产物 75.58 kB；产物中 `VITE_CLIENT_SECRET`/`VITE_READER_*`/`VITE_SESSION_SECRET` 只作为禁止暴露的校验名单出现，无秘密值；`dist` 与 `.env.local` 均被 `.gitignore` 忽略。 |
| `cargo machete` | 0.9.2，未发现未使用依赖 | — |
| `cargo audit` | 退出码 0；载入 1430 条 advisory，扫描 492 个 crate | 剩余 5 条仍在 `.cargo/audit.toml` 中逐条记录理由。 |

据此本轮勾选 C75—C82、C84、C85 共 10 项。C83 只取得 doctest 结果，“接入/升级说明按步骤复现”仍依赖 E7，因此保持未勾选；C86—C90 与 E1—E7 的全流程证据同样保持未勾选。剩余未交付项：真实 HTTPS 后通道成功投递（`HttpsLogoutTransport` 尚无自签 CA 注入，只有 mock）、旧隐藏管理页的 legacy Bearer 适配、旧全范围治理的 1001 行上限与逐用户加载、生产库升级/备份/容量演练、CI 三平台实际结果。其中真实 HTTPS 后通道投递与旧隐藏管理页两项已在同日后续提交中关闭，见下文两节。

上述未交付项已逐条整理到 [gaps.md](gaps.md)，含影响与下一步，作为阶段性交接清单。

## 旧 RBAC 管理面退役（2026-11-10）

按新的产品定位，旧 RBAC 管理面整体从产品中移除，而不是继续以 `hidden: true` 的中间态存在：

- 删除 `pages/users`、`pages/roles`、`pages/authority`、`pages/permissions`、`pages/recordings`、`pages/applications`、`pages/oauth`、`pages/authorized-apps`，以及只被它们使用的 `permission-catalog`、`permission-assignment` 组件、`use-permission-assignment` 组合式函数和 `pages/login/api`。
- `/users`、`/users/create` 重定向到 `/admin/accounts`，`/roles` 到 `/admin/groups`，`/authority`、`/authority/assign`、`/permissions`、`/permissions/assign` 到 `/admin/apps`，`/recordings` 到 `/admin/audit`；`/legacy/*` 别名同样直接指向新入口。旧 URL 不再挂载任何页面组件。
- 删除围绕旧管理接口的 e2e 套件（`fixture.ts`、`serve.mjs`、`real-auth.mjs` 与 7 个旧 spec）及其 `playwright.config.ts`，`test:e2e` 改为运行企业 SSO 套件；旧套件依赖的 `/user`、`/login`、`/refresh` mock 与中心 Cookie 会话不兼容，已无法反映当前产品。
- 后端旧 `/api/v1/{user,role,authority,permission}` 与旧 `/login` 保留，供既有客户端、MCP 与历史数据兼容；写入仍经身份治理。

| 命令 | 实际结果 | 覆盖及限制 |
| --- | --- | --- |
| `pnpm --filter @cobotweb/cobotusr check` | 通过 | `vue-tsc -b`，删除后无悬空引用。 |
| `pnpm --filter @cobotweb/cobotusr lint` | 通过 | ESLint 覆盖 `src`、`tests` 与配置。 |
| `pnpm --filter @cobotweb/cobotusr test` | 11 个文件、92 项通过 | 新增 `legacy-routes.test.ts` 断言旧路径只重定向、不挂载组件；移除 2 项只针对已删除组合式函数的用例。 |
| `pnpm --filter @cobotweb/cobotusr build` | 通过，2.19 秒 | 主包 303.24 kB，旧管理页 chunk 不再产出。 |

该变更只影响前端产品面，未改动后端协议与数据；C65 要求的“旧入口不能绕过治理”仍由后端测试覆盖，浏览器端不再提供旧入口。

## 后通道投递的真实 HTTPS 证据（2026-11-10）

此前 `HttpsLogoutTransport` 的成功路径只有 mock 与明文 HTTP 夹具，无法证明真实 TLS 下的投递、超时与重试行为。本轮补齐：

- `HttpsLogoutTransport` 新增 `with_extra_roots`，为投递客户端追加信任根；生产装配仍只用内置根证书。
- 新增 `with_loopback_fixture`，只有隔离测试策略能放行 loopback；`allowed_ip` 的 loopback 开关默认关闭，生产继续拒绝 loopback、链路本地与云元数据地址。
- `tests/cobotusr/routes/identity_logout_transport.rs` 用 `rcgen` 现场签发 CA 与 `rp.example` 叶证书，`tokio-rustls` 在 loopback 起真实 HTTPS 接收端，投递仍经 `resolve_to_addrs` 固定解析地址，与生产同一条代码路径。

| 命令 | 实际结果 | 覆盖及限制 |
| --- | --- | --- |
| `cargo test --test mod identity_logout_transport -- --test-threads=1` | 5 通过、0 失败，0.16 秒 | 204 连续两次投递各产生一条 `logout_token=token-value` 表单请求（重复投递）；不注入信任根时证书校验失败→`Retry network_error` 且接收端零请求；429 带 `Retry-After: 30`→`RetryAfter{http_429,30}`；503→`Retry http_5xx`；400→`PermanentFailure http_4xx`；302 不跟随→`PermanentFailure http_redirect`；接收端延迟 400ms 对 120ms 超时→`Retry timeout`；非 allowlist 主机解析到私网地址→`PermanentFailure ssrf_blocked`。 |
| `cargo test -p cobotusr --lib logout_transport` | 3 通过、0 失败 | 地址策略：loopback、链路本地、元数据、文档与隧道地址默认拒绝；私网仅对显式 allowlist 放行；loopback 仅在隔离测试策略下可达。 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 通过 | 仅既有 `proc-macro-error2` future-incompat notice。 |

该证据关闭 [gaps.md](gaps.md) 的 G1。C73 仍要求 E6 的完整退出矩阵（组成员移除、同意撤回、密码修改、账户停用、重启与内省故障），C86 仍要求三引擎实跑报告，因此两项保持未勾选。

## 全范围治理上限可配置（2026-11-10）

旧管理接口的全范围写入（如新增一条权限）会加载全部受影响账户，此前硬编码 `LIMIT 1001`：超过 1000 人的企业直接得到 `capacity_exceeded`，且该值无法按部署规模调整。现在：

- `identity.legacy_governance_max_users` 取代硬编码上限，默认 1000，校验范围 1—10000，配置文件缺省时沿用默认值。
- 查询按“上限 + 1”取样，超限在事务内、执行变更回调之前返回 `capacity_exceeded`（HTTP 503），不会留下部分写入。
- 逐用户加载的成本仍在 2 秒安全预算内结算，运维指引据此给出容量建议，见 [enterprise-identity-upgrade.md](../../operations/enterprise-identity-upgrade.md) 第 9 节。

| 命令 | 实际结果 | 覆盖及限制 |
| --- | --- | --- |
| `cargo test --test mod routes::authority -- --test-threads=1` | 5 通过、0 失败 | 新增 `authority_write_refuses_estates_above_the_configured_bound`：上限设为 1 并插入第二个用户后，`POST /api/v1/authority` 返回 503 `capacity_exceeded`，且 `authority` 表无残留行；原有增删改查用例不受影响。 |
| `cargo test -p cobotusr --lib config_identity` | 2 通过、0 失败 | 默认值 1000；0 与 10001 均被校验拒绝。 |
| `cargo test --workspace --all-targets --all-features -- --test-threads=1` | 全部通过：auth 50、root 集成 237（757.92 秒）、cobotusr 86，0 失败 | 本轮最终门禁。237 = 上轮 233 + 5 项 HTTPS 投递用例 + 1 项治理上限用例 − 2 项已删除的旧前端浏览器用例。 |
| `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets --all-features -- -D warnings` / `cargo machete` / `cargo audit` | 全部通过 | audit 载入 1430 条 advisory、扫描 494 个 crate，退出码 0；machete 未发现未使用依赖（新增 `rcgen`、`rustls`、`tokio-rustls` 均被测试使用）。 |

同时修正上一提交遗留的回归：`security_browser_real_frontend_login_refresh_logout_all_engines` 与 `security_browser_secure_http_rejection_is_actionable_in_webkit` 仍在调用已随旧前端删除的 `tests/e2e/real-auth.mjs`，全量套件因此失败。这两个用例只覆盖已退役的旧登录页浏览器流程，本轮连同 `run_browser_auth_test` 一并删除；浏览器证据由 `identity_browser_e2e` 与三引擎 SSO 套件承担。

## 容量与延迟实测（2026-11-10）

此前容量只有定性描述，没有并发登录、刷新、退出、投递积压与会话清理的实测数据。本轮新增 `tests/cobotusr/routes/capacity.rs`，六项探针默认 `#[ignore]`，按需在目标环境跑：

```text
cargo test --release --test mod capacity -- --ignored --nocapture --test-threads=1
```

必须带 `--test-threads=1`：六个探针各自起一个数据库容器并打满 CPU，并行执行会互相抢占，同一台机器上得到的数字会低一半以上。

测量环境：macOS 14.6，Apple M3 Pro（11 个逻辑核），Postgres 跑在 Docker 里，服务与压测客户端同机，因此数字只代表“单机 + 本地数据库”的上限，不能直接外推生产。每档固定 5 秒窗口。

| 探针 | 并发 | 请求数 | req/s | P50 | P95 | 最大 | 状态分布 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 登录（32 个不同账户） | 1 | 21 | 4.2 | 222 ms | 231 ms | 695 ms | 200 × 21 |
| 登录（32 个不同账户） | 4 | 76 | 15.2 | 264 ms | 337 ms | 365 ms | 200 × 76 |
| 登录（32 个不同账户） | 8 | 128 | 25.6 | 306 ms | 386 ms | 476 ms | 200 × 128 |
| 登录（32 个不同账户） | 16 | 131 | 26.2 | 624 ms | 761 ms | 789 ms | 200 × 131 |
| 登录（同一账户） | 4 | 67 | 13.4 | 285 ms | 482 ms | 491 ms | 200 × 67 |
| 登录（同一账户） | 16 | 124 | 24.8 | 694 ms | 853 ms | 892 ms | 200 × 124 |
| 刷新轮换 | 1 | 376 | 75.2 | 9 ms | 35 ms | 68 ms | 200 × 376 |
| 刷新轮换 | 4 | 872 | 174.4 | 17 ms | 49 ms | 254 ms | 200 × 872 |
| 刷新轮换 | 8 | 929 | 185.8 | 38 ms | 77 ms | 169 ms | 200 × 929 |
| 刷新轮换 | 16 | 939 | 187.8 | 84 ms | 137 ms | 226 ms | 200 × 939 |
| 退出（一次性，32 个会话） | 4 | 32 | 149.8 | 10 ms | 54 ms | 83 ms | 200 × 32 |
| 退出（一次性，32 个会话） | 16 | 32 | 181.9 | 48 ms | 131 ms | 141 ms | 200 × 32 |

投递积压与清理：

| 探针 | 规模 | 结果 |
| --- | --- | --- |
| 后通道投递（接收端固定成功） | 40 条待投递 | 5 个批次排空，264.0 条/秒，30.3 ms/批 |
| 会话与审计清理（`limit=100`） | 2000 条审计事件 + 2000 条操作回执 | 21 次调用删除 4000 行，19641 行/秒，9.7 ms/次 |

单机数字的抖动不小：同一份代码在负载不同的两次运行里，刷新 1 并发可以相差 40%（75 与 109 req/s），退出一次性 32 条可以相差一倍。表内数字用于判断量级和瓶颈位置，不作为容量承诺；上线前应在目标规格上重跑。

探针同时暴露了一个真实缺陷：同一账户并发登录时，除首个请求外大量返回 409。原因是登录在 REPEATABLE READ 事务里取身份写屏障（`UPDATE "user" SET identity_revision=identity_revision`），后到者的快照早于先到者的提交，屏障返回 40001，而 `login_local` 只尝试一次。

根因是屏障的粒度：会话发行和撤销共用同一把排他锁，于是两个互不相干的登录也会互相判为“陈旧快照”。修复分两处：

- 会话发行改为对账户行取 `FOR SHARE`（`serialize_session_mint`）。共享锁之间兼容，同一账户的并发登录不再互相冲突；撤销仍取排他写屏障（`serialize_user_roots`），因此快照早于撤销的登录依旧以 40001 失败，不会把已撤销的资格重新发出去。
- bcrypt 校验移出写事务，只做一次；写事务重新读取账户，密码哈希或启用状态变化就按 `InvalidCredential` 拒绝，保持“校验后账户被改”的语义。

| 命令 | 实际结果 | 覆盖及限制 |
| --- | --- | --- |
| `cargo test --test mod security_t24_concurrent_logins_for_one_account_all_succeed` | 1 通过、0 失败 | 4 个线程用 `Barrier` 同时登录同一账户，断言四次登录都拿到各自的 refresh token；修复前该用例稳定失败。 |
| `cargo test --test mod security_t25_application_revocation_prevents_stale_transaction_minting_fresh_roots` | 1 通过、0 失败 | 登录在签名后被暂停，期间提交一次应用级撤销；恢复后该事务仍以 `Conflict` 失败，证明共享锁没有放宽“陈旧快照不得建根”。 |
| `cargo test --test mod security_t26_parallel_new_roots_cannot_exceed_quota` | 1 通过、0 失败 | MCP 发行仍走排他屏障，并发建根不会突破配额。 |
| `cargo test --test mod security_t24_authentication_failures_and_signing_commit_failures_leave_no_session` | 1 通过、0 失败 | 未知用户、错误密码仍返回 `InvalidCredential`；签名失败与延迟约束触发器仍返回 `DependencyUnavailable` 且不留下会话，证明校验与写入拆分没有放宽失败语义。 |
| `cargo test --release --test mod capacity_probe_login_same_user -- --ignored --nocapture --test-threads=1` | 通过 | 修复前 4 并发 36 次请求中 11 次 409、16 并发 65 次中 42 次 409；修复后 4 并发 67 次、16 并发 124 次全部 200。 |

同一账户的登录不再因为彼此而失败，但仍受 bcrypt 与数据库插入约束：16 并发 24.8 req/s、P95 853 ms，与不同账户的 26.2 req/s 基本持平，说明瓶颈在 CPU 与写入，不在锁。真实流量按“登录（不同账户）”一行做容量规划。

锁语义调整后的完整门禁在同一源码状态复跑：`cargo test --workspace --all-targets --all-features -- --test-threads=1` 全部通过，auth 50 项（3.10 秒）、root 集成 238 项（789.04 秒，6 项 ignored）、cobotusr 86 项（0.15 秒），退出码 0；含 Playwright 10 项（18.0 秒）与 WebKit 四组场景。`cargo fmt --all -- --check` 通过，`cargo clippy --workspace --all-targets --all-features -- -D warnings` 通过（仅 `proc-macro-error2` future-incompat notice），`cargo machete` 未发现未使用依赖，`cargo audit` 退出码 0（1430 条 advisory、494 个 crate）。
