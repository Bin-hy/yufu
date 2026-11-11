# 企业统一身份与受控应用接入 Checklist

日期：2026-11-09，Asia/Shanghai。

状态：本验收清单已于 2026-11-09 经用户“确认”批准，四份设计文档全部确认，进入实现与验证。依据已确认的 [spec.md](spec.md)、[plan.md](plan.md)、[task.md](task.md)。90 项功能/交付检查中，C68、C69、C75—C82、C84、C85 共 12 项已按 2026-11-11 同一源码状态下的门禁与三引擎证据勾选，证据见 [verification.md](verification.md)；其余 78 项仍待实际验证通过后勾选。

## 1. 验收范围和完成判定

验证一个企业内的统一身份、跨应用 SSO 和受控开发者接入。验收覆盖账户、开发者和管理员三个入口，独立 Web A 与 SPA B/保护 API、受控开户、生产审批、访问分配、身份披露、会话联动以及历史升级。公开开发市场、多租户、联邦与强认证按 Spec 留在后续。

每个 C 编号都用实际请求、行为观察、可控时钟/故障注入或构建测试结果检验。条目在验证通过后才能勾选；拒绝场景需同时证明保护副作用/越权状态变化未发生。模块重命名或文件移动不改变验收行为，验证命令随实际测试入口同步维护。

验证以相同源码及依赖状态下的真实输出为准。可以一次运行一个命令组并覆盖多个条目，不逐项重复同一测试；如果完整 workspace 测试已包含真实三引擎场景，不为汇总重复运行。零用例、工具/环境缺失、模拟 authorize/token 成功或历史测试报告均不能代替本轮通过证据。

结果记录到实现阶段的 `verification.md`：每条 C 编号、对应 AC、命令/场景、环境、通过/失败/未验证、脱敏证据编号。未执行项保持未勾选；修复后只复跑受影响检查和必要集成。

## 2. 隔离环境与计时约定

fixture 提供可销毁 PostgreSQL、真实 OP 和中心 UI、Web A、SPA B 及独立保护 API、临时 RSA/JWKS、可控安全时钟、假退出接收端与局部 TLS CA。端口随机，OP/A/B 在不同站点；A 与 B 不共享 Cookie/storage。Chromium、Firefox、WebKit 每个 context 有独立用户和应用。

账户、邀请、注册、审批、分配经真实接口/界面执行；只使用既有初始化方式建立首次管理员，不预置已认证浏览器，也不拦截标准授权端点伪造成功。测试接收端与故障注入替代外部依赖，不读取业务数据库环境变量，不需要真实邮件/SaaS 账号。Node/保护 API 仅信任 fixture CA，浏览器证书设置仅限该测试 context。

默认策略按计划验证：中心根 12 小时，flow 10 分钟/最多 8 个未完成，code 60 秒，身份 access 5 分钟且 access/refresh 不晚于根到期；邀请 24 小时，reset 30 分钟；通知 token 120 秒、单次网络 timeout 2 秒，治理事务沿用原 2 秒预算。期限测试使用受控时钟，不要求长时间等待。

5 秒检查以失效事务实际提交完成的 `t_commit` 为边界。fixture 在提交完成事件和保护请求发起/拒绝处使用同一单调计时基准，记录操作 ID、相对时间、HTTP 结果及保护副作用计数；不得直接比较不同进程未对齐的时钟或用 UTC 变动计算延迟。在 `t_commit + 5 秒` 之后分别请求 A 和 B/API，必须拒绝。

如果只观察到成功响应接收时间，则单独标记 `t_ack`；此结果只证明成功确认之后的拒绝时间，不冒充精确提交延迟。缓存的 2 秒默认值/5 秒硬上限还需通过可控单调时钟测试独立验证。通知不可达、内省/主库/时钟故障分别覆盖，保留仍有分配的应用/另一个用户作为可用对照。

## 3. 验证命令组

以下命令在实现阶段对应 task.md 中的测试入口和 T101 新增脚本；本阶段尚未执行。带全部 feature 的 Rust 检查先准备所需前端构建和浏览器依赖；每个筛选命令必须实际包含对应用例。V_SSO 覆盖 E1—E6；E7 同时使用迁移/旧功能回归和文档复现证据。

| 标识 | 执行命令 | 主要验证 |
| --- | --- | --- |
| V_RULE | `cargo test -p cobotusr domain::identity`；`cargo test -p auth` | 纯规则、身份/签名/CSRF 的独立验证 |
| V_ACCOUNT | `cargo test -p cobotrix-root --all-features identity_account -- --test-threads=1` | 受控开户、资料、密码、资格及 action |
| V_APP | `cargo test -p cobotrix-root --all-features identity_application -- --test-threads=1` | 注册、成员、版本、审批、秘密和停用 |
| V_ASSIGN | `cargo test -p cobotrix-root --all-features identity_assignment -- --test-threads=1` | 直接/组来源、测试用户、最终失权与并发 |
| V_SESSION | `cargo test -p cobotrix-root --all-features identity_session -- --test-threads=1` | 中心/RP 根、撤销、预算、通知、清理与故障 |
| V_PROTOCOL | `cargo test -p cobotrix-root --all-features identity_protocol -- --test-threads=1` | 授权、代码/刷新、claims、内省、标准退出与响应 |
| V_MIGRATION | `cargo test -p cobotrix-root --all-features identity_migration -- --test-threads=1` | 全新/旧库、固定主体/key、bootstrap 与显式纳管 |
| V_HTTP | `cargo test -p cobotrix-root --all-features identity_ -- --test-threads=1` | 身份 HTTP 权限与最小披露矩阵，包含对应的并发/故障用例 |
| V_REGRESSION | `cargo test -p cobotrix-root --all-features auth -- --test-threads=1`；`cargo test -p cobotrix-root --all-features security -- --test-threads=1`；`cargo test -p cobotrix-root --all-features oidc -- --test-threads=1`；`cargo test -p cobotrix-root --all-features role -- --test-threads=1`；`cargo test -p cobotrix-root --all-features authority -- --test-threads=1` | 现有登录/重放/MCP/父链/OIDC/通用权限回归 |
| V_UI | `pnpm --filter @cobotweb/cobotusr test` | 三入口、恢复、flow、草稿/缓存及真实后端状态反馈 |
| V_WEB | `pnpm --filter @cobotrix/oidc-web test` | 标准 Web 客户端、会话、通知、保护缓存 |
| V_SPA | `pnpm --filter @cobotrix/oidc-spa test` | 标准 SPA、标签隔离、独立 API 与后端 reader |
| V_SSO | `pnpm test:sso`；其 Rust 入口为 `cargo test -p cobotrix-root --all-features identity_browser -- --test-threads=1` | 真实 OP 与两个独立站点/保护 API，三引擎 E1—E6 路径 |

