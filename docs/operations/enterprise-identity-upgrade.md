# 企业统一身份升级、纳管与故障恢复

本文面向部署与运维人员，说明从旧认证中心升级到统一身份与 SSO 提供方的操作顺序、历史数据纳管、回滚边界和常见故障恢复。所有命令在隔离环境先演练，再用于生产。

## 1. 升级前检查

1. 确认目标版本包含全部迁移，且 `sea-orm` 版本未被无关升级改动。
2. 备份数据库、签名私钥、Reader 凭据与当前配置；记录当前 issuer、回调白名单和 CORS 配置。
3. 确认生产 issuer 为 HTTPS，反向代理正确传递 `Host`、`X-Forwarded-Proto` 与客户端地址。
4. 确认系统时钟同步；身份失效、通知时效和 5 秒硬上限都依赖时钟。
5. 在隔离库执行一次完整迁移与回滚演练，记录耗时与锁等待。

## 2. 迁移顺序

1. 停止旧写入或进入维护窗口，避免迁移期间产生新会话与新授权。
2. 执行数据库迁移。迁移只新增结构、索引与受控回填，不隐式创建 legacy 客户端或业务授权。
3. 启动新版本，先检查 `/.well-known/openid-configuration`、`/.well-known/jwks.json` 与健康检查。
4. 用管理员账户登录三个入口，确认账户、开发者、管理控制台可用。
5. 用测试应用完成一次开发环境登录、同意、code 兑换、UserInfo、内省与退出。
6. 逐步恢复流量；观察错误率、内省延迟、后通道投递与审计写入。

## 3. 历史数据扫描与纳管

旧数据不会自动获得新权限。使用管理控制台的迁移页面或接口：

- `GET /admin/identity/migrations`：分页查看候选。
- `POST /admin/identity/migrations/scan`：显式扫描旧应用、客户端、授权与同意。
- `POST /admin/identity/migrations/{id}/adopt`：逐条纳管，写入审计与操作回执。

纳管原则：

1. 先扫描、后纳管；扫描不改变业务状态。
2. 每条纳管都需要管理员显式操作，不使用批量隐式授权。
3. 冲突（同一 client、回调、audience 或主体映射不一致）必须人工确认；不要用“最后写入获胜”。
4. 纳管后重新核对回调白名单、access audience、Reader 与分配；旧 token 不因纳管自动获得新环境权限。
5. 保留扫描与纳管回执，便于审计追溯。

## 4. 回滚边界

- 结构迁移可按迁移名回滚；回滚前确认没有依赖新结构的写入。
- **业务语义不可无损退回旧规则**：新版本创建的账户代次、会话根、授权、同意、审计与 outbox 记录不应被旧版本直接解释。
- 如果必须回退应用版本，优先保持新库结构只读或停写，恢复备份到独立库验证后再切换；不要把新库直接交给旧版本写入。
- 旧 Bearer 管理路径不作为回退方案；它不满足新的同源 Cookie + CSRF 约束。

## 5. 故障恢复

### 5.1 后通道投递失败

1. 在 `/admin/deliveries` 查看失败记录、尝试次数与最后错误。
2. 确认接收端 HTTPS 可达、证书有效、`logout_token` 校验通过。
3. 对通知期限内、未送达且无有效租约的记录执行重试；重试保留原 endpoint、jti 与 attempt_count。
4. 重复通知按 `jti` 幂等；接收方返回成功即停止重试。
5. 通知不可达不阻塞中心失效；中心会话与授权仍按事务提交失效，接收方缓存按 5 秒硬上限失败关闭。

### 5.2 内省不可用

- 保护 API 必须失败关闭，返回 `introspection_unavailable`，不得放行。
- 检查 Reader 凭据、`/oidc/introspect` 可达性、时钟偏移与数据库连接。
- 恢复后无需清空缓存；过期条目会在 2 秒/5 秒窗口后自然失效。

### 5.3 Reader 轮换

1. 在开发者或管理控制台为应用环境创建新 Reader 或重置 secret。
2. 先更新 API 进程配置并重启，确认新凭据内省成功。
3. 再停用旧 Reader；不要先停用再更新，否则保护 API 会失败关闭。

### 5.4 签名密钥轮换

1. 新私钥写入配置并重启，确认 JWKS 发布新公钥。
2. 等待依赖方刷新 JWKS；在旧 token 生命周期内保留旧公钥验证能力（如部署支持多钥）。
3. 旧 token 自然过期后再移除旧钥；不要在旧 token 未过期时强制只保留新钥。

### 5.5 时钟异常

- 时钟回拨会导致有效期、通知时效与缓存年龄判断异常。
- 发现时钟漂移时先停止保护写入或缩短缓存，修复 NTP 后再恢复。
- 不要通过放宽 `clockTolerance` 掩盖持续漂移。

