# 鉴权中心：市场方案与 cobotusr 实现对比

调研日期：2026-11-08，Asia/Shanghai。代码基线：`29b62d0`。按项目语境，将用户所说的“健全中心”理解为“鉴权中心”。

本次依据当前源码、现有测试与验收记录，以及直接读取的产品官方文档进行比较。没有重新运行测试、部署外部产品或做性能压测。产品能力表示官方资料提供该能力，不表示所有部署模式、套餐或配置都具备相同效果；本文不比较价格、市场份额或最新版本。

## 用户澄清后的目标：统一身份提供方与 SSO

用户明确：企业各应用的用户认证统一交由本 Provider；用户在中心注册一次，在接入的应用中使用同一身份，减少各业务设计和维护注册、登录的成本。已登录中心且会话有效时，访问其他已接入应用应复用认证状态。这是本项目当前的评价目标。

因此，下文的广义 IAM/权限引擎对比保留为背景，原先提出的 PDP、通用资源和多角色扩展顺序不再作为当前主线。SSO 应优先对标 Keycloak、Casdoor 等身份 Provider；Casbin/OpenFGA 可作为后续授权扩展的参考。

### 按 SSO 目标重新核对现状

| 能力 | 当前实现证据 | 下一步验收目标 |
| --- | --- | --- |
| 中心统一账户 | 本地账户与注册、登录服务已有 | 用户只在中心注册，业务应用按稳定的 issuer/sub 识别同一用户；必要的业务资料自动建立 |
| 应用通过 OIDC 登录 | `/oidc/authorize`、授权码、PKCE、token、userinfo、JWKS 已有 | 两个独立应用分别接入；都无需保存用户密码或开发独立认证表单 |
| 中心登录状态复用 | authorize 跳转中心 consent 页面；前端 guard 根据中心 token 决定是否转 OIDC 登录；见 `cobotusr/src/route/oidc.rs:794`、`cobotweb/apps/cobotusr/src/router/guard.ts:17` | 应用 A 登录完成后访问应用 B，同一浏览器不再输入凭据；会话过期时能正确恢复或重新登录 |
| 已授权应用自动继续 | consent 页面遇到 `already_consented` 会自动提交；见 `cobotweb/apps/cobotusr/src/pages/oidc-consent/index.vue:50` | 已授权应用自动回调；企业自有应用是否可通过管理员策略预授权，应明确设计，当前未发现该策略 |
| 首次访问时注册 | OIDC 模式登录表单用 `v-if="!oidc"` 隐藏注册入口；注册页完成后返回普通 login，未保留 OIDC 上下文 | 在业务应用发起登录后，未注册用户能完成中心注册并返回原应用，保留原授权请求参数 |
| 退出与禁用 | 中心会话撤销、OIDC logout 与应用授权撤销已有 | 分别明确“退出某应用”“退出中心”“退出所有应用”；两个业务应用已有会话的生效时机需端到端验证 |
| 标准客户端兼容 | 当前 AuthorizeQuery 未包含 `prompt`、`max_age` 等字段；授权入口由 SPA 处理中心登录状态 | 按实际客户端需要补齐标准参数语义，使用标准 OIDC 客户端验收，避免每个应用写自定义接入逻辑 |
| 降低接入开发量 | 应用/客户端注册管理已有 | 提供可运行的 Web 后端和 SPA 示例，说明 issuer、client_id、回调、PKCE、身份映射与会话管理 |

本次确认的是实现路径，未运行两个真实业务应用的完整 SSO 测试，因此不将“已有相关代码”表述为“跨应用 SSO 已全部验收通过”。

### 当前建议顺序

1. 以两个独立应用建立真实 SSO 验收：从未注册用户开始，完成注册、应用 A 登录、应用 B 免重复认证，以及中心会话过期的恢复。
2. 补齐 OIDC 场景的注册入口和返回流程，明确企业自有应用的授权确认策略，稳定中心会话体验。
3. 明确应用会话与中心会话的关系，验收退出中心、全局退出、账户禁用与密码修改后的生效行为。
4. 验证标准 OIDC 客户端兼容性，提供接入模板，并完善账户找回、管理员开户与所需 MFA；SAML、LDAP/AD 等根据企业已有应用和身份系统决定。