## 4. 行为和集成检查

### 账户准入与协作

- [ ] **C01** 匿名调用新开户及旧 `/register` 无有效邀请均被拒绝；有效邀请只建立一个账户及明确授予的资格；过期、篡改、撤销与并发重复使用不产生额外账户或资格。（对应 AC1；验证：执行 V_ACCOUNT、V_MIGRATION，比较每次请求后的账户/资格/消费记录及 HTTP 结果；重复消费仅一次成功。）

- [ ] **C02** 邀请/重置链接交换后页面地址不含原秘密，动作证明不能替代另一授权 flow 的证明；匿名资料不能自选角色、开发者资格、用户组或应用分配。（对应 AC1、AC2；验证：执行 V_ACCOUNT、V_PROTOCOL，改变 action/flow/目标参数并检查拒绝结果和脱敏地址摘要；状态未被越权修改。）

- [ ] **C03** 从 A 发起授权并接受邀请开户后，返回 A 的已注册回调，原 state、client、nonce、PKCE 关联保持正确；任意 return_to 或替换回调不能改变授权目标。（对应 AC2；验证：执行 V_SSO 的邀请开户场景，以标准客户端真实完成回调及授权码兑换，校验 state/nonce/issuer/audience。）

- [ ] **C04** 已认证现有账户接受符合目标的邀请后继续原授权，不建立第二身份；撞名要求认证现有账户，未认证或目标不符时拒绝消费。（对应 AC4、AC2；验证：执行 V_ACCOUNT、V_SSO，比较账户数量与 `(iss,sub)`，测试匿名/错误账户接受后邀请仍未消费。）

- [ ] **C05** 个人修改密码需正确旧密码，管理员重置需未过期、未消费且仍对应原认证代次的证明；成功后旧密码、所有旧中心/应用授权、待用代码和旧重置链接失效。（对应 AC3；验证：执行 V_ACCOUNT、V_SESSION、V_SSO；分别尝试旧登录/refresh/userinfo/code，并按第 2 节时序检查 A、B/API 超过 5 秒的保护操作被拒绝。）

- [ ] **C06** 停用后新登录、授权、刷新与身份获取均被拒绝；再次启用只允许重新认证，旧令牌、旧中心 Cookie 与旧应用会话不能恢复。（对应 AC3；验证：执行 V_ACCOUNT、V_SESSION、V_SSO；保留停用前凭据重试，在两个参考保护 API 测量超过 5 秒拒绝，另一个用户保持可用。）

- [ ] **C07** 修改真实资料、角色/开发者资格或重启服务后，同一账户在 A、B 中的 `(iss,sub)` 不变；缺邮箱时不拼接虚构邮箱，也不宣称未完成的邮箱验证。（对应 AC4、AC17；验证：执行 V_ACCOUNT、V_PROTOCOL、V_MIGRATION、V_SSO，比较两应用 ID Token/userinfo 的脱敏主体与 claim 名单。）

- [ ] **C08** 开发者仅能看到并管理本人参与的应用；读取或修改他应用/环境的配置、成员、版本、reader 与密钥均被拒绝，失败无状态变化。（对应 AC5、AC24；验证：执行 V_APP 和 V_HTTP，使用普通开发者及合作方逐项直接调用新/旧管理入口，比较操作前后状态；同时观察对应页面拒绝反馈。）

- [ ] **C09** 合作方只能查询本人、既有成员和管理员明确允许的协作候选；旧人员目录、角色/权限查询或名称搜索不能变成全企业目录入口。（对应 AC5；验证：执行 V_ACCOUNT、V_MIGRATION、V_HTTP，使用合作方遍历新候选及旧读取入口，响应不包含无可见关系的人员。）

- [ ] **C10** 负责人可原子交接给符合管理方式资格的启用人员；并发交接仍保留唯一负责人，不自动授予生产用户访问权；交接使相关待审批快照失效。（对应 AC6；验证：执行 V_APP、V_RULE，测试两次交接竞态、无资格目标与 admin_configured 负责人，观察唯一 owner、分配和申请状态。）

- [ ] **C11** 停用活跃应用唯一负责人或撤销其必要资格时要求先交接或停用应用；禁止停用最后一个有效管理员。撤销一般维护者/开发者资格仅在最终失去开发准入来源时撤销对应访问。（对应 AC6、AC3、AC24；验证：执行 V_ACCOUNT、V_APP、V_ASSIGN，对约束失败检查状态不变，对合法交接/停用后更新检查成功；测试保留另一来源的访问。）

### 应用注册、配置与审批

- [ ] **C12** 一次注册完整建立应用、负责人、开发/生产环境、独立客户端和信任绑定；使用返回配置即可在标准参考客户端完成真实开发登录，无需人工补数据库关系。（对应 AC7；验证：执行 V_APP、V_SSO，全部注册和接入只走界面/API；注入任一写入/明确 commit 失败后无半成品可签发，成功审计与状态同提交。）

- [ ] **C13** 同一 actor、幂等键与相同请求只形成一个已提交资源/操作；请求内容不同时冲突；响应丢失后可查询安全回执，不能恢复旧秘密或重新创建应用。（对应 AC7、AC18；验证：执行 V_APP，模拟并发及提交后回包丢失；查询/重试仅见资源摘要，需重新获取秘密时独立重置目标环境。）

- [ ] **C14** 开发/生产 callback、client_id 和 Web 凭据分别管理，access audience 来自各环境批准绑定，ID Token aud 对应其 client_id；错误登录/退出回调、通配地址及跨环境凭据均拒绝。生产 HTTPS 与开发明确 loopback 策略分别生效。（对应 AC8；验证：执行 V_RULE、V_APP、V_PROTOCOL，逐项替换回调、origin、client、secret 和 audience，检查错误与无授权结果。）

- [ ] **C15** SPA 注册不交付 client_secret，不允许 client_credentials 或自动业务/MCP 权限；Web/SPA 授权均要求 code、S256 PKCE 和已批准的最小身份范围。（对应 AC8、AC17；验证：执行 V_APP、V_PROTOCOL、V_SPA，检查注册/授权响应、浏览器配置和构建产物，错误流程拒绝且身份授权无业务执行许可。）