### 5.6 数据库或主库故障

- 中心失效事务失败时不得返回成功；保护 API 不得因缓存未清理而继续放行超过 5 秒的结果。
- 恢复后重新执行必要的退出/撤回操作，并核对审计与 outbox。

## 6. 审计与监控

重点观察：

- 登录、同意、code 兑换、刷新、内省、撤销与退出的失败率。
- 后通道投递成功率、重试次数、租约冲突与过期。
- 保护 API 的 `invalid_token`、`inactive_token`、`wrong_client`、`wrong_subject`、`stale_authorization` 与 `introspection_unavailable` 比例。
- 迁移扫描/纳管的待处理量与冲突数。
- 时钟偏移、数据库连接池与迁移锁等待。

审计事件与 outbox 只反映真实提交结果；不要用页面提示代替事务回执。

## 7. 升级后验证

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --all-targets -- --test-threads=1
pnpm check:examples
pnpm test:examples
pnpm build:examples
```

生产演练还应覆盖：真实 HTTPS 后通道成功投递、跨站 TLS 三浏览器 SSO/退出（T95—T98）、旧库升级与冲突纳管。真实 HTTPS 投递、三引擎 SSO、容量测量与旧库升级/恢复演练均已在测试环境取得证据，逐项状态见 `docs/specs/enterprise-sso-provider/gaps.md`。

## 8. 备份与恢复

### 8.1 备份对象

一次完整备份必须同时包含四类内容，缺一项都会导致恢复后无法登录或无法验证已签发的 token：

1. **数据库**：全部表，重点是 `user`、`identity_subject`、`identity_sso_session`、`identity_oidc_rp_session`、`identity_app_*`、`identity_access_generation`、`identity_audit_event`、`identity_logout_delivery` 与 `seaql_migrations`。
2. **签名私钥**：`oidc.rsa_private_key_path` 指向的文件（默认 `conf/oidc_rsa_private.local.pem`，权限 0600）。恢复时不要重新生成，否则已签发的 ID Token 与 JWKS 全部失效。
3. **配置文件**：issuer、回调白名单、CORS、`[identity]` 策略、数据库连接与时钟源。issuer 变化会让既有主体命名空间失配。
4. **凭据**：Reader secret 与客户端 secret 在库中只存哈希，无法从备份还原明文。恢复后如需继续使用，按 5.3 重新签发；把新 secret 写入受管密钥库，不要写进备份目录。

```bash
# 逻辑备份，保留自定义格式便于选择性恢复
pg_dump --format=custom --no-owner --file=identity-$(date +%Y%m%dT%H%M%S).dump "$DATABASE_URL"

# 私钥与配置单独加密归档，和数据库备份分开存放
tar -czf identity-secrets-$(date +%Y%m%d).tar.gz conf/oidc_rsa_private.local.pem config.toml
```

### 8.2 恢复演练

在隔离环境按顺序执行，每一步都要记录耗时：

1. 新建空库，`pg_restore --no-owner --clean --if-exists --dbname "$RESTORE_URL" identity-*.dump`。
2. 恢复私钥与配置，确认私钥权限为 0600，issuer 与备份时一致。
3. 启动服务，检查 `/.well-known/openid-configuration` 的 issuer、`/.well-known/jwks.json` 的公钥指纹与备份前一致。
4. 用管理员账户登录三个入口，完成一次开发环境登录、code 兑换、UserInfo、内省与退出。
5. 抽查审计与投递：`identity_audit_event` 最新记录时间、`identity_logout_delivery` 的待投递与失败计数。
6. 记录恢复总耗时、迁移锁等待与首次登录延迟，作为 RTO 依据。

### 8.3 恢复后的边界

- 备份时间点之后签发的 token 仍然有效（签名密钥未变），但对应会话与授权可能已不存在；保护 API 会按内省结果拒绝，不需要额外清理。
- 备份时间点之后的审计与投递记录会丢失，恢复后应在运维记录中标注时间窗。
- 不要把新库直接交给旧版本写入，见第 4 节。
- 恢复完成后重新执行一次退出与撤回，确认 outbox 能正常入队与投递。

### 8.4 可复跑的演练与首次记录

仓库提供与第 8.2 节对应的自动演练（`tests/cobotusr/routes/upgrade_drill.rs`，默认 `#[ignore]`）：

```bash
cargo test --release --test mod upgrade_drill -- --ignored --nocapture --test-threads=1
```

它会重建 identity 改造前的旧版 schema、种入旧版账户/应用/客户端，然后依次执行升级前备份、与生产启动一致的迁移、升级后服务与登录验证、升级后备份恢复到新库并二次启动、升级前备份的失败回退验证。2026-11-10 在 macOS 14.6 / Apple M3 Pro / Docker Postgres 上的首次记录：迁移 210.07 ms、升级后就绪 299.29 ms、备份+恢复 96.96 ms + 235.59 ms、恢复库就绪 256.87 ms。数字只证明流程与断言可行，生产 RTO 以目标环境重跑结果为准。

