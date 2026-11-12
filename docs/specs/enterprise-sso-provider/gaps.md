# 企业统一身份与受控应用接入 未完成缺口

日期：2026-11-11，Asia/Shanghai。

本文件只记录尚未取得证据或尚未实现的部分，作为阶段性提交的交接清单。已完成项与命令结果见 [verification.md](verification.md)，验收状态见 [checklist.md](checklist.md)。当前 90 项验收中 C68—C71、C75—C82、C84、C85 已勾选，其余 76 项未勾选。

## 一、功能缺口

| 编号 | 缺口 | 当前状态 | 影响 | 下一步 |
| --- | --- | --- | --- | --- |
| G1 | ~~真实 HTTPS 后通道投递~~ 已关闭 | `HttpsLogoutTransport` 增加额外信任根与隔离测试地址策略，用局部 CA 起真实 TLS 接收端 | 已取得 HTTPS 成功、超时、5xx、429、4xx、重定向、重复投递与证书不受信的实测证据 | 见“已关闭的缺口” |
| G2 | ~~旧隐藏管理页仍走 legacy Bearer~~ 已关闭 | 旧 RBAC 页面、组件与旧 e2e 套件已删除，旧 URL 改为重定向到新控制台 | 已消除“路由可达但取数失败”的中间态；后端旧接口保留兼容，供既有客户端与 MCP 调用 | 见“已关闭的缺口” |
| G3 | ~~旧全范围治理容量~~ 已关闭 | 上限由 `identity.legacy_governance_max_users` 配置（默认 1000，范围 1—10000），超限在写入前返回 `capacity_exceeded` | 大企业可按实际规模调整；逐用户加载仍受 2 秒安全预算约束，运维指引给出容量建议 | 见“已关闭的缺口” |
| G4 | ~~生产库升级与恢复演练~~ 已关闭 | `tests/cobotusr/routes/upgrade_drill.rs` 用 `#[ignore]` 用例完成闭环：重建 identity 前旧版 schema → 种入旧版账户/应用/客户端 → 升级前 `pg_dump` → 与生产一致的迁移路径升级 → 真实服务启动/登录/读旧账户 → 升级后备份恢复到新库后二次启动（issuer 与 JWKS 模数一致、登录成功、审计行数一致）→ 升级前备份恢复到新库验证失败回退 | 已取得备份、维护窗口迁移、恢复与失败回退的实际记录；单机小数据集耗时见 verification.md，不作为生产 RTO 承诺 | 见“已关闭的缺口” |
| G5 | ~~容量与延迟~~ 已关闭 | 新增六项 `#[ignore]` 探针（登录/同账户登录/刷新/退出/投递积压/清理），并修复探针暴露的同账户并发登录 409：会话发行改用账户行共享锁，撤销保留排他屏障 | 已取得各档并发下的吞吐与 P50/P95，以及投递排空与清理速率 | 见“已关闭的缺口” |
| G6 | ~~运维文档~~ 已关闭 | `docs/operations/enterprise-identity-upgrade.md` 补齐备份与恢复、恢复边界、容量成本点与测量方法 | 运维可按文档执行备份、恢复演练与容量评估 | 见“已关闭的缺口” |

## 二、验收证据缺口