- [ ] **C16** 开发环境只允许有必要开发者资格的负责人/维护者或指定测试用户；管理员无自动测试访问，生产用户分配也不能自动成为开发测试资格。（对应 AC8、AC11；验证：执行 V_ASSIGN、V_PROTOCOL、V_SSO，分别使用无测试授权管理员、生产用户、开发维护者和指定测试用户完成真实授权请求。）

- [ ] **C17** 生产 active 配置未获批准时不能签发；另一有权限的管理员批准指定完整快照后，可由有分配用户完成生产登录；未完成生产草案不能提交上线。（对应 AC9；验证：执行 V_APP、V_PROTOCOL、V_SSO，比较批准前后 code/token 与界面状态，验证完整配置和实际申请版本。）

- [ ] **C18** 自批、旧版本/过时快照、重复批准及审批期间 owner/敏感配置/访问范围变化不能使未审配置上线；冲突不留下成功审计或部分生效状态。（对应 AC9、AC10；验证：执行 V_APP、V_HTTP 的交错/故障测试，固定并发顺序后比较 active、client/trust、申请结果和提交记录。）

- [ ] **C19** 提交生产敏感变更后旧 active 仍可用，pending 新回调/范围不能使用；批准切换失败保留旧配置，成功切换后旧代码和旧授权失效，页面及接入摘要只反映真实 active。（对应 AC10；验证：执行 V_APP、V_UI、V_SSO，分别使用旧/新回调和范围，并注入 commit 失败；比较后端状态、生成参数与页面差异。）

- [ ] **C20** Web 与 SPA 类型切换经批准后使用新的 client_id，旧客户端停用，SPA 不沿用 Web secret；单环境 secret reset 提交后旧秘密失效、新秘密可用，另一环境不受影响。（对应 AC10、AC18；验证：执行 V_APP、V_PROTOCOL，测试类型切换前后与独立 reset，检查一次性响应、后续不可读和无权请求状态不变。）

- [ ] **C21** 应用停用使两环境旧代码、刷新和身份授权失效；恢复要求有效负责人和已批准配置，且不使任何旧凭据恢复；中心根仍能用于其他获准应用。（对应 AC10、AC19；验证：执行 V_APP、V_SESSION、V_SSO，测试停用/恢复与并发签发，验证另一应用和重新认证后的新授权。）

- [ ] **C22** 密钥/reader 的读取或重置严格按应用、环境、管理资格及版本授权，首次成功响应之外没有明文恢复能力；离页或维护权被撤回后页面不再保留曾显示的秘密。（对应 AC18、AC24；验证：执行 V_APP、V_HTTP、V_UI，检查普通用户/非成员 reset 的拒绝、同键重试、结果查询及权限撤回后的页面缓存。）

- [ ] **C23** SPA 保护 API 的 reader 仅有批准环境/audience 的内省查询权，秘密只保存在 API 服务端；不得用 reader 做 token 认证、跨环境枚举、业务操作或 MCP 创建。（对应 AC8、AC17、AC24；验证：执行 V_PROTOCOL、V_SPA，直接尝试 reader 的不同用途；审查 public 构建参数/产物不含 reader 秘密，合法内省只返回绑定目标。）

### 访问分配与身份披露

- [ ] **C24** 有效中心认证、开发者/管理员资格、应用维护权及用户同意均不能代替生产访问分配；无明确直接/组分配的用户不能获取该应用代码或令牌。（对应 AC11；验证：执行 V_RULE、V_ASSIGN、V_PROTOCOL、V_SSO，交叉组合身份、资格、成员和同意，确认无分配组合全部拒绝。）

- [ ] **C25** 组获分配后成员可登录；移除成员、取消组分配或删除组时，只有最终失去所有来源的 user/environment 被撤销；保留直接或另一组来源时继续可用。（对应 AC12；验证：执行 V_ASSIGN、V_SSO，测试重叠组与直接来源；测量最终失权后超过 5 秒的 A/B/API 拒绝，其他有分配应用保持可用。）

- [ ] **C26** 重新加入组、重新授予分配或恢复开发资格只允许新授权，不复活失权前 token；待审批范围随组/分配变化失效，不能批准过期快照。（对应 AC12、AC10；验证：执行 V_ASSIGN、V_APP、V_SESSION，保留旧 token 重试、请求新授权并复核 pending 状态。）

- [ ] **C27** 内部应用经管理员企业预授权且用户已认证/已分配时自动登录；未预授权第三方首次访问显示真实身份范围并要求同意，拒绝返回 access_denied 且不保存同意。（对应 AC16；验证：执行 V_PROTOCOL、V_UI、V_SSO，分别访问预授权/第三方并接受或拒绝，观察标准回调、同意记录与披露字段。）

- [ ] **C28** 请求范围必须是已批准范围的子集；新版本新增身份范围不能沿用旧或历史 NULL 同意，响应只给 requested、approved、consented 共同允许的实际资料。（对应 AC16、AC17；验证：执行 V_PROTOCOL、V_SSO，改变请求/批准/同意集合，检查 invalid_scope 或重新同意与最小 claim 集合。）

- [ ] **C29** A、B 均以标准 RSA ID Token 获取稳定身份，aud 各为自身 client_id，auth_time 来自认证且 sid 属于各 RP；userinfo 可在没有业务资源角色时使用获准身份 token。（对应 AC17；验证：执行 V_PROTOCOL、V_WEB、V_SPA、V_SSO，由标准客户端/JWKS 校验签名、state/nonce/iss/aud 并比较两 RP 的 sid/auth_time。）

- [ ] **C30** 中心管理 token、另一应用 token、未知但可验签 JWT 或错误 audience 不能被当作当前应用身份/保护 API 凭据；已失效根、分配、配置对应 token 的内省为 active=false。（对应 AC17；验证：执行 V_PROTOCOL、V_WEB、V_SPA，逐类调用 userinfo/introspection/保护操作，记录拒绝或 inactive；依赖故障不能返回允许。）

- [ ] **C31** 撤回个人同意只撤销对应用户/环境授权并通知，其他应用保持有效；企业预授权政策单独展示，撤回不得变成撤销整个用户中心根或隐式分配。（对应 AC12、AC19；验证：执行 V_PROTOCOL、V_SESSION、V_UI、V_SSO，撤回前后比较 A/B 授权、中心 Cookie 和个人同意/预授权页面。）

### 中心会话与标准授权