演练同时固化了一条硬约束：`identity_provider` 的 issuer 是固定命名空间。恢复后的部署如果换了 issuer（例如端口变化），`initialize_provider` 会写入 `issuer_mismatch` 冲突并拒绝启动——恢复时必须按第 2 步保持 issuer 与备份时一致。

## 9. 容量规划

### 9.1 已知成本点

| 环节 | 成本特征 | 相关配置 |
| --- | --- | --- |
| 密码校验 | bcrypt cost 12，单次约 200—300 毫秒且占满一个核；并发上限约等于可用核数，登录吞吐约等于核数 ÷ 单次耗时 | 无（固定 cost） |
| 会话与内省 | 保护 API 缓存 2 秒、失败关闭窗口 5 秒，内省 QPS 上限约为活跃会话数 ÷ 2 | — |
| 后通道投递 | 每秒租约 10 条、并发 4，单条受 `logout_timeout_millis` 约束；接收端即时返回时实测 260—420 条/秒、19—30 ms/批 | `logout_timeout_millis`、`logout_retry_seconds` |
| 清理任务 | 每 60 秒一次，按保留期分批删除 | `audit_retention_seconds`、`operation_retention_seconds` |
| 旧全范围治理 | 每个受影响用户约 6 次查询 × 2 轮，整体受 2 秒安全预算约束 | `legacy_governance_max_users` |

### 9.2 配置建议

- `database.max_connections` 至少覆盖“请求并发 + 投递 worker 4 + 清理任务 1”，并留出迁移与运维连接。
- `legacy_governance_max_users` 默认 1000 是保护上限，不是性能承诺。按 9.3 实测单用户耗时后，用 `2 秒 ÷ 单用户耗时` 反推可完成规模；超过该规模的治理操作应改用控制台按应用/环境维度执行。
- `logout_timeout_millis` 越小，投递波次越快，但接收端慢时更容易判超时；企业内网建议 1000—2000。
- 审计保留期直接决定表体积与清理压力；`audit_retention_seconds` 低于合规要求前先确认留存义务。

### 9.3 测量方法

容量数据必须来自目标部署环境，不要用开发机数字外推：

1. 跑仓库自带的探针：`cargo test --release --test mod capacity -- --ignored --nocapture --test-threads=1`，覆盖登录（不同账户与同账户）、刷新轮换、一次性退出、投递积压与清理；需要更长时间时把探针里的 5 秒窗口调大。`--test-threads=1` 不能省，六个探针并行会互相抢占 CPU 与数据库。
2. 用固定并发（如 1/4/8/16）对 `/login`、`/oidc/token`、`/oidc/introspect`、`/oidc/logout` 各压测 60 秒，记录 P50/P95 与错误码分布。
3. 用 `EXPLAIN (ANALYZE, BUFFERS)` 记录全范围治理查询与内省查询的计划，确认走索引。
4. 在投递积压场景（如 1000 条待投递）观察每秒完成数与重试分布。
5. 记录清理任务单次耗时与删除行数，确认不会与业务高峰重叠。

把结果写入部署记录，并在升级或扩容后重测；数值变化超过 30% 时复查索引与连接池配置。

### 9.4 单机基线（参考值，不是承诺）

2026-11-10 在 macOS 14.6、Apple M3 Pro（11 逻辑核）、Postgres 运行于 Docker、服务与压测客户端同机的条件下，用上面的探针取得：

| 路径 | 并发 | 吞吐 | P95 |
| --- | --- | --- | --- |
| 登录（不同账户） | 16 | 26.2 req/s | 761 ms |
| 登录（同一账户） | 4 | 13.4 req/s | 482 ms |
| 刷新轮换 | 8 | 185.8 req/s | 77 ms |
| 退出 | 16 | 181.9 req/s | 131 ms |
| 后通道投递（接收端即时成功） | 4 | 264.0 条/秒 | 30.3 ms/批 |
| 清理（`limit=100`） | 1 | 19641 行/秒 | 9.7 ms/次 |

登录是唯一被 CPU 卡住的路径：bcrypt cost 12 单次约 250 ms，吞吐约等于“可用核数 ÷ 单次耗时”。同一账户的并发登录彼此兼容（发行取共享行锁），但撤销与新登录仍互斥，因此同一账户的并发数不会带来额外吞吐；容量按不同账户的并发估算。刷新、退出、投递与清理都由数据库往返主导，扩连接池与数据库规格比加 CPU 更有效。单机数字抖动较大，同一份代码两次运行可相差 30%—50%，这些值用于判断量级，不作为承诺。
