# 接续交接：网关 OIDC、RBAC/ABAC 与执行授权整改

更新日期：2026-11-08。

## 当前状态

- 本机工作目录为 `/Users/xionghaoyi.binhy/BinhyPC/cobot-for`，分支 `codex/gateway-oidc-authorization`，远端 `git@github.com:Bin-hy/cobot-for.git`。原工作目录 `/Users/binhy/Binhy-Projects/cobotrix` 在本机不存在。
- 四份设计文档已批准。T01–T23完成，当前23/80；T34仍部分完成。下一项T24。不要重新要求设计确认。
- T21–T23生命周期与存储提交为 `73e01d1`。同轮第二组接入实际OIDC路由、时钟注入与HTTP测试并更新文档；以本分支最新git log为准。此前独立OIDC字符串sub修复 `8b01876` 已保留。
- 开始时工作区干净；共享目录仍可能有其他聊天改动，每次先检查git status，保留他人改动及未跟踪文件，仅暂存任务文件。旧交接提到的资产占位文件删除在本次切换后未出现，不应自行恢复或制造这些改动。
- 只有本地提交，没有push、部署或真实设备操作。完整链路尚未验收，checklist保持未验收；不能宣称全部漏洞修复。

## 已实现的生命周期

1. T21：客户端认证、服务端受众绑定、redirect/PKCE/到期校验都在同一主库一致事务内进行；条件消费授权码并登记access/refresh、稳定grant/session/family后提交。16路并发仅一次成功。校验、签名、预算或提交失败不烧码、不遗留替代凭据。
2. T22：锁定稳定grant→session→family→refresh，再原子消费旧refresh、撤销明确配对旧access并登记同根替代。保持原有权限上界，不按刷新时新角色或新资源范围扩大。access到期不妨碍仍有效的refresh；签名和提交失败回滚。
3. T23：保留refresh摘要与消费历史。旧摘要重放先认证客户端并核对issuer/client/根归属，再撤销正确族及显式委托后代；过期祖先仍可保护活动后代。其他会话和仅minted_by关联的凭据不受影响。最多4次主库冲突重试继承原预算，双实例竞争不能恢复活动族；提交失败不回报已持久撤销。
4. 实际OIDC授权码/refresh路由已调用上述生命周期。删除了先烧码与oauth_token明文独立签发/非原子轮换辅助方法。新access采用RS256、at+jwt及严格登记claims；ID Token的sub为字符串，aud保持client_id。
5. 现有明确refresh-token hint的撤销入口兼容了统一配对登记，但完整撤销作用域、无hint、consent及Cookie退出/CSRF仍留在T25及后续。

## 实现入口与组装限制

- `cobotusr/src/service/security/lifecycle.rs`：CodeExchangeRequest、RefreshRequest、SessionTokenSigner及SessionTokenSet；生产RSA签名使用spawn_blocking，其等待由原SecurityBudget约束。
- `cobotusr/src/service/security/repository.rs`：同一主库SecurityTransaction的客户端/绑定读取、授权码CAS、refresh配对消费、族撤销与委托传播。REPEATABLE READ / READ WRITE，没有副本、缓存或另开不受预算限制连接的回退。
- `cobotusr/src/migration/m20261108_000001_security_refresh_pair.rs`：添加security_session_client与凭据oidc_client_id/paired_access_jti，在原操作种子之后注册。旧记录允许NULL，刷新时缺必要事实失败关闭；没有自动补齐旧JWT。
- `security_session_client`按issuer/client_id登记cobotusr或cobotsrv的audience/application，默认停用。不能从client_id猜受众或在refresh时切换旧会话受众。资源元数据缺失/旧glob不会自动升级为权限。
- `cobotusr::serve_with_security_clock`需由宿主注入真实时钟健康监控。旧serve入口没有监控时，新OIDC兑换/刷新返回503；测试中显式健康时钟不代表生产同步监控。配置登记、健康就绪和生产启动接线仍属于T70/T71及迁移演练。
- `cobotusr/src/service/oidc.rs`及`route/oidc.rs`是实际接入点；SecurityError通过AppError保留安全HTTP分类，机密客户端缺secret仍返回400，依赖不可用/超时返回503。
- `tests/cobotusr/routes/security.rs`覆盖事务、并发、回滚、非空g1上界、过期祖先和双实例竞争；`routes/oidc.rs`覆盖真实RSA/JWKS HTTP及缺监控/不健康/HMAC失败关闭。
- 普通API认证/当前凭据检查的完整组装仍待T32等任务；不能从本轮仓储检查推断旧access已被所有业务入口拒绝。旧/enforce缺主体证据返回401，不得恢复自报subject或默认放行。