中心负责认证和提供可信身份；业务应用建立自己的业务会话，并按各自规则决定功能与数据访问。一次中心注册不自动代表获得每个应用的全部业务权限。上述划分能减少重复认证开发，同时保持应用接入边界。

协议参考补充：[6] OpenID Connect Core 1.0 incorporating errata set 2 的简介及 Authentication Request 中 `prompt` 参数。地址：`https://openid.net/specs/openid-connect-core-1_0.html`，2026-11-08 直接读取。

## 原始广义对比结论（作为背景）

cobotusr 已具备独立鉴权中心的核心实现，覆盖本地身份、OIDC、应用与权限管理、会话与撤销、MCP 凭据和结构化策略判断。其实现重点是凭据边界与数据库事务一致性。

与市场方案比较，当前最影响通用性的地方是：内部应用授权服务尚未形成可用的对外 PDP 接入闭环、资源类型仍带有原业务领域约束、用户角色与组织模型较简单。企业身份接入、认证方式、策略表达和运维验证还有进一步扩展空间。这是基于当前代码的架构判断，不是成熟度量化评分。

## 应当比较哪些市场方案

| 类型 | 代表方案 | 官方资料中的实现方向 | 与我们的比较点 |
| --- | --- | --- | --- |
| 完整身份与访问管理平台 | Keycloak | SSO、OIDC/SAML 身份代理、LDAP/AD 联邦、用户/会话管理、细粒度授权 [1] | 身份接入、单点登录、管理能力的完整性 |
| 集成身份与权限管理的平台 | Casdoor | 多组织、OAuth/OIDC/SAML/CAS、MFA/Passkey、SCIM、基于 Casbin 的权限检查 API，以及 MCP 授权 [2] | 对外接入与管理产品的完整性 |
| 嵌入应用的授权库 | Casbin | 模型和策略分离，支持 RBAC、ABAC 等模型；不负责用户名密码认证 [3] | 权限模型灵活性、嵌入与多语言接入 |
| 独立细粒度授权服务 | OpenFGA | 以资源关系为核心的授权模型，支持关系检查、对象查询及条件/上下文建模 [4] | 跨资源关系、授权查询与通用服务契约 |
| 托管身份平台 | Auth0 | OIDC/OAuth，以及授权码、PKCE、客户端凭据、设备授权等流程 [5] | 应用类型与协议接入覆盖 |

这些方案职责不同，不能把 Casbin 或 OpenFGA 当成完整登录中心来逐项计分，也不能只用登录功能衡量细粒度授权引擎。

## 与当前代码逐项比较