- [ ] **C32** A 登录后从新标签访问不同站点的 B，无需再次输入密码；中心页面刷新或清除当前标签 access token 后，只要中心 Cookie 有效仍能恢复认证。中心根撤销后这些动作均不能恢复旧身份。（对应 AC13；验证：执行 V_SSO 的跨站、新标签、刷新和撤销恢复场景；核对 site/Cookie/storage 隔离与真实授权导航。）

- [ ] **C33** 生产中心 Cookie 具有 HttpOnly、Secure、SameSite=Lax、Path=/ 且无 Domain；普通会话读取不轮换 Cookie/CSRF、不延长固定默认 12 小时期限或 auth_time。（对应 AC13、AC14；验证：执行 V_SESSION、V_SSO，比较并发 account/session 响应与受控时钟前后会话期限，检查 Set-Cookie 属性及 JavaScript 无法读取根秘密。）

- [ ] **C34** A、B 并发发起授权和多个中心标签正常恢复均能各自完成，不覆盖 flow 或误触发 refresh replay；一个 app 的 flow 完成不消费另一个 app 的证明。（对应 AC14；验证：执行 V_PROTOCOL、V_SESSION、V_SSO，使用 barrier 同时发请求并校验两个标准回调，观察根稳定及两个独立 flow/RP/代码。）

- [ ] **C35** 交互 flow 默认 10 分钟、最多 8 个未完成记录；各自浏览器证明绑定请求，超时、错误证明或重复完成拒绝；URL 仅含 flow_id，完成/过期清对应 Cookie。（对应 AC2、AC14；验证：执行 V_PROTOCOL、V_SESSION、V_SSO，以受控时钟及并发请求验证边界，尝试交换 flow 证明并查看脱敏 URL/Cookie 摘要。）

- [ ] **C36** prompt=login 和不满足 max_age 必须完成该 flow 的新密码认证，不能被另一标签旧根满足；成功重新认证更换中心根并撤销原根关联授权。prompt=consent 确实重新呈现同意。（对应 AC15、AC19；验证：执行 V_PROTOCOL、V_SSO，校验新认证时间、必要页面、旧应用授权失效与新授权成功。）

- [ ] **C37** prompt=none 缺认证、缺同意或无法满足必要交互时返回适用标准错误及原 state，不显示登录/同意页面；none 与其他 prompt 混用被拒绝。（对应 AC15；验证：执行 V_PROTOCOL、V_SSO，由标准客户端检查 login_required、consent_required 等对应错误与导航记录。）

- [ ] **C38** 授权码默认 60 秒、只消费一次且兑换必须匹配 client/回调/S256；两次并发兑换仅一次成功，重放返回 invalid_grant；新代码数据库仅存摘要。（对应 AC14、AC17；验证：执行 V_PROTOCOL，测试过期、错 verifier、跨环境和并发兑换，比较 token 注册及代码摘要样本，确保没有新代码明文副本。）

- [ ] **C39** 签名、code 消费、token 登记和提交失败均不返回可用 token；成功提交后回包丢失不能用已消费 code 重取原 token，客户端重新发起授权。（对应 AC14、AC17；验证：执行 V_PROTOCOL 的逐点故障/丢包测试，失败后比较 code/凭据状态，验证重新授权可用而旧 code invalid_grant。）

- [ ] **C40** 正常 refresh 保留原身份范围、auth_time、RP sid 和业务边界，不延长原根/family 的绝对寿命；旧 refresh 秘密真正重放仍撤销 family，根或准入失效立即拒绝刷新。（对应 AC3、AC14、AC17；验证：执行 V_PROTOCOL、V_SESSION、V_WEB、V_SPA，分别测试串行正常轮换、真重放、失权/根到期；access 默认 5 分钟且 access/refresh 不晚于根到期，不接受 offline_access。）

- [ ] **C41** SPA 的 refresh 秘密仅本标签持有，单标签刷新串行；Web 的同一本地会话刷新互斥。跨站 SSO 通过顶层导航完成，禁用第三方 iframe Cookie 仍可使用中心认证。（对应 AC13、AC14、AC23；验证：执行 V_WEB、V_SPA、V_SSO，两个独立标签和并发请求均可正常登录/刷新；阻断 iframe 与共享存储后仍可完成 A→B 登录。）

### 退出联动与生效时限

- [ ] **C42** A 本地退出只结束 A 本地会话/相应授权，B 和中心根仍有效；当前中心退出只覆盖该根关联应用；全部退出覆盖该用户所有浏览器根，不影响另一用户。（对应 AC19；验证：执行 V_SESSION、V_SSO，以两用户、两根和 A/B 组合分别请求旧 Cookie、refresh 和保护 API，按第 2 节计时验证范围。）

- [ ] **C43** RP 退出使用专用已注册退出 URI，回到正确地址并原样携带 state；GET 确认页不撤销，POST 需正确 flow、来源、Cookie/提示关联和 CSRF 才提交退出。（对应 AC20；验证：执行 V_PROTOCOL、V_SSO，先 GET 后验证原会话仍可用，再 POST 并检查回调/失效；未注册地址或错误证明拒绝。）

- [ ] **C44** 伪造、错 issuer/aud 或指向其他用户/浏览器根的 id_token_hint 不能撤销他人；过期但可关联的 hint 按退出约定确认，无当前中心根时仅安全返回结果。（对应 AC20、AC19；验证：执行 V_PROTOCOL、V_SESSION，保存无关根继续请求并验证仍有效，覆盖有/无 client_id、有/无 Cookie 和过期 hint。）

- [ ] **C45** 后通道 Logout Token 经标准 JWKS 签名验证并检查 iss/aud/iat/jti/sid/sub/events、短时有效期且无 nonce；伪造/目标不符拒绝，重复有效通知幂等清除目标会话和缓存。（对应 AC21；验证：执行 V_PROTOCOL、V_WEB、V_SPA、V_SSO，发送真实签名通知及各类错误载荷，观察接收状态、sid 清理和无关会话可用。）

- [ ] **C46** 通知不可达或接收端失败时中心撤销仍成功；发送不持有治理事务。任务在进程重启、租约过期或多 worker 竞争后可继续，且同一事件/RP 不丢失或重复产生逻辑通知。（对应 AC21、AC22；验证：执行 V_SESSION、V_SSO，断开接收端并重启 worker，观察提交响应、持久通知状态、目标计数和重启后重试。）

