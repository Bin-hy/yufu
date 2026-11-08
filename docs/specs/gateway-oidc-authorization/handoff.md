# 接续交接：网关 OIDC、RBAC/ABAC 与执行授权整改

更新日期：2026-11-08。

## 当前状态

- 本机工作目录为 `/Users/xionghaoyi.binhy/BinhyPC/cobot-for`，分支 `codex/gateway-oidc-authorization`，远端 `git@github.com:Bin-hy/cobot-for.git`。原工作目录 `/Users/binhy/Binhy-Projects/cobotrix` 在本机不存在。
- 四份设计文档已批准。T01–T27完成，当前27/80；T34仍部分完成。下一项T28机器认证与引导。不要重新要求设计确认。
- T21–T23提交为 `73e01d1`、`f28fff1`；T24提交为 `19cf7df`、`c4b1a26`；T25提交为 `5b45bbc`、`1fdf2b8`。T26/T27核心提交为 `5eca8fb`，实际HTTP接线/证据为第二组本地提交，身份以本分支最新git log及evidence.md为准。此前独立OIDC字符串sub修复 `8b01876` 已保留。
- 开始时工作区干净；共享目录仍可能有其他聊天改动，每次先检查git status，保留他人改动及未跟踪文件，仅暂存任务文件。旧交接提到的资产占位文件删除在本次切换后未出现，不应自行恢复或制造这些改动。
- 只有本地提交，没有push、部署或真实设备操作。完整链路尚未验收，checklist保持未验收；不能宣称全部漏洞修复。

## 已实现的生命周期

1. T21：客户端认证、服务端受众绑定、redirect/PKCE/到期校验都在同一主库一致事务内进行；条件消费授权码并登记access/refresh、稳定grant/session/family后提交。16路并发仅一次成功。校验、签名、预算或提交失败不烧码、不遗留替代凭据。
2. T22：锁定稳定grant→session→family→refresh，再原子消费旧refresh、撤销明确配对旧access并登记同根替代。保持原有权限上界，不按刷新时新角色或新资源范围扩大。access到期不妨碍仍有效的refresh；签名和提交失败回滚。
3. T23：保留refresh摘要与消费历史。旧摘要重放先认证客户端并核对issuer/client/根归属，再撤销正确族及显式委托后代；过期祖先仍可保护活动后代。其他会话和仅minted_by关联的凭据不受影响。最多4次主库冲突重试继承原预算，双实例竞争不能恢复活动族；提交失败不回报已持久撤销。
4. 实际OIDC授权码/refresh路由已调用上述生命周期。删除了先烧码与oauth_token明文独立签发/非原子轮换辅助方法。新access采用RS256、at+jwt及严格登记claims；ID Token的sub为字符串，aud保持client_id。
5. T25：单凭据/当前会话/刷新族/应用授权根按明确作用域撤销，提交后返回毫秒时间回执及committed_at+10秒的effective_by。会话退出和kickout保留独立MCP；应用撤销包含全部对应根、consent、未兑换授权码及旧oauth_token撤销标记。传播只使用显式revocation_parent_jti。OIDC revoke先认证客户端再查登记，hint仅作提示，无hint/access/refresh均支持，未知值经客户端认证后才可成功无操作；没有旧令牌签发或明文登记回退。
6. T24：本地/login、/register、/refresh接入统一生命周期，固定aud/application=cobotusr、UserSession、稳定grant/session/family。密码处理与RSA签名等待继承同一2秒预算；账户、权限上界与配对凭据同一主库事务，提交后返回。本地auth时限不继承OIDC客户端覆盖值；本地不签发ID Token，OIDC仍签发client_id受众ID Token。管理员创建账户不再顺带签发访问令牌。
7. T25浏览器证据：GET /api/v1/csrf签发启动或当前refresh绑定的csrf+jwt证据，Cookie使用__Host-cobotusr-csrf、Secure、Path=/、SameSite=Lax。登录/注册/刷新/退出校验精确配置Origin和Cookie/Header证据；管理撤销携带Cookie时同样校验。成功签发/轮换返回新csrf_token。GET /oidc/logout只提供带证据的确认表单，不清会话；POST在主库中复核redirect、撤销会话，提交后才清Cookie/浏览器存储和跳转。前端适配尚未执行。
8. T26：MCP新建仅接受当前登记的cobotusr UserSession及具体管理URI权限，固定目标cobotsrv；权限取管理凭据旧上界与当前结构化目标权限交集，必须保有mcp:access。签名、独立grant、registry及兼容元数据同一主库事务，提交后返回秘密。新根复用用户行写屏障，活动registry grant最多20根，旧撤销前快照不能在应用撤销提交后创建活动根。
9. T27：锁旧grant/credential，轮换上界再与旧MCP上界相交；条件设置revoked_at实现一次消费，同事务撤销旧凭据及显式委托后代、登记同grant替代并提交。替代凭据无session/family/旧jti撤销父，minted_by只记录当前管理凭据作审计。签名/提交/原预算失败全部回滚；并发只一次成功。consumed_at/replacement_jti仍只用于refresh消费历史。