| 维度 | 市场参照 | cobotusr 当前证据 | 判断 |
| --- | --- | --- | --- |
| OAuth/OIDC | Keycloak、Casdoor、Auth0 提供标准协议接入 [1][2][5] | discovery、授权码、PKCE、refresh、client_credentials、userinfo、JWKS、introspection、撤销及 logout 均有入口；见 `cobotusr/src/route/oidc.rs:199`、`:230`、`:331` | 主流程覆盖较完整；未找到 OIDC 一致性认证或跨产品互操作验收证据 |
| 企业身份与认证方式 | Keycloak 支持身份代理与 LDAP/AD；Casdoor 提供 MFA、Passkey、同步和 SCIM [1][2] | 当前用户与登录链路以本地账户为主；在路由、服务、实体和配置中未找到相应联邦登录、LDAP/SCIM、TOTP/WebAuthn 的完整实现 | 接入企业已有身份体系时需要补齐 |
| 角色、组织、租户 | Casdoor 有多组织；Casbin 提供角色与角色关系建模 [2][3] | `cobotusr/src/entity/user.rs:25` 为单个 `role_id`；当前应用策略也通过该字段加载角色。应用和 audience 分隔已有实现，未找到完整组织/租户模型 | 应用隔离不能等同于租户隔离；多角色、组织成员关系与租户边界尚需设计 |
| RBAC 与显式拒绝 | Casbin 可配置模型、允许/拒绝与角色关系 [3] | 用户授权、角色授权、结构化资源范围及显式拒绝优先；见 `libs/auth/src/security/policy.rs:824`、`cobotusr/src/service/security/repository.rs:939` | 核心决策语义已有实现；角色继承和策略管理能力仍较有限 |
| ABAC | Casbin 支持属性规则；OpenFGA 有条件与上下文模型 [3][4] | `ResourceConstraint` 仅列出所有者、能力版本、绑定版本；见 `libs/auth/src/security/policy.rs:657`。仓储中对 `task:read` 硬编码所有者条件，其他规则构造主要传空条件 | 有有限属性约束能力，尚未形成通用、可配置的 ABAC 产品 |
| ReBAC 与对象查询 | OpenFGA 使用资源关系建模并提供 Check、List Objects 等接口 [4] | 未找到关系元组存储、关系图推导或通用“列出用户可访问对象”接口 | 文档共享、目录继承、团队关系等场景还需额外实现 |
| 通用资源模型 | Casbin 可自定义请求与模型；OpenFGA 通过模型声明资源关系 [3][4] | `ResourceKind` / `ResourceTarget` 固定列举应用、网关、设备、工具、任务及管理资源；见 `libs/auth/src/security/policy.rs:56`、`:72` | 通用应用键已具备，但接入订单、文档、项目等新资源时仍受固定类型约束 |
| 对外 PDP | Casdoor 有权限检查 API，OpenFGA 提供独立授权查询 [2][4] | `/enforce` 路由已注册，但 `cobotusr/src/service/permission.rs:123` 固定返回 Unauthorized。`AuthorizationService::authorize_application` 存在，搜索到的调用点位于集成测试 | 内部授权能力与可用的外部接入接口之间仍有缺口，应优先补齐 |
| 会话、刷新和撤销 | Keycloak、Casdoor 有会话管理；Auth0 覆盖令牌流程 [1][2][5] | 数据库注册凭据与授权根，刷新轮换、重放后的 family 撤销、并发和提交失败测试已有实现；见 `cobotusr/src/service/security/lifecycle.rs`、`revocation.rs`、`tests/cobotusr/routes/security.rs:2929`、`:3529` | 对生命周期边界投入较细；尚不能据此认定整体安全性高于市场产品 |
| MCP / Agent | Casdoor 官方资料包含 MCP 授权；OpenFGA 文档包含 Agent/MCP 授权建模 [2][4] | MCP 要求注册应用及 audience，检查 `mcp:access`；签发与轮换结合当前权限及原始凭据上限；见 `cobotusr/src/service/security/mcp.rs:123`、`:217`、`:255` | 有针对性的边界控制；“支持 MCP”本身已不足以构成独有差异 |
| 密钥与运维 | Keycloak 提供服务管理文档；Casdoor 文档覆盖部署、证书、会话与监控 [1][2] | RS256、JWKS、客户端 secret 更新、日志脱敏、时钟检查、就绪检查及 CI 已有。当前签名器只持有一组 RSA 密钥，见 `libs/auth/src/token_manager.rs:226` | 未找到完整签名密钥重叠轮换方案；缺少持续性能、灾备和运维演练证据 |

## 两个容易高估的地方

### PDP 接口名称不代表已经完成对外授权

当前 `/enforce` 的请求包含调用方提交的 subject，但这些数据不能作为可信身份凭据，因此实现主动返回 401。这个拒绝行为保留了安全边界。

真正的内部路径是：`AuthenticatedContext` → `AuthorizationService::authorize_application` → 数据库凭据和策略快照 → `evaluate_policy`。代码里已经有这条服务链，但当前路由未把它变成外部资源服务可接入的完整协议。

