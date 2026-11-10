# 企业统一身份与受控应用接入 未完成缺口

日期：2026-11-10，Asia/Shanghai。

本文件只记录尚未取得证据或尚未实现的部分，作为阶段性提交的交接清单。已完成项与命令结果见 [verification.md](verification.md)，验收状态见 [checklist.md](checklist.md)。当前 90 项验收中 C75—C82、C84、C85 已勾选，其余 84 项未勾选。

## 一、功能缺口

| 编号 | 缺口 | 当前状态 | 影响 | 下一步 |
| --- | --- | --- | --- | --- |
| G1 | 真实 HTTPS 后通道投递 | `HttpsLogoutTransport` 没有注入自签 CA 的入口，成功投递只有 mock 与明文 HTTP 夹具证据 | 无法证明对真实 HTTPS 接收端的投递、超时与重试行为；C73/C86 的 HTTPS 成功路径缺证 | 给 transport 增加仅测试可用的 CA 注入，用局部 CA 起一个 HTTPS 接收端，覆盖成功、超时、5xx 与重复投递 |
| G2 | ~~旧隐藏管理页仍走 legacy Bearer~~ 已关闭 | 旧 RBAC 页面、组件与旧 e2e 套件已删除，旧 URL 改为重定向到新控制台 | 已消除“路由可达但取数失败”的中间态；后端旧接口保留兼容，供既有客户端与 MCP 调用 | 见“已关闭的缺口” |
| G3 | 旧全范围治理容量 | `legacy_governance.rs` 全范围发现最多 1000 用户（`LIMIT 1001`），逐用户加载 | 超过上限返回 `CapacityExceeded`，大企业库无法一次完成治理 | 改为分页/游标遍历并测量耗时，或把上限变成可配置并给出运维指引 |
| G4 | 生产库升级与恢复演练 | 迁移与回滚只在隔离 fixture 验证 | C66 要求备份、预演、维护窗口迁移、失败恢复的实际记录 | 在隔离旧库按运维指南完整演练一次并记录样本恢复结果 |
| G5 | 容量与延迟 | 未测量 | 没有并发登录、投递积压、会话清理的容量数据 | 用受控脚本测登录/刷新/退出/投递吞吐与 P95 |
| G6 | 运维文档 | 只有 README 的开发与部署段落 | 缺备份恢复、密钥轮换、故障诊断、容量规划章节 | 补 `docs/` 下的运维指南并与 C66 演练互相引用 |

## 二、验收证据缺口

| 编号 | 缺口 | 说明 |
| --- | --- | --- |
| E-A | C01—C74、C83、C86—C90 未勾选 | 其中 C83 只取得 doctest 结果，“接入/升级说明按步骤复现”依赖 E7 |
| E-B | E1—E7 完整流程 | 三引擎用例已覆盖登录、跨客户端 SSO、退出、准入拒绝与后通道投递，但 E1 的组织搭建、E3 的审批与分配、E7 的纯文档复现尚未逐条取证 |
| E-C | C03/C04 邀请开户与 SSO continuation | 需要真实浏览器下的完整邀请→开户→继续授权证据 |
| E-D | C87 CI 三平台 | macOS/Windows/Ubuntu 的实际 job 输出未取得；本地只验证了当前 macOS |
| E-E | C88 资源隔离复核 | 需要逐项核对 fixture 连接、端口、CA 与释放记录 |

## 三、已知设计取舍（不是缺陷）

- 单企业独立部署、固定 issuer，不做多租户与公开应用市场；外部开发者不开放自助注册，接入走企业邀约与审批。
- 不实现 SAML、LDAP、SCIM 与 MFA，按 Spec 留给后续。
- 旧 RBAC 数据与旧客户端保留兼容读取，写入统一经身份治理；旧 `/login` 仍按兼容返回原管理 token set，但中心 UI 不再使用。

## 四、已关闭的缺口

| 编号 | 关闭方式 | 证据 |
| --- | --- | --- |
| G2 | 旧 RBAC 管理面整体退役：删除 `pages/{users,roles,authority,permissions,recordings,applications,oauth,authorized-apps}`、`permission-catalog`/`permission-assignment` 组件、`use-permission-assignment` 与旧 e2e 套件；`/users`、`/roles`、`/authority`、`/permissions`、`/recordings` 及 `/legacy/*` 别名改为重定向到 `/admin/accounts`、`/admin/groups`、`/admin/apps`、`/admin/audit` | `pnpm --filter @cobotweb/cobotusr check`、`lint`、`build` 通过；单测 11 个文件 92 项通过，其中 `legacy-routes.test.ts` 断言旧路径只重定向、不再挂载组件 |