## 实现入口与组装限制

- `cobotusr/src/service/security/lifecycle.rs`：CodeExchangeRequest、RefreshRequest、LocalLoginRequest/LocalRegisterRequest、LocalPassword、SessionTokenSigner及SessionTokenSet；生产密码处理和RSA签名使用spawn_blocking，其等待由原SecurityBudget约束。本地refresh与OIDC复用原子轮换，但按明确来源核对配对记录，不能混用。
- `cobotusr/src/service/security/repository.rs`：同一主库SecurityTransaction的客户端/绑定读取、授权码CAS、refresh配对消费、族撤销与委托传播。REPEATABLE READ / READ WRITE，没有副本、缓存或另开不受预算限制连接的回退。
- `service/security/revocation.rs`、`repository_revocation.rs`：明确作用域、主库锁、consent/应用归属解析和提交后回执。应用撤销/踢出与register_session_pair的新根登记共用用户行写屏障，使撤销前的REPEATABLE READ签发快照产生冲突并继承原预算重试；T26新建MCP根已复用这一约束。MCP元数据撤销标记与registry同事务同步。
- `service/security/browser.rs`、`libs/auth/src/security/csrf.rs`：拒绝缺失/null/非受信/重复Origin及重复Cookie/证据头；信任配置Origin和配置issuer的Origin，不信任Host或通配符。CSRF使用配置auth.secret的HS256、专用csrf+jwt用途和当前refresh摘要绑定；access认证仍严格RS256、at+jwt和主库登记。CSRF、Cookie、logout表单及响应已排除通用日志。
- `cobotusr/src/migration/m20261108_000001_security_refresh_pair.rs`：添加security_session_client与凭据oidc_client_id/paired_access_jti，在原操作种子之后注册。旧记录允许NULL，刷新时缺必要事实失败关闭；没有自动补齐旧JWT。
- `security_session_client`按issuer/client_id登记cobotusr或cobotsrv的audience/application，默认停用。不能从client_id猜受众或在refresh时切换旧会话受众。资源元数据缺失/旧glob不会自动升级为权限。
- `cobotusr::serve_with_security_clock`需由宿主注入真实时钟健康监控。旧serve入口没有监控时，新OIDC兑换/刷新和本地登录/注册/刷新返回503；测试中显式健康时钟不代表生产同步监控。配置登记、健康就绪和生产启动接线仍属于T70/T71及迁移演练。
- `cobotusr/src/service/oidc.rs`及`route/oidc.rs`是实际接入点；SecurityError通过AppError保留安全HTTP分类，token接口机密客户端缺secret仍返回400，T25 revoke缺客户端认证返回401，依赖不可用/超时返回503。
- `tests/cobotusr/routes/security.rs`覆盖事务、并发、回滚、非空g1上界、过期祖先和双实例竞争；`routes/oidc.rs`及`routes/auth.rs`覆盖真实RSA/JWKS HTTP及缺监控/不健康/HMAC失败关闭。
- `service/user.rs`汇入本地生命周期，删除旧refresh JWT独立轮换；`AppState::sign_access_token`旧用户签发辅助已移除。`security_credential.oidc_client_id=NULL`在新本地记录中表示本地来源，但仍要求明确配对及完整稳定根，不会信任旧NULL/未登记凭据。空密码和超过72字节的密码拒绝，空格仍合法；不默默截断长密码或自动迁移/重置现存账户。
- 普通API认证/当前凭据检查的完整组装仍待T32等任务；不能从本轮仓储检查推断旧access已被所有业务入口拒绝。旧/enforce缺主体证据返回401，不得恢复自报subject或默认放行。
- kickout和consent DELETE通过get_revocation_routes独立组装，使用LocalStrictAccessState校验配置RSA，再在同一主库撤销事务复核当前UserSession及URI权限，不HTTP回调自身JWKS。仅接受cobotusr受众/应用；同用户凭据所有权、拒绝优先和旧权限上界保持。普通API及旧OIDC introspect仍未完成迁移；当前测试不证明所有业务入口或跨服务十秒撤权通过。
- `service/security/mcp.rs`、`repository_mcp.rs`、`repository_bounds.rs`：MCP新建/轮换/元数据管理及typed上界计算。新UserSession同时携带管理URI及cobotsrv权限上界，refresh保留原上界；重叠deny剔除整个allow selector，deny selector非法时剔除该operation全部allow，deny operation非法时目标应用deny-all，不让后续删deny扩大旧凭据。
- `route/mcp.rs`从旧auth中间件组移入get_revocation_routes中的独立管理组，使用browser::authenticate_manager、本地严格RSA、主库登记及Cookie的精确Origin/真实CSRF，成功响应no-store。已删除AppState::mint_access_token及service/mcp_token旧独立签发/CRUD辅助；MCP服务introspect仍为兼容denylist adapter，未知jti的revoked=false不能证明活动。
- MCP新建只修正新插入mcp:access种子的typed Application selector/action，已有管理员或legacy条目不升级；缺旧目标上界/旧glob的管理session须在明确迁移后重新登录，不能靠refresh扩大权限。T34完整种子与迁移演练仍待后续。测试入口为security.rs注册的security_mcp.rs及真实MCP HTTP测试，原9项HTTP测试全部保留并迁移到健康时钟/RSA/登记session夹具。