| 编号 | 缺口 | 说明 |
| --- | --- | --- |
| E-A | C01—C67、C72—C74、C83、C86—C90 未勾选 | 其中 C83 只取得 doctest 结果，“接入/升级说明按步骤复现”依赖 E7 |
| E-B | E5—E7 完整流程 | E1 的组织搭建、E2 的开发者接入、E3 的生产治理与 E4 的跨站协议已在三引擎取证并勾选 C68—C71；E5—E7 仍待逐条取证 |
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
| G1 | 后通道投递接入真实 TLS 接收端：`HttpsLogoutTransport` 新增 `with_extra_roots`（额外信任根）与 `with_loopback_fixture`（仅隔离测试放行 loopback，生产仍拒绝）；`allowed_ip` 增加 loopback 开关，默认关闭。测试用 `rcgen` 现场签发 CA 与 `rp.example` 叶证书，`tokio-rustls` 在 127.0.0.1 起 HTTPS 接收端，经 `resolve_to_addrs` 固定解析地址走完整投递路径 | `cargo test --test mod identity_logout_transport` 5 项通过：204 两次投递各产生一条 `logout_token` 表单请求、无信任根时 `network_error`、429+`Retry-After: 30`→`RetryAfter`、503→`Retry http_5xx`、400→`PermanentFailure http_4xx`、302→`PermanentFailure http_redirect`、400ms 延迟对 120ms 超时→`Retry timeout`、非 allowlist 主机解析到私网→`ssrf_blocked`；`cargo test -p cobotusr --lib logout_transport` 3 项通过（含 loopback 默认拒绝、仅测试策略放行） |
| G3 | 全范围治理上限改为部署配置：`identity.legacy_governance_max_users`（默认 1000，校验范围 1—10000）替代硬编码 `LIMIT 1001`，查询按 `上限+1` 取样，超限在事务内、执行变更前返回 `capacity_exceeded`，不留下部分写入；逐用户加载与 2 秒安全预算的关系写入运维指引 | `cargo test --test mod routes::authority` 5 项通过，其中新增用例把上限设为 1 并插入第二个用户，断言 503 `capacity_exceeded` 且 `authority` 表无残留；`cargo test -p cobotusr --lib config_identity` 2 项通过（含 0 与 10001 越界拒绝） |
| G6 | 运维指南补齐第 8 节“备份与恢复”（四类备份对象、`pg_dump`/`pg_restore` 命令、六步恢复演练、恢复后的边界）与第 9 节“容量规划”（成本点表、配置建议、测量方法），并与 G3 的治理上限、G4 的恢复演练互相引用 | 文档评审：备份对象覆盖数据库、私钥、配置与不可还原的 secret；恢复步骤含 JWKS 指纹比对与冒烟；容量表逐项对应真实配置项与代码中的批量/并发参数 |
| G5 | 容量与延迟：`tests/cobotusr/routes/capacity.rs` 提供六项 `#[ignore]` 探针，覆盖登录（不同账户与同账户）、刷新轮换、一次性退出、后通道投递积压与会话清理；探针暴露的同账户并发登录 409 已修复——根因是会话发行与撤销共用排他写屏障，改为发行取 `FOR SHARE`（`serialize_session_mint`）、撤销保留排他屏障（`serialize_user_roots`），bcrypt 校验移出写事务只做一次并在写事务内重新校验密码哈希与启用状态 | `cargo test --release --test mod capacity -- --ignored --nocapture --test-threads=1`：登录 16 并发 26.2 req/s（P95 761 ms）、同账户 16 并发 24.8 req/s（P95 853 ms，全部 200）、刷新 8 并发 185.8 req/s（P95 77 ms）、退出 16 并发 181.9 req/s（P95 131 ms）、投递 264.0 条/秒（30.3 ms/批）、清理 19641 行/秒（9.7 ms/次）；同账户 4 并发修复前 11/36 为 409，修复后 67/67 全部 200。回归用例 `security_t24_concurrent_logins_for_one_account_all_succeed`、`security_t25_application_revocation_prevents_stale_transaction_minting_fresh_roots` 与既有失败语义用例同时通过 |
| G4 | 升级与恢复演练固化为 `tests/cobotusr/routes/upgrade_drill.rs` 的 `#[ignore]` 用例：重建 identity 前旧版 schema、种入旧版账户/应用/客户端、升级前备份、按生产启动同路径迁移、升级后服务与登录验证、升级后备份恢复到新库并二次启动（issuer/JWKS/审计行数一致）、升级前备份恢复验证失败回退；演练还固化了“恢复后的部署必须复用原 issuer，否则 `initialize_provider` 记录 `issuer_mismatch` 并拒绝启动”的约束 | `cargo test --release --test mod upgrade_drill -- --ignored --nocapture --test-threads=1` 1 项通过（4.03 秒）：旧 schema 0.12 s、升级前备份 85.20 ms/38491 字节、迁移 210.07 ms、升级后就绪 299.29 ms、备份+恢复 96.96 ms + 235.59 ms、恢复库就绪 256.87 ms、失败回退恢复 67.70 ms；`cargo clippy --test mod --all-features -- -D warnings` 通过 |
| G2 | 旧 RBAC 管理面整体退役：删除 `pages/{users,roles,authority,permissions,recordings,applications,oauth,authorized-apps}`、`permission-catalog`/`permission-assignment` 组件、`use-permission-assignment` 与旧 e2e 套件；`/users`、`/roles`、`/authority`、`/permissions`、`/recordings` 及 `/legacy/*` 别名改为重定向到 `/admin/accounts`、`/admin/groups`、`/admin/apps`、`/admin/audit` | `pnpm --filter @cobotweb/cobotusr check`、`lint`、`build` 通过；单测 11 个文件 92 项通过，其中 `legacy-routes.test.ts` 断言旧路径只重定向、不再挂载组件 |

