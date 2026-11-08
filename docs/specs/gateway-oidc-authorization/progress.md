# 实施进度

四份文档已获批准；当前按 task.md 执行。

当前完成27/80；下一任务为T28，T34仍部分完成。接续开发请先读[交接说明与开场提示词](./handoff.md)，验证记录见[evidence.md](./evidence.md)。

| 任务 | 状态 | 证据 |
|---|---|---|
| T01 | 完成 | 见 evidence.md |
| T02 | 完成 | 见 evidence.md |
| T03 | 完成 | 见 evidence.md |
| T04 | 完成 | 见 evidence.md |
| T05 | 完成 | 见 evidence.md |
| T06 | 完成 | 见 evidence.md |
| T07 | 完成 | 见 evidence.md |
| T08 | 完成 | 见 evidence.md |
| T09 | 完成 | 见 evidence.md |
| T10 | 完成 | 见 evidence.md |
| T11 | 完成 | 见 evidence.md |
| T12 | 完成 | 见 evidence.md |
| T13 | 完成 | 见 evidence.md |
| T14 | 完成 | 见 evidence.md |
| T15 | 完成 | 见 evidence.md |
| T16 | 完成 | 见 evidence.md |
| T17 | 完成 | 见 evidence.md |
| T18 | 完成 | 见 evidence.md |
| T19 | 完成 | 见 evidence.md |
| T20 | 完成 | 见 evidence.md |
| T21 | 完成 | 授权码校验后原子消费、登记与提交；16路并发、失败回滚及真实OIDC HTTP验证，见 evidence.md 2026-11-08 |
| T22 | 完成 | 同稳定根原子轮换、配对旧access撤销、非空上界保留；并发和失败回滚验证，见 evidence.md 2026-11-08 |
| T23 | 完成 | 已消费摘要重放撤销对应族及派生凭据；过期祖先、错误客户端、提交失败和双实例竞争验证，见 evidence.md 2026-11-08 |
| T24 | 完成 | 本地登录/注册同事务登记cobotusr UserSession及稳定根，刷新共用原子轮换/重放保护；5组事务/预算/并发及3组真实HTTP测试，见 evidence.md 2026-11-08 T24 |
| T25 | 完成 | 明确撤销作用域、提交后回执、退出/踢出/consent及OIDC撤销迁移、绑定当前refresh的Origin/CSRF校验；9组事务/并发与9组真实HTTP测试，见 evidence.md 2026-11-08 T25；完整链路尚未验收 |
| T26 | 完成 | 登记UserSession个人管理、独立MCP根、管理旧上界与当前目标权限交集、拒绝优先及20根并发配额；8项事务/HTTP专项，见 evidence.md 2026-11-08 T26/T27 |
| T27 | 完成 | 同主库事务锁旧根/凭据并条件撤销消费，仅一次替代、稳定grant及旧上界、显式委托撤销和失败回滚；5项事务/HTTP专项，见 evidence.md 2026-11-08 T26/T27 |
| T28 | 未开始 | 尚未执行 |
| T29 | 未开始 | 尚未执行 |
| T30 | 未开始 | 尚未执行 |
| T31 | 未开始 | 尚未执行 |
| T32 | 未开始 | 尚未执行 |
| T33 | 未开始 | 尚未执行 |
| T34 | 进行中 | T19既有URI登记修复及T26新插入mcp:access的typed Application种子已完成；既有legacy条目迁移、完整操作目录、路由覆盖及应用种子仍待执行 |
| T35 | 未开始 | 尚未执行 |
| T36 | 未开始 | 尚未执行 |
| T37 | 未开始 | 尚未执行 |
| T38 | 未开始 | 尚未执行 |
| T39 | 未开始 | 尚未执行 |
| T40 | 未开始 | 尚未执行 |
| T41 | 未开始 | 尚未执行 |
| T42 | 未开始 | 尚未执行 |
| T43 | 未开始 | 尚未执行 |
| T44 | 未开始 | 尚未执行 |
| T45 | 未开始 | 尚未执行 |
| T46 | 未开始 | 尚未执行 |
| T47 | 未开始 | 尚未执行 |
| T48 | 未开始 | 尚未执行 |
| T49 | 未开始 | 尚未执行 |
| T50 | 未开始 | 尚未执行 |
| T51 | 未开始 | 尚未执行 |
| T52 | 未开始 | 尚未执行 |
| T53 | 未开始 | 尚未执行 |
| T54 | 未开始 | 尚未执行 |
| T55 | 未开始 | 尚未执行 |
| T56 | 未开始 | 尚未执行 |
| T57 | 未开始 | 尚未执行 |
| T58 | 未开始 | 尚未执行 |
| T59 | 未开始 | 尚未执行 |
| T60 | 未开始 | 尚未执行 |
| T61 | 未开始 | 尚未执行 |
| T62 | 未开始 | 尚未执行 |
| T63 | 未开始 | 尚未执行 |
| T64 | 未开始 | 尚未执行 |
| T65 | 未开始 | 尚未执行 |
| T66 | 未开始 | 尚未执行 |
| T67 | 部分完成 | cobotusr CSRF、刷新轮换、会话取消与真实浏览器退出已适配；其余受众/前端仍待后续 |
| T68 | 未开始 | 尚未执行 |
| T69 | 未开始 | 尚未执行 |
| T70 | 部分完成 | cobotusr 时钟健康模板与显式 loopback Cookie 开发模式；三服务完整安全模板未完成 |
| T71 | 部分完成 | cobotusr/cobotrix/cobotnil 注入真实时钟监控；完整云端控制就绪门禁未验收 |
| T72 | 未开始 | 尚未执行 |
| T73 | 未开始 | 尚未执行 |
| T74 | 未开始 | 尚未执行 |
| T75 | 未开始 | 尚未执行 |
| T76 | 未开始 | 尚未执行 |
| T77 | 未开始 | 尚未执行 |
| T78 | 未开始 | 尚未执行 |
| T79 | 未开始 | 尚未执行 |
| T80 | 未开始 | 尚未执行 |