## 继续顺序

1. 从T28机器认证与引导继续：先补client_credentials、本地机器状态、应用/用途/受众绑定及错误主体失败测试，再实现；不得递归调用自身PDP或将机器subject回退为用户0。
2. 后续按依赖统一主体证据及专用安全服务入口，不恢复旧用户/MCP签发旁路。
3. 后续按task.md依赖推进，T34仍需完整操作目录/路由覆盖/应用种子；T67等任务需适配当前CSRF/刷新/退出协议。不要将当前默认失败关闭误认为全入口迁移完成。

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
131 passed; 0 failed; 0 ignored
cargo test -p auth -p cobotusr --lib --locked
auth 50 passed; cobotusr 24 passed
cargo test --test mod security_t26 --locked -- --test-threads=4
8 passed; 0 failed
cargo test --test mod security_t27 --locked -- --test-threads=4
5 passed; 0 failed
cargo clippy -p auth -p cobotusr --lib --tests --locked -- -D warnings
cargo clippy --test mod --locked -- -D warnings
cargo fmt --all -- --check
git diff --check
```

工具链rustc/cargo 1.99.1，隔离Docker PostgreSQL 18-alpine可用，Cargo.toml/Cargo.lock未修改。Rust 1.99下async_trait重复must_use仅作相关trait/方法局部lint处理。proc-macro-error2 future-incompat提示仍存在。额外的auth/cobotusr libs all-features Clippy退出101：utoipa-swagger-ui下载v5.17.14.zip后InvalidArchive("Could not find EOCD")，不记为通过；未扩展范围修复该依赖下载。历史auth doc 8项失败/1项ignored、完整workspace/all-features（含OpenAPI）、前端、故障矩阵、多实例10秒撤权和checklist仍待T79/T80等任务。最终回归在非法deny operation失败关闭及元数据归属校验修正后执行，全部131项通过。

宿主可能在本地提交后自动触发神医分析；没有具体finding内容的回传不代表安全或完整分析结束。不要为此主动重扫、上传或宣称验收通过。

## 可复制开场提示

```text
继续项目 /Users/xionghaoyi.binhy/BinhyPC/cobot-for 的网关OIDC、RBAC/ABAC与执行授权整改，分支 codex/gateway-oidc-authorization。
先读 docs/specs/gateway-oidc-authorization/handoff.md 及同目录 spec.md、plan.md、task.md、checklist.md、progress.md、evidence.md；遵守实际可用的AGENTS.md、mew-spec和rust-dev-workflow。
四份设计已批准，直接继续，不重新确认。T01–T27完成（27/80），T34部分完成；从T28机器认证与引导继续。MCP新建/轮换及元数据管理已接入本地严格RSA和主库登记，独立grant及旧上界交集保持；条件revoked_at消费只一次替代，旧jti不是替代凭据撤销父，新根已复用应用撤销/踢出的用户行写屏障。前端CSRF适配、普通API当前状态、旧OIDC introspect及MCP服务denylist adapter仍待后续任务。
先补失败和并发测试，再实现；原始2秒预算、主库一致事务、拒绝优先、权限上界与稳定撤销根语义必须保留。缺生产时钟监控或客户端绑定时当前OIDC路径失败关闭，不添加默认同步或client_id猜受众回退。
检查并保留其他工作区改动；实际验证后更新进度/证据，分组本地提交。完整链路仍未验收，不宣称漏洞全部修复，不自动push、部署或操作真实设备。
```