- [ ] **C47** 网络失败、429、5xx 依计划退避及限额内 Retry-After 重试，普通 4xx 标永久失败待管理员重试；未许可地址、DNS 解析目标和重定向不能绕过批准范围。（对应 AC21；验证：执行 V_SESSION，以假接收端/解析器与受控时钟覆盖 timeout、5秒/30秒/2分/10分/1小时退避、24 小时截止及手动 retry；明确本地 fixture 的许可与生产策略不同。）

- [ ] **C48** 管理员接入 LocalOnly 第三方时必须明确接受退出限制，界面和文档展示实际能力；通知 2xx 只表示对端接受，失败/仅本地退出不显示第三方全部会话已退出。（对应 AC22、AC24；验证：执行 V_APP、V_UI、V_SSO，尝试未确认启用、成功确认及失败/2xx 投递，比较操作记录、提示与接入文档。）

- [ ] **C49** 参考保护 API 的正向状态缓存默认最多 2 秒，从内省开始的单调时刻计龄，总年龄不超过 5 秒且受 token 到期限制；读取或转发不重置 TTL，慢查询不得生成新的过龄允许。（对应 AC21、AC27；验证：执行 V_WEB、V_SPA 的可控时钟测试，在内省期间耗尽期限并连续读取/转发，检查过龄结果不缓存且保护写入前重检。）

- [ ] **C50** 以中心失效提交完成为边界，在超过 5 秒后对 A 和 B/API 的真实保护操作均拒绝；通知不可达时结论仍成立。内省/主库/可信时钟不可用且缓存过期后不允许操作。（对应 AC3、AC12、AC19、AC21、AC27；验证：执行 V_SSO，逐种失效动作记录响应时刻与单调差值，并验证 HTTP 拒绝且保护副作用未发生；其他有分配应用/用户作为可用对照。）

### 事务、接口与前端集成

- [ ] **C51** 业务资格、准入、版本与范围的核心决策可脱离数据库/HTTP 独立验证，成功/拒绝/边界路径都有实际执行的规则测试。（对应 AC27；验证：执行 V_RULE，记录覆盖的资格、owner、自批、范围、epoch/generation、URI、prompt/time 决策和用例数量；零个测试不算通过。）

- [ ] **C52** 并发签发/刷新与停用、失权、审批切换使用同一个合法生效边界；旧 guard/主库快照不能在失效提交后签发旧权限凭据，结果与提交先后顺序一致。（对应 AC9、AC12、AC14、AC27；验证：执行 V_APP、V_ASSIGN、V_SESSION、V_PROTOCOL 的 barrier 交错测试，比较响应、凭据有效性和提交记录，确认重检后的唯一合法结果。）

- [ ] **C53** 注册、邀请/重置/flow/code 消费、批准、密钥重置和组变更在确定失败时整体回滚；超原 2 秒预算失败关闭，最多三次整事务重试不重置期限。提交结果不确定时不得冒报成功，管理操作查安全回执，已消费代码不得重放原 token。（对应 AC7、AC9、AC12、AC14、AC27；验证：执行 V_ACCOUNT、V_APP、V_ASSIGN、V_SESSION、V_PROTOCOL，以受控时钟/失败点耗尽预算；确定回滚无状态残留，不确定提交只能呈现完整提交或未提交，查询回执可核对，不出现半成功。）

- [ ] **C54** 大组移除/删除超过预算时保留原组、分配与授权状态，返回明确可重试失败；成功变更同步阻止旧准入，不等待后台逐条撤销或通知完成。（对应 AC12、AC27；验证：执行 V_ASSIGN、V_SESSION，对同一隔离组分别制造超时和正常提交，立即测试授权/内省并核对通知独立运行。）

- [ ] **C55** 管理成功事件与实际状态同一次提交；拒绝事件可检索但不伪装成功。注册、审批、分配、凭据重置、停用/退出均可追溯操作者、对象、原因和 request_id。（对应 AC24、AC27；验证：执行 V_HTTP，注入 commit 失败后检索 audit，并比较真实状态/通知；使用带敏感值的请求检查事件和错误输出只留脱敏摘要。）

- [ ] **C56** 未认证为 401，已认证无资格/成员/操作权限为 403；错误格式、资源不存在、revision/幂等冲突返回约定错误且无越权变更。前端 401 清认证，403 保留认证并说明缺权限。（对应 AC24、AC27；验证：执行 V_HTTP、V_UI，逐类直接请求和界面操作，观察结果/状态/草稿/缓存；不以导航显隐代替后端授权。）

- [ ] **C57** 中心、匿名登录和旧 refresh 的 CSRF 证据不能互换；错误 Origin、缺/错误证明均拒绝写入。只有精确批准 SPA origin 可访问必要协议 CORS，中心 Cookie 与内省查询不被跨域开放。（对应 AC24、AC27；验证：执行 V_SESSION、V_PROTOCOL、V_HTTP、V_UI，替换 Origin/证明并比较拒绝和 Cookie 稳定性；检查 no-store、no-referrer、嵌入限制及必要 CSP。）

- [ ] **C58** 清理过期 flow/action 与可过期操作摘要后仍能识别真正 refresh 重放；活跃 family/后代所需消费证据、根/RP tombstone 和审计未被提前清除。（对应 AC14、AC21、AC27；验证：执行 V_SESSION，在受控时钟清理前后重放历史 refresh、读取活跃后代和审计，验证拒绝规则及保留期边界。）

- [ ] **C59** 账户、开发者、管理员三个入口的功能真实可调用；入口或维护权撤回后清相关缓存/秘密。active/pending/rejected/obsolete/suspended 状态与后端一致，失败不提前显示上线/退出成功。（对应 AC24、AC27；验证：执行 V_HTTP、V_UI、V_SSO，经本人/开发者/管理员真实调用资料、密码、应用、成员、审批、分配、会话与审计功能并交叉检查状态。）

- [ ] **C60** discovery 中全部宣告端点和能力有真实可调用实现，Web basic/post、SPA none 认证按类型生效；非法 client/回调不跳转陌生地址，标准拒绝只有验证回调后带原 state。（对应 AC17、AC23、AC27；验证：执行 V_PROTOCOL、V_WEB、V_SPA、V_SSO，逐项调用 discovery/JWKS/authorize/token/userinfo/introspection/revoke/logout，检查认证冲突和未实现能力未宣告。）

### 历史升级与运维连续性