因此要保留可信主体与资源事实的要求，再完成对外接入，而不是简单恢复“传 user_id 得 allow”的旧接口。README 对 `/enforce` 的介绍也应在该闭环完成前说明实际状态。

### 通用性仍受历史模型影响

业务服务和执行入口已经移除，但共享权限库仍保留固定的网关/设备/任务类型，应用策略还对 `task:read` 加入硬编码所有者条件。这些不表示已删除的业务服务仍在运行，却说明策略模型尚未完全从具体业务中抽离。

如果新增一个“订单系统”，期望只注册 `order:read` 和订单资源就能接入，当前固定资源模型会成为扩展点。建议先明确稳定的应用、资源类型、资源标识与可信属性契约，再决定使用受控类型注册还是其他模型；不要直接把身份和资源验证全部退化为任意字符串。

## 建议的补齐顺序

1. **完成一个真实外部应用的接入闭环。** 注册应用与权限，完成 OIDC 登录，在资源服务验证凭据，再调用可信 PDP，最后验证撤销和权限修改的生效行为。产出可运行示例、错误契约及最小 SDK。
2. **明确资源与角色模型。** 解决新资源类型的接入方式，处理固定业务条件；根据实际应用需求补充用户多角色、角色继承或组织成员关系。多租户要独立定义所有数据与管理权限的边界。
3. **补齐稳定运行的证据。** 明确签名密钥轮换与旧 token 过渡机制，完成容量和延迟测试、数据库故障处理、备份恢复及多实例演练。事务型决策需要实测数据库成本，不能因为使用 Rust 就推断更快。
4. **按目标客户补身份能力。** 面向内部小范围应用，先满足上述接入闭环；面向企业或外部客户，MFA、联邦 OIDC/SAML、账号生命周期同步和租户隔离需要进入上线要求。
5. **按业务复杂度决定策略引擎路线。** 有限 RBAC 与属性边界可以继续维护现有引擎；若出现文档共享、资源继承、团队关系和授权对象查询，再评估 OpenFGA；若属性规则和模型配置快速增长，再评估 Casbin 等引擎。替换时应明确唯一策略来源与拒绝语义。

我的建议是继续验证“小而可靠、应用接入清晰”的独立鉴权中心定位，优先完成上述前三项。只有明确需要企业身份产品的广泛能力时，再比较自研扩展与采用 Keycloak/Casdoor 的总维护成本。

## 验证范围与来源

现有 `docs/specs/auth-center-only/acceptance-report.md` 记录了 2026-11-08 的后端、隔离集成测试、前端与三浏览器验证结果。本次只读取该报告、核对关键源码与测试文件，没有重跑上述检查。测试数量不能直接换算为生产成熟度。

网络搜索工具本轮未返回可读结果，因此改为通过 HTTPS 直接读取以下官方网页。外部能力只采用这些一手资料；404 的 Auth0 Agent 页面未作为证据。

- [1] Keycloak 官方首页，重点读取 Single-Sign On、Identity Brokering、User Federation、Admin Console。地址：`https://www.keycloak.org/`
- [2] Casdoor 官方《What is Casdoor》，重点读取协议、认证、多组织、权限与 AI agents。地址：`https://casdoor.org/docs/overview`
- [3] Casbin 官方《Overview》，重点读取模型、Enforcer、角色关系与认证职责边界。地址：`https://casbin.org/docs/overview`
- [4] OpenFGA 官方《Authorization Concepts》，及该页的建模/API 导航，重点读取身份与授权边界、ReBAC、Conditions、Contextual Tuples 和对象查询入口。地址：`https://openfga.dev/docs/authorization-concepts`
- [5] Auth0 官方《Authentication and Authorization Flows》，重点读取 OIDC/OAuth、PKCE、客户端凭据与设备授权。地址：`https://auth0.com/docs/get-started/authentication-and-authorization-flow`

以上网页均于 2026-11-08 读取，未将产品导航中的条目视为完整实测验证。本文对相对完整性、差异价值和优先级的判断是根据代码与资料作出的分析。
