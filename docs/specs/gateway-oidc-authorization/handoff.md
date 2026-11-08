# 网关 OIDC / RBAC / ABAC 实施交接

## 接续状态

- 工作目录：`/Users/binhy/Binhy-Projects/cobotrix`
- 分支：`codex/gateway-oidc-authorization`
- origin：`git@github.com:Bin-hy/cobot-for.git`
- 四份设计文档已获用户批准，处于开发阶段；不需要重新启动逐段审批。
- T01–T20 完成，累计20/80。T34仅完成T19所需的既有URI方法登记修复，整体仍为进行中。其他任务以progress.md为准。
- 代码和进度已提交：`7cc28ef`（主库凭据状态）、`b774719`（URI与应用权限适配）。`8b01876`是另一组已独立提交的OIDC sub字符串修复，需要保留。
- 未完成全入口迁移或端到端验收，checklist.md没有提前勾选；不应宣称全部漏洞已修复。
- 当前剩余工作区改动属于原有内容：两个`assets/*/.gitkeep`删除和未跟踪`docs/reviews/`。保留，不加入安全整改提交。每次开始仍先检查git status，其他聊天可能继续修改共享工作区。
- 本轮只做本地提交，没有推送或部署。

## 从哪里继续

先阅读本目录的spec.md、plan.md、task.md、checklist.md、progress.md、evidence.md，再按依赖继续：

1. T21：授权码原子消费。先验证client、redirect URI和PKCE，再在事务内条件消费及登记替代凭据。并发只能一次成功；错误verifier不能烧码。
2. T22：普通refresh原子轮换。同一稳定grant/session/family，旧refresh只能消费一次，同时使对应旧access失效；签名或提交失败应回滚。
3. T23：保留已消费refresh摘要，重用时撤销对应族及派生访问凭据，不影响其他会话。
4. 再继续T24及其后任务，不跳过实际验证，不反复要求用户确认已批准的设计。

T21/T22的仓储写入和条件消费接口仍需实现；现有SecurityTransaction主要提供一致快照读取。不能把新生命周期写入放到另一个不受预算约束的连接上，绕过主库事务。

## 实现入口

- `libs/auth/src/security/credential.rs`：严格JWT契约、CurrentCredentialChecker、AuthenticatedContext、DecisionValidity。上下文携带原始SecurityBudget；后续授权只能缩短预算和有效期。
- `libs/auth/src/security/{identity,policy,message,budget,redaction}.rs`：身份命名空间、纯拒绝优先判定、不可变消息摘要、时限和脱敏工具。
- `cobotusr/src/service/security/repository.rs`：SecurityRepository/SecurityTransaction、PgSecurityRepository。REPEATABLE READ / READ WRITE，同一事务读取凭据和根、绑定；检查备用/只读状态，没有副本或缓存回退。
- `cobotusr/src/service/security/lifecycle.rs`：当前凭据与根的校验；status_for_snapshot供同一事务内的授权复用。minted_by只用于审计，revocation_parent_jti才表示委托撤销依赖。
- `cobotusr/src/service/security/authorization.rs`：管理受众的URI授权及业务应用授权，复核登记claims/kid后使用共享纯判定器。
- `cobotusr/src/service/{oidc,user,mcp_token,permission,authority}.rs`及对应route：旧签发/生命周期的实际调用点。OIDC sub已改为字符串，保留该修复，不恢复数字或用户0回退。
- `cobotusr/src/entity/security_*.rs`和`migration/m20261107_00000{1,2}_*.rs`：已建立的凭据、稳定根、刷新族、绑定、许可和审计表；更新模型时检查数据库约束。
- `tests/cobotusr/routes/security.rs`：真实PostgreSQL和真实RS256/JWKS测试；新增I用例继续放这里，使用security_tNN命名并确保非零用例执行。

## 必须保留的安全边界