- [ ] **C61** 新数据库默认受控开户/应用准入，没有默认全员分配或新增开发者资格；固定 issuer 不因 Host/重启改变。生产 HTTP 或缺持久 RSA key 启动失败，已有 key 不静默替换。（对应 AC25、AC27；验证：执行 V_MIGRATION，启动全新库、重复进程和配置异常变体，比较策略、主体命名空间和 JWKS；显式 loopback 开发模式单独验证。）

- [ ] **C62** 旧库升级保留原用户/角色/密码散列、主体、应用 key、client ID/回调/secret hash、签名/JWKS 与历史记录；历史应用和孤立 client 均有清晰 legacy 状态，无自动扩大信任或访问。（对应 AC25；验证：执行 V_MIGRATION，使用旧字段形状样本逐项比对，重复迁移/启动后样本和记录数量稳定，旧主体不因名称变化重建。）

- [ ] **C63** 不可靠 owner/应用关联或不同 issuer 列为可解释冲突，未解冲突不能纳管；管理员明确选择已有 client 及环境后，不凭相似名称猜测或重复创建原客户端。（对应 AC25；验证：执行 V_MIGRATION、V_APP，查看冲突/候选列表、尝试未解纳管并核对状态不变，再明确解决并成功纳管。）

- [ ] **C64** 显式纳管复用选定原 client_id/key，另一个环境只经明确管理操作补建；纳管后旧代码、授权和旧同意失效，新登录遵循批准/分配/范围规则，其他未纳管应用保留原边界。（对应 AC25、AC26；验证：执行 V_MIGRATION、V_PROTOCOL，比较标识并使用旧/新凭据实测；客户端不能通过参数或普通管理更新降回 legacy。）

- [ ] **C65** 旧 OAuth 新建/编辑、账户删改/角色赋权、注册、目录和 kickout/revoke 不能绕过资格、owner、审批、隐私及新根有效性；bootstrap 重启不追加 managed 回调或创建新不受控客户端。（对应 AC26、AC5；验证：执行 V_MIGRATION、V_HTTP，逐一走旧入口尝试敏感变更/越权，重启后对比 callback、信任和访问状态；合法旧管理调用保持原授权范围。）

- [ ] **C66** 升级说明可在隔离旧库完成备份、预演、维护窗口迁移、启动、主体/JWKS 核对和失败恢复；开放新治理写入后不直接运行忽略 managed 的旧二进制，数据恢复影响在文档中明确。（对应 AC25、AC27；验证：按运维指南在隔离 fixture 演练并记录备份/样本恢复结果，运行 V_MIGRATION；核对新写入前后两种恢复边界与通知/issuer/key 故障诊断。）

- [ ] **C67** 既有合法管理 bearer/refresh、MCP 与通用权限用例保持原有效边界；新身份用户/应用不自动获得机器凭据、MCP 或业务许可，既有真 refresh replay 与父链撤销仍生效。（对应 AC26、AC27；验证：执行 V_REGRESSION、V_SESSION、V_PROTOCOL，对照既有合法调用和无权新身份的实际结果，不以历史通过记录代替本轮执行。）

### 真实端到端路径

- [x] **C68** E1：通过真实初始化/管理员登录建立第二名管理员、用户组、企业用户、两名开发者和合作方邀请；合作方未自动进入员工组或得到全目录/生产访问，普通账户只见获准入口。（对应 AC1、AC4、AC5、AC24、AC27；验证：执行 V_SSO，使用隔离库和真实账户 API，不预置已认证浏览器状态；在 Chromium、Firefox、WebKit 各独立 context 重复，并保存脱敏操作结果。）

- [x] **C69** E2：开发者通过控制台一次注册 Web A 与 SPA B，维护者协作、负责人交接和指定测试用户登录可完成；两示例按生成配置启动，不直接读取中心数据库或共享会话存储。（对应 AC5、AC6、AC7、AC8、AC18、AC23；验证：按接入说明执行 V_SSO 的开发接入路径，记录两个不同 client/site、Web secret 一次性交付、SPA 无 secret 与 reader 服务端配置。）

- [ ] **C70** E3：提交 A/B 生产配置，由另一管理员审核并给用户组分配；旧 active 与 pending 显示正确，第三方同意和 LocalOnly 退出限制按批准策略执行，自批/无分配不能上线或登录。（对应 AC9、AC10、AC11、AC16、AC22、AC24；验证：执行 V_SSO，所有批准/分配经真实 API/界面；比较申请/active 状态、首访交互、拒绝与审计，三引擎各自完成。）

- [ ] **C71** E4：用户从 A 发起授权、接受邀请并完成标准回调，再在不同站点的新标签进入 B；两应用映射同一身份而使用独立 token/sid，刷新/并发/清标签 token 后恢复及 prompt/max_age/none 均符合要求。（对应 AC2、AC4、AC13、AC14、AC15、AC17、AC23；验证：执行 V_SSO 的完整登录路径；两个标准客户端验证 state/nonce/issuer/audience；三引擎禁用共享 storage/iframe 依赖仍完成跨站 SSO。）

- [ ] **C72** E5：无分配、未批准生产、错误回调、未批准范围、无开发者资格、非成员、跨环境秘密和旧入口绕过尝试全部拒绝；本人合法访问可作为同时可用对照。（对应 AC5、AC8、AC9、AC11、AC16、AC17、AC24、AC26；验证：执行 V_SSO 的准入拒绝路径，检查拒绝后没有 token/身份泄露/状态变更，比较直接 API 和页面结果，三引擎重复。）

- [ ] **C73** E6：依次验证本地/当前中心/全部退出、组成员移除、同意撤回、密码修改/重置、账户停用/恢复及配置切换；通知不可达、重复、重启和内省故障时，两个保护 API 的超时拒绝与无关会话保留仍正确。（对应 AC3、AC10、AC12、AC19、AC20、AC21、AC22；验证：执行 V_SSO 的退出路径，逐动作保存提交边界、单调计时、保护操作拒绝及无副作用、通知/audit 摘要；每引擎覆盖通知故障与过期缓存。）

- [ ] **C74** E7：仅按公开接入及运维说明，在新的隔离实例启动 A/B，完成 SSO/退出，再对旧数据预演升级和纳管；样本主体/client/key 与既有业务边界稳定。（对应 AC23、AC25、AC26、AC27；验证：从文档操作重建 fixture，执行 V_SSO、V_MIGRATION、V_REGRESSION；记录环境/步骤/比对结果，说明无需中心内部库/DB 或真实邮件/第三方账号。）

### 编译、测试与交付检查