## 2026-11-08 接续边界

OIDC授权码和refresh实际路由已接入主库生命周期；移除了旧的先烧码、明文oauth_token独立签发及非原子刷新辅助方法。客户端服务受众由security_session_client显式登记；ID Token的aud仍为client_id。

本地登录/注册/refresh已接入同一生命周期，固定签发aud/application=cobotusr，使用本地auth时限；管理员创建用户只创建账户。旧refresh JWT即使保留在旧表中也不能继续签发，新refresh仅存摘要，OIDC与本地刷新不能互换。

T25已将本地logout/kickout、consent删除和OIDC revoke/logout接入主库登记及明确作用域。当前会话退出和用户踢出保留独立MCP根；撤销应用授权终止该用户在应用下的全部对应根，并在同一事务取消consent和未兑换授权码。只沿显式revocation_parent_jti传播撤销，minted_by仍只作审计。应用撤销/踢出与新会话根登记共用用户行写屏障，旧一致快照不能在撤销提交后再创建活动根。

Cookie参与的登录/注册/刷新/退出与管理撤销需精确受信Origin和签名CSRF证据；GET /api/v1/csrf提供启动/当前refresh绑定的证据，成功轮换返回新证据。GET /oidc/logout仅确认，POST校验证据、登记会话及redirect后提交，再清Cookie和跳转。cobotusr 前端本地认证适配已完成，其他受众和入口仍待T67等任务。

T26/T27已将MCP新建、轮换、列表、重命名和删除接入本地严格RSA与主库登记的个人管理生命周期，仅接受cobotusr UserSession。新建固定cobotsrv、独立grant，权限取管理旧上界与当前目标权限交集；轮换再与旧MCP上界相交，沿显式委托撤销旧凭据后代，替代凭据保留原grant且无旧jti撤销父。MCP访问凭据以条件设置revoked_at原子消费，consumed_at/replacement_jti仍专用于refresh历史。新根复用用户行写屏障，按活动registry grant限制20根。兼容mcp_token只作元数据及撤销镜像，不升级未知旧记录；新插入的mcp:access种子采用typed Application selector，既有legacy条目未自动升级。

生产宿主必须注入真实时钟健康监控并登记OIDC客户端绑定。普通serve入口没有时钟监控时，新OIDC兑换/刷新、本地签发/刷新、撤销及MCP管理入口失败关闭；不存在默认同步或HMAC access签发回退。普通API当前状态检查的完整接线仍待T32，旧OIDC introspect及MCP服务denylist adapter尚未迁移；当前测试不证明全部入口已拒绝旧access。跨服务十秒撤权、checklist和完整链路仍未验收。下一项从T28机器认证与引导继续。

2026-11-08 本地登录故障修复：三种 IdP 启动方式已注入 NTP 健康监控，默认要求不同 IP 的至少两个可信来源区间一致且误差 <=1s；失败/过期/时间跳变后拒绝认证，库入口没有默认同步回退。普通 NTP 没有传输认证，生产部署应使用受信网络的来源或自有健康 Clock。cobotusr 前端已完成 CSRF 与长 JWT 的当前标签页存储适配，三浏览器隔离实际后端注册、管理员身份恢复、刷新和退出通过；HTTP 放宽需显式 loopback 配置。详见 [认证集成证据](../cobotusr-frontend-redesign/login-security-integration.md)。本轮不改变27/80完成计数，不将T67/T70/T71整体标为完成。