- 主体按issuer/kind/subject区分；签名证明、当前凭据状态和资源授权是不同步骤。
- 未登记操作默认拒绝；扫描全部匹配显式拒绝，再考虑用户允许、角色允许，超级管理员不绕过拒绝。
- 活动凭据/稳定根、签发范围、当前权限及资源系统约束同时成立才允许。
- 同步安全链共享最多2秒预算；查询开始确定最多5秒的新鲜期，继承父/原期限并预留1秒时钟余量。不能转发或命中缓存后重新计时。全实例10秒撤权仍待后续验收。
- 新访问凭据不为旧JWT补默认用途、受众或权限。StrictAccessState与生产仓储已通过隔离测试，但各服务的生产组装仍需后续任务完成。
- refresh只存摘要，保留消费历史供重放检测。轮换替代保持稳定根，不能把被消费旧refresh当成替代凭据的撤销父。
- 旧`/enforce`缺少主体证据，现明确返回401。不要恢复自报subject、缺资源或未知规则放行；新协议由T28/T29建立、云端调用方由T38迁移。
- 新资源权限使用ResourceSelector JSON。缺失范围、旧glob和分隔符别名不能自动转成全设备；旧字符串权限解析仅为迁移期元数据。
- 普通task:read需要可信owner且属于主体；读取他人任务使用独立操作。机器只用当前服务绑定，不借同名用户角色。
- StreamConnect目前失败关闭，受限浏览器票据尚未接入。日志脱敏工具不等于所有服务日志已迁移，T65仍需完成。
- T34种子按注册顺序位于新安全schema之后；它只补既有URI方法，已有登记与管理员授权不覆盖。仅新建个人服务行补标准角色授权，未给User添加网关执行权限。其完整操作目录和路由覆盖仍待完成。

## 实际验证与未完成门禁

最近通过：

```bash
cargo test --test mod --locked -- --test-threads=4
# 76 passed
cargo test -p auth -p cobotusr --lib --locked
# auth 45 passed；cobotusr 22 passed
cargo clippy -p auth -p cobotusr --lib --tests --locked -- -D warnings
cargo clippy --test mod --locked -- -D warnings
cargo fmt --all -- --check
git diff --check
```

最后增加旧/enforce真实HTTP的401断言后，单独重跑该HTTP测试（1 passed）和集成目标clippy，均成功。auth 45中包含独立OIDC sub提交增加的3项基础库测试。

曾额外运行`cargo test -p auth --doc --locked`，记录了8项原有示例编译失败、1项ignored，尚待T79修正。完整workspace/all-features、前端、跨实例撤权、故障矩阵及checklist未验收。proc-macro-error2还有future-incompat提示，不要把它当成本次测试失败或当作已经修复。

Docker隔离PostgreSQL测试已可运行，Cargo依赖按lock准备完毕。遇到沙箱/Docker访问限制时按工具的权限机制处理，不把环境失败、过滤为0或ignore记为通过。

## 工作方式

使用用户指定的mew-spec和本项目rust-dev-workflow技能。先写负向/并发测试取得失败证据，再实现并验证；每组相关任务更新progress.md/evidence.md并提交。只提交自己的文件，避免共享索引中混入其他任务。不要自动push、部署或向真实设备发指令。

按AGENTS.md优先用CodeGraph回答结构问题；本轮没有可用CodeGraph工具，才采用本地检索，不能据此推断项目未初始化。下一Agent应以其实际工具可用性为准。

## 可复制的开场提示词

```text
继续本项目已批准的“网关OIDC、RBAC/ABAC与执行授权整改”。
工作目录 /Users/binhy/Binhy-Projects/cobotrix，分支 codex/gateway-oidc-authorization。
先读取 docs/specs/gateway-oidc-authorization/handoff.md，以及该目录的 spec.md、plan.md、task.md、checklist.md、progress.md、evidence.md，并遵守 AGENTS.md、mew-spec、rust-dev-workflow。
四份文档已经批准，直接继续开发，不重新索要设计确认。T01–T20完成（20/80），T34仅部分完成；从T21授权码原子消费开始，按依赖继续T22刷新轮换和T23重放保护。
保持主库一致事务、原始2秒预算、不可延期有效期、拒绝优先、签发范围上界及稳定撤销根语义；先补失败和并发用例，验证通过后更新进度、证据并分组提交。
旧/enforce已因缺少主体证据返回401，不得恢复放行。新入口和生产接线尚未全部完成；不能用组件测试代替端到端验收。
先检查git status，保留已有资产占位文件删除、docs/reviews及其他聊天的改动。仅提交本任务文件，不自动push或部署；不要操作真实设备。报告实际测试数量及剩余问题。
```