- [x] **C75** Node/前端/示例依赖使用锁定版本可重现安装，没有隐式 lockfile 改写或未记录的依赖差异。（对应 AC27；验证：运行 `pnpm install --frozen-lockfile`，命令成功且 lockfile 不变；记录实际 Node/pnpm/标准客户端版本。）

- [x] **C76** 中心前端单元/组件测试、类型检查、lint 和构建全部通过，恢复/入口/flow/权限/状态用例实际执行。（对应 AC27；验证：运行 `pnpm --filter @cobotweb/cobotusr test`、`pnpm --filter @cobotweb/cobotusr check`、`pnpm --filter @cobotweb/cobotusr lint`、`pnpm build:cobotusr`，均成功并记录用例数。）

- [x] **C77** Web 示例类型、构建和协议/会话/退出/保护缓存测试通过，不依赖内部 auth 或中心数据库。（对应 AC23、AC27；验证：执行 V_WEB，并运行 `pnpm --filter @cobotrix/oidc-web check`、`pnpm --filter @cobotrix/oidc-web build`，各命令成功，标准兼容由真实 SSO 补证。）

- [x] **C78** SPA/保护 API 类型、构建和标签隔离/协议/通知/缓存测试通过；public 产物不含 Web/reader/client secret。（对应 AC8、AC23、AC27；验证：执行 V_SPA，并运行 `pnpm --filter @cobotrix/oidc-spa check`、`pnpm --filter @cobotrix/oidc-spa build`，检查产物及命令结果。）

- [x] **C79** Rust workspace 格式检查通过。（对应 AC27；验证：运行 `cargo fmt --all -- --check`，返回成功且无格式差异。）

- [x] **C80** Rust 所有目标和全部 feature 的静态检查无警告。（对应 AC27；验证：运行 `cargo clippy --workspace --all-targets --all-features -- -D warnings`，返回成功。）

- [x] **C81** Rust 所有目标/feature 的发布构建成功，并保持当前独立部署形态。（对应 AC27；验证：运行 `cargo build --workspace --all-targets --all-features --release`，构建成功；结合 CI 三平台构建记录核对平台范围。）

- [x] **C82** 全 workspace 测试实际通过，包含本轮规则、隔离 Pg/HTTP、并发失败注入、迁移与注册浏览器用例；未执行项不能填通过。（对应 AC27；验证：运行 `cargo test --workspace --all-targets --all-features -- --test-threads=1`，记录全部通过/失败/忽略数量和执行范围，新增筛选结果不能为零。）

- [ ] **C83** 文档测试全部通过，接入/升级说明能按步骤复现。（对应 AC27；验证：运行 `cargo test --workspace --doc --all-features`，返回成功，并附 E7 文档复现证据。）

- [x] **C84** Rust 依赖使用检查实际执行并完成未使用依赖处理。（对应 AC27；验证：运行 `cargo machete`，记录工具版本、输出和处理结果，工具未安装不能记为通过。）

- [x] **C85** Rust 锁定依赖审计实际执行，检查发现和处理结果可追溯。（对应 AC27；验证：运行 `cargo audit`，记录工具/数据库时点、发现及处理；未解决导致门禁失败或检查不可用时保持未通过/未验证。）

- [ ] **C86** 专用真实 SSO 命令实际启动 OP、A、B/API 并跑 Chromium、Firefox、WebKit 的 E1—E6 路径，不用 mock UI 成功代替标准协议兼容；E7 由文档复现与迁移/回归结果补证。（对应 AC13、AC14、AC19、AC23、AC27；验证：执行 V_SSO，报告列明每个引擎和场景的真实结果；若全 workspace 已包含同批完整场景且源码未变更，复用该证据不重复跑。）

- [ ] **C87** CI 保留 macOS/Windows/Ubuntu 构建，Ubuntu 有隔离 PostgreSQL 和三引擎 SSO/前端/示例检查；资源缺失或测试 runner 未执行不能产生成功结果。（对应 AC27；验证：检查本轮 CI job 实际输出；注入缺 Docker/浏览器/runner 的 fixture 启动失败用例，观察明确失败或未验证结果；未触发 CI 时报告该范围未验证。）

### 证据、隔离与完成判定

- [ ] **C88** 测试只访问可销毁的 fixture 数据库、通知接收端及随机端口站点，使用临时 RSA/可信时钟/局部 CA；退出后释放资源，没有真实业务 DB/邮件/SaaS 依赖或全局 TLS 校验关闭。（对应 AC27；验证：检查 V_MIGRATION、V_SESSION、V_SSO 的实际连接/站点摘要和关闭结果，两个 fixture 的身份/密钥/存储互相隔离；受信 CA 仅限测试进程。）

- [ ] **C89** 每条验收项都有本轮命令/场景、实际结果和证据编号，源码、OS、依赖/浏览器版本、通过/失败/跳过范围明确；任务仅在相应验证通过后标完成。（对应 AC27；验证：将本文件每个 C 编号及 AC1—AC27 对齐 verification.md 的结果表，核对 task.md 与本轮输出；功能失败/环境缺失保持未通过/未验证。）

- [ ] **C90** 报告、截图、网络/audit/通知摘要均已脱敏，不保留密码、邀请/reset、完整 code/token/Cookie/secret 或原始含密 trace；报告说明实际第三方退出能力、迁移恢复边界和未验证限制。（对应 AC22、AC23、AC27；验证：扫描生成证据并检查脱敏样本及公开接入/运维说明，复核日志产生时即屏蔽敏感值；文档检查或历史记录不冒充本轮功能/性能/认证等级证明。）

## 5. 覆盖映射

### 5.1 验收标准

每条 AC 至少有一项可观测检查；功能条目与真实 E 路径共同证明完整闭环。