## 继续顺序

1. T24：本地/login、/register、/refresh统一登记UserSession、稳定根及IdP管理受众；移除旧本地独立签发旁路。
2. T25：单凭据/当前会话/刷新族/应用根的明确撤销作用域，提交后回执；浏览器退出保留独立MCP根，Cookie变更校验CSRF。
3. T26/T27：新建与轮换MCP统一生命周期、独立授权根及权限上界，不把被消费旧凭据当撤销父。
4. 后续按task.md依赖推进，T34仍需完整操作目录/路由覆盖/应用种子。不要将当前默认失败关闭误认为全入口迁移完成。

## 必须保留的安全语义

- 主体按issuer/kind/subject区分；签名证明、当前凭据状态和资源授权是分别必要的步骤。
- 全部匹配显式拒绝先于允许，超级管理员不绕过拒绝；活动根、签发上界、当前权限与系统资源约束同时成立才允许。未登记操作和必要资源事实缺失默认拒绝。
- 原始同步安全预算最多2秒；重试、缓存、转发和排队不得重新计时。查询开始确定最多5秒新鲜期，受父/原期限限制并预留1秒时钟余量。全实例10秒撤权仍待验收。
- grant/session/family是稳定撤销根，minted_by只作审计；显式revocation_parent_jti才传播委托撤销。
- refresh只存摘要并保留消费历史；失败响应不能包含秘密/底层数据库异常。没有真实时钟健康、正确绑定或RSA能力时不回落到旧签发路径。
- 设备/任务所有权、签名许可、执行领取、Unknown和结果身份链仍需后续任务完成。StreamConnect仍失败关闭。

## 实际验证

2026-11-08最终代码实际执行并通过（详细失败到通过记录见evidence.md）：

```text
cargo test --test mod --locked -- --test-threads=4
92 passed; 0 failed; 0 ignored
cargo test -p auth -p cobotusr --lib --locked
auth 45 passed; cobotusr 23 passed
cargo test --test mod oidc --locked -- --test-threads=4
4 passed; 0 failed
cargo clippy -p auth -p cobotusr --lib --tests --locked -- -D warnings
cargo clippy --test mod --locked -- -D warnings
cargo fmt --all -- --check
git diff --check
```

工具链rustc/cargo 1.99.1，隔离Docker PostgreSQL 18-alpine可用，Cargo.lock未修改。Rust 1.99下async_trait重复must_use仅作相关trait/方法局部lint处理。proc-macro-error2 future-incompat提示仍存在。历史auth doc 8项失败/1项ignored尚待T79；完整workspace/all-features、前端、故障矩阵、多实例10秒撤权和checklist未验收。

本轮仓库提交被宿主自动触发神医分析；回传没有具体finding内容，不代表安全或完整分析结束。不要为此重扫、上传或宣称验收通过。

## 可复制开场提示

```text
继续项目 /Users/xionghaoyi.binhy/BinhyPC/cobot-for 的网关OIDC、RBAC/ABAC与执行授权整改，分支 codex/gateway-oidc-authorization。
先读 docs/specs/gateway-oidc-authorization/handoff.md 及同目录 spec.md、plan.md、task.md、checklist.md、progress.md、evidence.md；遵守实际可用的AGENTS.md、mew-spec和rust-dev-workflow。
四份设计已批准，直接继续，不重新确认。T01–T23完成（23/80），T34部分完成；从T24本地登录汇入开始，再推进T25撤销作用域和T26/T27。
先补失败和并发测试，再实现；原始2秒预算、主库一致事务、拒绝优先、权限上界与稳定撤销根语义必须保留。缺生产时钟监控或客户端绑定时当前OIDC路径失败关闭，不添加默认同步或client_id猜受众回退。
检查并保留其他工作区改动；实际验证后更新进度/证据，分组本地提交。完整链路仍未验收，不宣称漏洞全部修复，不自动push、部署或操作真实设备。
```