| Spec 验收 | 检查项 |
| --- | --- |
| AC1 | C01、C02、C68 |
| AC2 | C02、C03、C04、C35、C71 |
| AC3 | C05、C06、C11、C40、C50、C73 |
| AC4 | C04、C07、C68、C71 |
| AC5 | C08、C09、C65、C68、C69、C72 |
| AC6 | C10、C11、C69 |
| AC7 | C12、C13、C53、C69 |
| AC8 | C14、C15、C16、C23、C69、C72、C78 |
| AC9 | C17、C18、C52、C53、C70、C72 |
| AC10 | C18、C19、C20、C21、C26、C70、C73 |
| AC11 | C16、C24、C70、C72 |
| AC12 | C25、C26、C31、C50、C52、C53、C54、C73 |
| AC13 | C32、C33、C41、C71、C86 |
| AC14 | C33、C34、C35、C38、C39、C40、C41、C52、C53、C58、C71、C86 |
| AC15 | C36、C37、C71 |
| AC16 | C27、C28、C70、C72 |
| AC17 | C07、C15、C23、C28、C29、C30、C38、C39、C40、C60、C71、C72 |
| AC18 | C13、C20、C22、C69 |
| AC19 | C21、C31、C36、C42、C44、C50、C73、C86 |
| AC20 | C43、C44、C73 |
| AC21 | C45、C46、C47、C49、C50、C58、C73 |
| AC22 | C46、C48、C70、C73、C90 |
| AC23 | C41、C60、C69、C71、C74、C77、C78、C86、C90 |
| AC24 | C08、C11、C22、C23、C48、C55、C56、C57、C59、C68、C70、C72 |
| AC25 | C61、C62、C63、C64、C66、C74 |
| AC26 | C64、C65、C67、C72、C74 |
| AC27 | C49、C50、C51、C52、C53、C54、C55、C56、C57、C58、C59、C60、C61、C66、C67、C68、C74、C75、C76、C77、C78、C79、C80、C81、C82、C83、C84、C85、C86、C87、C88、C89、C90 |

### 5.2 功能与非功能需求

| 需求 | 检查项 |
| --- | --- |
| F1 | C01、C02、C03、C04、C05、C06、C07、C11、C35、C40、C50、C68、C71、C73 |
| F2 | C08、C09、C10、C11、C65、C68、C69、C72 |
| F3 | C12、C13、C14、C15、C16、C23、C53、C69、C72、C78 |
| F4 | C17、C18、C19、C20、C21、C26、C52、C53、C70、C72、C73 |
| F5 | C16、C24、C25、C26、C31、C50、C52、C53、C54、C70、C72、C73 |
| F6 | C02、C03、C04、C32、C33、C34、C35、C36、C37、C38、C39、C40、C41、C52、C53、C58、C71、C86 |
| F7 | C04、C07、C15、C16、C23、C24、C27、C28、C29、C30、C38、C39、C40、C60、C68、C70、C71、C72 |
| F8 | C08、C09、C13、C14、C15、C16、C18、C19、C20、C21、C22、C23、C26、C65、C68、C69、C70、C72、C73、C78 |
| F9 | C05、C06、C11、C21、C25、C26、C31、C36、C40、C42、C43、C44、C45、C46、C47、C48、C49、C50、C52、C53、C54、C58、C70、C73、C86、C90 |
| F10 | C41、C60、C69、C71、C74、C77、C78、C86、C90 |
| F11 | C08、C11、C22、C23、C46、C48、C55、C56、C57、C59、C68、C70、C72、C73、C90 |
| F12 | C61、C62、C63、C64、C65、C66、C67、C72、C74 |
| N1 | C05、C06、C08、C10、C11、C16、C21、C23、C24、C25、C26、C27、C28、C29、C30、C31、C40、C42、C44、C50、C51、C52、C56、C59、C64、C65、C67 |
| N2 | C02、C03、C14、C15、C22、C23、C33、C35、C38、C43、C44、C45、C47、C57、C61、C88、C90 |
| N3 | C01、C04、C10、C12、C13、C18、C19、C20、C25、C26、C34、C35、C38、C39、C40、C46、C52、C53、C54、C55、C58、C62、C64 |
| N4 | C14、C15、C23、C27、C28、C29、C30、C36、C37、C38、C39、C40、C43、C44、C45、C57、C60、C69、C71、C77、C78 |
| N5 | C25、C42、C46、C48、C49、C50、C53、C54、C73、C89 |
| N6 | C51、C52、C53、C58、C62、C68、C69、C70、C71、C72、C73、C74、C82、C86、C88、C89 |
| N7 | C61、C62、C63、C64、C65、C66、C67、C74、C75、C76、C77、C78、C79、C80、C81、C82、C83、C84、C85、C86、C87、C89、C90 |

### 5.3 实现任务与集成归属

表中任务范围覆盖 task.md 的 T1—T103；检查项聚焦可观察行为，文件位置调整后仍按相同行为验收。

| 实现任务 | 主要检查项 |
| --- | --- |
| T1 | C68、C88 |
| T2—T7 | C07、C10—C11、C14、C24、C29、C51、C57、C90 |
| T8—T12 | C61—C64 |
| T13—T20 | C12—C13、C34、C38、C46、C52—C55 |
| T21—T26 | C30、C40、C42、C49—C50、C58、C61 |
| T27—T35 | C01—C11、C25—C26、C53 |
| T36—T48 | C12—C31、C52—C55 |
| T49—T60 | C29—C41、C57、C60 |
| T61—T67 | C42—C50、C63—C67 |
| T68—T73 | C08—C09、C16、C24、C55—C60、C65 |
| T74—T85 | C22、C27、C32—C33、C36—C37、C48、C56、C59、C68—C70、C76 |
| T86—T91 | C15、C23、C29—C30、C41、C45、C49—C50、C69、C77—C78 |
| T92—T98 | C52—C54、C61—C65、C68—C73、C86、C88 |
| T99—T103 | C66、C74—C90 |

## 6. 本轮文档核验与后续执行

2026-11-09 已执行文档核验：C01—C90 编号唯一，每项有操作、预期与验证方式；覆盖 AC1—AC27、F1—F12、N1—N7 和 T1—T103；7 条端到端路径及 13 个命令组引用完整，本地文档链接有效，占位/空章节/空白检查通过。全部 90 项仍未执行，文档核验不构成功能通过证据。

清单获确认后，按 task.md 的依赖顺序开发；每项任务先有实际验证结果再记完成。最终以本清单逐项验收，输出本轮 `verification.md`，明确通过、失败、未验证及能力/恢复限制。

## 显式纳管补充检查

- [ ] 历史扫描只返回脱敏关联、固定冲突码和快照摘要，匿名、无权限与精确 deny 均拒绝；扫描不新增信任。
- [ ] 纳管保留选定 client ID、secret hash、已有 application ID、sub 和 JWKS；另环境独立凭据，同键并发只一次秘密，重复只读回执。
- [ ] issuer/多应用/类型不匹配或快照过期不能纳管，提交失败不产生半应用、半撤销或回执；成功纳管旧授权/同意失效。
- [ ] bootstrap 重启不追加任何回调，不创建新 legacy 客户端，不改变已有 public/confidential 类型。
