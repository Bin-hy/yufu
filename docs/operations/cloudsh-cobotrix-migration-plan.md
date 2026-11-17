# cloudsh / cobotrix.gataccay.com 升级变更方案

面向对象：`VM-0-14-ubuntu`（腾讯云）上的生产 OIDC 服务，从遗留 `cobotsrv` 迁移到本仓库的 `yufu` 认证中心。

本文所有结论来自 2026-11-15 的**隔离演练**：以生产库 `cobotrix` 的逻辑备份为本底，在本地 PostgreSQL 18 容器中完整跑通迁移、启动、验证与回滚点校验。演练未改动生产。

配套文档：`docs/operations/enterprise-identity-upgrade.md`（通用手册）、`docs/specs/auth-center-only/`（范围）。

---

## 1. 演练结论摘要

| 判定项 | 结果 |
| --- | --- |
| 迁移可执行性 | ✅ 16 → 43 条迁移，`up` 全为增量，无 `DROP`（`DROP` 仅在 `down`） |
| 历史数据完整性 | ✅ 15 用户 / 4 客户端 / 561 token / 125 授权码 / 108 recording 全部保留 |
| 签名密钥一致性 | ✅ JWKS modulus 与生产**逐字节相同**，RP 校验不会失效 |
| 遗留凭据兼容 | ✅ 生产 `Bin-hy` bcrypt 密码登录返回 200 |
| OIDC 路由兼容 | ✅ `/oidc/userinfo` 与 `/api/v1/oidc/userinfo` 同时可用 |
| 回滚点有效性 | ✅ 备份可完整还原为 16 迁移 / 13 表的原状 |
| **业务连续性** | ⚠️ **4 个既有客户端在纳管前无法完成新的 authorize 流程** |
| **可用性前置条件** | ⚠️ **`[security_clock]` NTP 门禁失败即全站 503（演练中已真实触发）** |

> 一句话：**结构迁移没有风险，风险全在「时钟门禁」和「客户端纳管」这两件配置/流程上。**

---

## 2. 现状基线

### 2.1 主机与服务

| 项 | 值 |
| --- | --- |
| 主机 | `VM-0-14-ubuntu`，Ubuntu 24.04.4，kernel 6.8，x86_64 |
| 规格 | 4 核 / 3.6 GiB（已用 2.3 GiB，swap 已用 1.5 GiB/1.9 GiB）/ 磁盘 17 GiB 可用 |
| 服务 | `cobotsrv.service` → `/opt/cobotrix/cobotsrv -c /opt/cobotrix/cobotsrv.toml` |
| 二进制 | 2026-04-28 构建，`--version` = `cobotsrv 0.0.0`（遗留构建，无版本语义） |
| 监听 | `0.0.0.0:2644` |
| 入口 | Caddy 2.8 容器（80/443）→ `host.docker.internal:2644` |
| Rust 工具链 | **无** —— 不得在服务器上编译 |
| 数据库 | `postgres://…@localhost:5444/cobotrix`，即 `postgres18` 容器（PG 18），库体 9.6 MB |

Caddy 对 `cobotrix.gataccay.com` 的两条兼容 rewrite（**保持不变**）：

```
rewrite /oidc/oauth/token  /oidc/token
rewrite /oidc/userinfo     /api/v1/oidc/userinfo
```

新版本 `/oidc/oauth/token` 本身返回 404，但 Caddy 在到达后端前已改写为 `/oidc/token`，因此该 rewrite 仍须保留。

### 2.2 生产 OIDC 数据

issuer `https://cobotrix.gataccay.com`；近 7 天签发 26 个 access token，近 30 天 68 个 —— **在用**。

| 客户端 | client_id | 回调 | 签发量 | 最近 |
| --- | --- | --- | --- | --- |
| 叶叶笔记 (Outline) | `ab9dde80-4018-4cc9-bff3-5c1f29941980` | `https://outline.gataccay.com/auth/oidc.callback` | 530 | 2026-11-15 |
| Binrag | `3a0cd7e2-a5d4-4ee6-acad-d1bc7fc68c90` | `http://localhost:8085/api/v1/auth/oidc/cobotrix/callback` | 18 | 2026-08-14 |
| BinIM | `45ef2a0f-4c8c-4615-973b-f2490f42b09d` | `http://127.0.0.1:20800/api/auth/auth_public/v1/oidc/callback` | 10 | 2026-11-08 |
| 小Bytes的博客 (Artalk) | `1de14ede-45cb-4030-ae50-f9a8108cf42f` | `https://artalk.gataccay.com/api/v2/auth/auth0/callback` | 3 | 2026-08-03 |

全部客户端 `allowed_scopes = openid,profile,email`、`grant_types = authorization_code,refresh_token`；`client_secret_hash` 与 15 个用户 `password` 均为 bcrypt `$2b$12$`。

### 2.3 迁移水位

`seaql_migrations` 现有 16 条，最新 `m20260411_000001_rebuild_oauth_client_with_owner`。

---

## 3. 演练实测结果

演练环境：本地 Docker `postgres:18-alpine`（端口 55432）+ `yufu 0.1.1`（`cargo build --release`，macOS 本机构建）+ 生产库逻辑备份。

### 3.1 迁移与启动

| 步骤 | 实测 |
| --- | --- |
| 备份（`pg_dump --format=custom`） | 287 KB |
| 还原到隔离库 | 13 表 / 16 迁移 / 数据完好 |
| 首次启动（含 27 条新迁移 + 建 39 张表） | **约 0.74 s** 到达就绪 |
| 二次启动（无迁移，4 个 NTP 源） | 约 2.2 s（时钟首次采样占大头） |

迁移后：`seaql_migrations` = 43，`information_schema` 表数 13 → 52。

### 3.2 数据完整性（逐项核对，全部相等）

```
users 15 | clients 4 | tokens 561 | refresh 14 | scopes 6
consents 17 | auth codes 125 | recordings 108
```

新增结构对 `"user"` 只做可空/带默认的 `ADD COLUMN`，15 行全部落默认值：

```
enabled=t | person_kind=employee | developer_enabled=f | auth_epoch=0 | identity_revision=0
```

`identity_provider` 初始化成功：`issuer = https://cobotrix.gataccay.com`；`identity_subject` 自动为 15 个用户建了 subject 映射。

### 3.3 签名密钥

演练加载的是生产**实际在用**的私钥，产出 JWKS modulus 与线上 `kid=cobotrix-rsa-1` **完全一致**。既有 RP 的签名校验与会话不受影响。

### 3.4 凭据兼容

- `POST /api/v1/login`，用户名 `Bin-hy` + 生产原始密码 → **HTTP 200**，返回 access/refresh token。
- 生产 `client_secret_hash` 与用户密码同为 `$2b$12$` bcrypt，走同一 `bcrypt::verify` 路径 → 4 个客户端 secret 无需重置。
- 错误 secret 走统一拒绝（400 `invalid authorization code, client_secret, redirect_uri or PKCE`），无 500、无哈希格式报错。

### 3.5 路由与发现文档

| 路径 | 新版本 | 说明 |
| --- | --- | --- |
| `/oidc/authorize` | 存在 | 需完整浏览器流 |
| `/oidc/token` | 400（缺参） | 正常 |
| `/oidc/revoke` `/oidc/introspect` `/oidc/logout` | 存在 | |
| `/oidc/userinfo` | 401（需鉴权） | **新默认路径** |
| `/api/v1/oidc/userinfo` | 401（需鉴权） | **旧路径保留，向后兼容** |
| `/.well-known/*` | 200 | |
| `/api/v1/health/ready` | `{"ready":true,...}` | |

发现文档差异（**cosmetic，不阻断既有集成**）：

- `userinfo_endpoint` 由 `/api/v1/oidc/userinfo` 变为 `/oidc/userinfo`（两条路径都通）。
- `scopes_supported` 由 6 项收敛为 `openid,profile,email`；但 `offline_access` 仍在库中且被 identity 路径使用。
- `code_challenge_methods_supported` 只公示 `S256`；代码仍接受 `plain`（`service/oidc.rs`），既有 `plain` 客户端不受影响。
- `grant_types_supported` 缺少 `client_credentials`（4 个客户端均未使用）。
- 新增 `end_session_endpoint`、`backchannel_logout_supported: true`。

---

## 4. 五个关键风险与对策

### R1（最高）签名私钥被「找错」而不是「找不到」

- 配置使用**相对路径** `conf/oidc_rsa_private.local.pem`，systemd unit **没有 `WorkingDirectory`**，cwd = `/`，因此实际加载 `/conf/oidc_rsa_private.local.pem`。
- 两处各存一把**不同**的密钥（同为 1704 B、同为 2026-04-04）：

  | 路径 | 状态 |
  | --- | --- |
  | `/conf/oidc_rsa_private.local.pem` | **线上在用**（与 JWKS 吻合） |
  | `/opt/cobotrix/conf/oidc_rsa_private.local.pem` | 未被使用 |

- 新版 fail-closed 只防「文件缺失」（`persistent RSA key unavailable`），**不防「文件存在但内容不同」**——会正常启动并静默换钥。
- 两把 key 权限均为 `0644`，新版按 `0600` 管理。

**对策**：迁移时把 `rsa_private_key_path` 改成**绝对路径**并指向 `/conf/oidc_rsa_private.local.pem`；备份两把 key；收敛为 `0600`；**不要**在未核对的情况下给 unit 加 `WorkingDirectory=/opt/cobotrix`。

### R2（最高）`[security_clock]` NTP 门禁是硬可用性依赖

- 参数边界（`yufu/src/clock.rs` 校验）：`min_agreeing_sources ∈ (0, servers.len()]`、`poll_interval_seconds ∈ [1,30]`、`sample_ttl_seconds > poll and ≤ 60`、`timeout_millis ∈ [100,2000]`；误差上限 `MAX_ERROR_MILLIS = 1000`。
- **首次检查失败 → 进程退出；运行中失同步 → 全部认证入口 503**（`dependency_unavailable`）。
- 演练中已真实触发：默认 `poll=10s / ttl=30s` 下采样抖动约 1 分钟后转为 unhealthy，此后每次登录与纳管接口均 503。
- 服务器实测 NTP 可达性：`ntp1.tencent.com` / `time1`,`time2.cloud.tencent.com` / `ntp.aliyun.com` / `ntp1.aliyun.com` / `cn.pool.ntp.org` / `time.cloudflare.com` 均 OK；`ntp.tencent.com` 超时；`time.apple.com` **时通时断**。

**对策**：换用同地域 4 源（`time1.cloud.tencent.com`、`time2.cloud.tencent.com`、`ntp1.tencent.com`、`ntp1.aliyun.com`），`min_agreeing_sources=2`、`poll=30s`、`ttl=60s`、`timeout=2000ms`；上线前连续用 `--check-security-clock` 观测；注意 `poll=10s` 对公共 NTP 有被限流风险。若后续进入企业内网，改为内网 NTP 源。

### R3（高）既有客户端必须先「纳管」才能继续走 authorize

- 新版把客户端纳入「应用 / 环境 / 配置版本」治理模型。演练后 `identity_app_environment = 0`、`identity_app_config_version = 0`。
- 结果：`/oidc/authorize` 对遗留客户端返回 `403 access_denied`（`target.enabled == false`），**纳管前新的登录授权流程不可用**。
- 官方手册第 3 节已明确：旧数据不自动获得新权限，须 `scan → adopt` 逐条纳管。
- 注意：`[[oidc.bootstrap_clients]]` 在当前版本**不会创建客户端**，启动日志仅告警 `Bootstrap entry requires explicit governed application registration`。

**对策**：把 4 个客户端的纳管列为**切换窗口内必做项**（见 §6 阶段 5），并按手册核对回调白名单、audience、Reader 与分配。

### R4（中）管理控制台走的是 SSO 会话，不是遗留登录

- 纳管接口 `GET/POST /api/v1/admin/identity/migrations*` 依赖 `read_sso_cookie`，即身份中心 SSO 会话，由 `POST /api/v1/identity/login` 建立。
- 遗留 `POST /api/v1/login` 只发 access/refresh token，**不能**用于管理操作（实测 `/admin/identity/migrations/scan` 返回 403）。
- SSO 登录要求 CSRF + 浏览器绑定 cookie，绑定由 `/oidc/authorize` 流程签发 —— 即必须通过**嵌入式 WebUI**（浏览器）操作，无法用裸 curl 完成。

**对策**：必须发布带 `--features webui` 的构建产物（`release.yml` 的归档已内嵌 WebUI，`deploy/docker` 镜像同样），用浏览器登录控制台完成纳管。

### R5（中）回滚语义是单向的

- 结构可回滚，但新版写入的会话根/授权/同意/审计/outbox 不应交回旧版解释。
- **回滚 = 还原备份 + 换回旧二进制**；不要用新库直接接旧版写入。
- `identity_provider.issuer` 是固定命名空间，日后变更 issuer 会以 `issuer_mismatch` 拒绝启动。

**对策**：见 §7。

---

## 5. 交付物与安装方式

生产为裸二进制 + systemd，改动面最小的是**沿用 systemd 换二进制**（Caddy 配置与端口 2644 保持不变），不使用容器。

- 产物：`release.yml` 的 `x86_64-unknown-linux-gnu` 归档（已内嵌 WebUI），或本地 `deploy/docker/Dockerfile` 交叉构建。
- 服务器**无 Rust 工具链且内存吃紧**，禁止在服务器编译。
- 新版可执行文件名为 `yufu`，配置文件名习惯 `yufu.toml`。

---

## 6. 生产变更步骤

> 建议窗口：低峰期；预计停机 5—10 分钟（不含纳管操作时间）。

### 阶段 0：预检（不停机）

```bash
# 记录基线
systemctl status cobotsrv.service --no-pager
curl -s https://cobotrix.gataccay.com/.well-known/openid-configuration
curl -s https://cobotrix.gataccay.com/.well-known/jwks.json | sha256sum > /tmp/jwks.before

# 备份数据库
sudo docker exec postgres18 pg_dump --format=custom --no-owner \
  -U xionghaoyi -d cobotrix > /tmp/cobotrix-$(date -u +%Y%m%dT%H%M%SZ).dump

# 备份两把私钥 + 现行配置
sudo tar -czf /tmp/cobotrix-secrets-$(date +%Y%m%d).tar.gz \
  /conf/oidc_rsa_private.local.pem \
  /opt/cobotrix/conf/oidc_rsa_private.local.pem \
  /opt/cobotrix/cobotsrv.toml /opt/cobotrix/cobotsrv
```

备份三项缺一不可：数据库、私钥、配置。私钥丢失即无法验证已签发 token（新版不会自动重新生成）。

### 阶段 1：就绪新版配置（不停机）

以现有 `cobotsrv.toml` 为底，生成 `/opt/cobotrix/yufu.toml`，**必须**包含下列差异：

```toml
[http]
listen = "0.0.0.0:2644"                 # 与现状一致

[auth]
origins = ["http://cobotrix.gataccay.com", "https://cobotrix.gataccay.com"]

[auth.super_admin]
username = "Bin-hy"                     # 沿用

[oidc]
issuer = "https://cobotrix.gataccay.com"          # 不可再变
rsa_private_key_path = "/conf/oidc_rsa_private.local.pem"   # 绝对值：线上在用那把
access_token_expire_seconds = 3600
refresh_token_expire_seconds = 604800
email_domain = "binhy.email"

[database]
url = "<沿用现有连接串>"

[security_clock]                        # 新增，见 R2
servers = ["time1.cloud.tencent.com:123", "time2.cloud.tencent.com:123",
           "ntp1.tencent.com:123", "ntp1.aliyun.com:123"]
min_agreeing_sources = 2
poll_interval_seconds = 30
sample_ttl_seconds = 60
timeout_millis = 2000

[identity]                              # 新增
sso_session_ttl_seconds = 43200
```

上线前必须先验时钟（不碰库、不开 HTTP）：

```bash
sudo /opt/cobotrix/yufu -c /opt/cobotrix/yufu.toml --check-security-clock
# 期望：security_clock: synchronized=true, max_error_millis≈<1000
```

再核对私钥指纹是否与线上 JWKS 一致（**这是 R1 的闸门**）：

```bash
sudo openssl rsa -in /conf/oidc_rsa_private.local.pem -noout -modulus \
  | sed 's/^Modulus=//' | xxd -r -p | base64 | tr '+/' '-_' | tr -d '=' \
  > /tmp/key.modulus
curl -s https://cobotrix.gataccay.com/.well-known/jwks.json \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)["keys"][0]["n"],end="")' > /tmp/jwks.modulus
diff /tmp/key.modulus /tmp/jwks.modulus && echo "KEY MATCHES PRODUCTION"
```

### 阶段 2：停写并切换（停机开始）

```bash
sudo systemctl stop cobotsrv.service
sudo cp /opt/cobotrix/cobotsrv /opt/cobotrix/cobotsrv.rollback-$(date +%Y%m%d)

# 安装新二进制
sudo install -m 0755 yufu /opt/cobotrix/yufu

# 新增 unit，保留旧 unit 不删
sudo tee /etc/systemd/system/yufu.service >/dev/null <<'EOF'
[Unit]
Description = yufu identity center
After = network.target

[Service]
Type = simple
WorkingDirectory = /
ExecStart = /opt/cobotrix/yufu -c /opt/cobotrix/yufu.toml
User = root
Restart = on-failure
RestartSec = 5s

[Install]
WantedBy = multi-user.target
EOF
```

> 若此处给 `WorkingDirectory` 赋别的值，`rsa_private_key_path` 用绝对路径即可免疫（见 R1）。

```bash
sudo systemctl daemon-reload
sudo systemctl start yufu.service
sudo journalctl -u yufu.service -n 50 --no-pager
```

迁移在启动时自动执行（`Migrator::up`），实测约 0.7 s。

### 阶段 3：技术验证（仍停机）

```bash
curl -s http://127.0.0.1:2644/api/v1/health/ready          # {"ready":true,...}
curl -s http://127.0.0.1:2644/.well-known/jwks.json | sha256sum   # 与 /tmp/jwks.before 比对
curl -s http://127.0.0.1:2644/.well-known/openid-configuration   # issuer 不变
sudo docker exec postgres18 psql -U xionghaoyi -d cobotrix -c \
  "select count(*) from seaql_migrations;"                  # 期望 43
sudo docker exec postgres18 psql -U xionghaoyi -d cobotrix -c \
  "select (select count(*) from \"user\") u,(select count(*) from oauth_client) c,(select count(*) from oauth_token) t;"
# 期望 15 / 4 / 561
```

`jwks.json` 与基线不一致 → **立即按 §7 回滚**。

### 阶段 4：恢复流量

```bash
sudo systemctl disable --now cobotsrv.service   # 旧服务不再自启（保留 unit 与二进制）
# Caddy 无需改动：仍反代 2644
curl -s https://cobotrix.gataccay.com/.well-known/openid-configuration
```

### 阶段 5：纳管 4 个客户端（R3，窗口内必做）

用浏览器打开 `https://cobotrix.gataccay.com`，以 `Bin-hy` 登录控制台，然后：

1. `POST /api/v1/admin/identity/migrations/scan` 触发扫描（控制台「迁移」页）。
2. `GET /api/v1/admin/identity/migrations` 逐条检查候选与冲突。
3. 对下列 4 个客户端逐个 `POST /api/v1/admin/identity/migrations/{id}/adopt`：

   - 叶叶笔记 `ab9dde80-…` → `https://outline.gataccay.com/auth/oidc.callback`
   - Binrag `3a0cd7e2-…` → `http://localhost:8085/api/v1/auth/oidc/cobotrix/callback`
   - BinIM `45ef2a0f-…` → `http://127.0.0.1:20800/api/auth/auth_public/v1/oidc/callback`
   - 小Bytes的博客 `1de14ede-…` → `https://artalk.gataccay.com/api/v2/auth/auth0/callback`

4. 纳管后核对：回调白名单、access audience、Reader 凭据、用户分配。
5. 保留扫描与纳管回执。

**验证顺序**：先 Outline（生产在用），确认能完成登录 → 再 Artalk / Binrag / BinIM。

### 阶段 6：观察

盯 `journalctl -u yufu.service`：登录/兑换/刷新/内省/撤销/退出的失败率、后通道投递成功率、`dependency_unavailable`（时钟）比例、`access_denied`（纳管遗漏）比例。

---

## 7. 回滚方案

**触发条件**：JWKS 指纹与基线不一致；连续 503；关键客户端无法纳管或纳管后仍无法登录。

```bash
sudo systemctl stop yufu.service

# 1) 还原数据库到变更前
sudo docker exec -i postgres18 psql -U xionghaoyi -d postgres \
  -c "CREATE DATABASE cobotrix_restore;"
sudo docker exec -i postgres18 pg_restore --no-owner --no-privileges \
  -U xionghaoyi -d cobotrix_restore < /tmp/cobotrix-<ts>.dump
# 校验还原库：16 迁移 / 13 表 / 15 用户 / 4 客户端 / 561 token
# 确认无误后由 DBA 决定是否 drop 现有 cobotrix 并重命名

# 2) 换回旧二进制与旧配置
sudo cp /opt/cobotrix/cobotsrv.rollback-<date> /opt/cobotrix/cobotsrv
sudo systemctl start cobotsrv.service
sudo systemctl enable cobotsrv.service
```

**边界**：

- 不要把已迁移的库直接交给旧版写入 —— 必须走备份还原。
- 还原后私钥不变，token 签名仍然有效；但会话/授权可能已不存在，保护 API 按内省结果拒绝。
- 还原点之后的审计与投递记录会丢失，需在运维记录中标注时间窗。
- 新版写入的 `identity_*` 数据不会回到旧库里（备份还原本身即丢弃）。

回滚点有效性已在演练中验证：备份可完整还原为 16 迁移 / 13 表 / 数据齐全。

---

## 8. 迁移后验收清单

- [ ] `/-.well-known/jwks.json` 指纹与变更前一致
- [ ] `/-.well-known/openid-configuration` 的 `issuer` 不变
- [ ] `seaql_migrations` = 43；`user` 15 / `oauth_client` 4 / `oauth_token` 561
- [ ] `--check-security-clock` 连续 10 次均为 `synchronized=true`
- [ ] 控制台可用 `Bin-hy` 登录（SSO 会话）
- [ ] 4 个客户端扫描无未解释冲突
- [ ] 4 个客户端逐个纳管成功，回执留档
- [ ] Outline 真实登录 → 同意 → code 兑换 → UserInfo → 刷新 → 退出 全通
- [ ] Artalk / Binrag / BinIM 抽查登录
- [ ] 观察 30 分钟无 `dependency_unavailable` 峰值
- [ ] 旧 `cobotsrv.service` 已 disable 但**未删除**，二进制与配置留存

---

## 9. 演练环境复现命令

```bash
# 生产备份
docker exec postgres18 pg_dump --format=custom --no-owner -U xionghaoyi -d cobotrix > cobotrix.dump

# 隔离库
docker run -d --name yufu-drill-pg -e POSTGRES_PASSWORD=drill -e POSTGRES_USER=drill \
  -e POSTGRES_DB=cobotrix -p 55432:5432 postgres:18-alpine
docker exec -i yufu-drill-pg pg_restore --no-owner --no-privileges -U drill -d cobotrix < cobotrix.dump

# 构建（勿在服务器执行）
cargo build --release --bin yufu

# 时钟预检 + 启动
./target/release/yufu -c yufu.drill.toml --check-security-clock
./target/release/yufu -c yufu.drill.toml
```

隔离演练 2026-11-15 记录：迁移 + 首次启动 0.74 s；二次启动 2.2 s；备份还原 < 1 s。数字仅证明流程可行，生产 RTO 以目标环境重跑为准。

---

## 10. 生产执行记录（2026-11-15）

已在 `VM-0-14-ubuntu` 完成切换，全流程按 §6 执行。

### 10.1 结果

| 项 | 实际 |
| --- | --- |
| 停机窗口 | 06:59:29Z → 06:59:36Z，**约 7 秒** |
| 产物 | `yufu v0.1.1 x86_64-unknown-linux-gnu`，commit `0b15b80`，SHA256 `822a91616248cf256648e24fa4d673af79c00f8125ad902045b528750e44bb0f`（与 `SHA256SUMS` 一致） |
| 安装位置 | 二进制 `/opt/cobotrix/yufu`；配置 `/opt/cobotrix/yufu.toml`（0600）；unit `/etc/systemd/system/yufu.service` |
| 迁移 | `seaql_migrations` 16 → **43**，全量数据保留 |
| 数据 | 15 用户 / 4 客户端 / 563 token / 108 recording，与切换前一致 |
| 签名 | JWKS modulus 与切换前**完全相同** |
| 服务 | `yufu.service` enabled+active；`cobotsrv.service` disabled+inactive（二进制与配置保留） |
| 时钟 | 4 个真实源，实测误差 20—26 ms |

### 10.2 执行中发现的两个额外问题

**A. `issuer_conflict` 会阻断纳管（原方案未覆盖）**

扫描时 `叶叶笔记` 与 `小Bytes的博客` 报 `issuer_conflict`。根因：历史 `oauth_token.id_token` 的 `iss` 有两种值 —

```
https://cobotrix.backend.gataccay.com   321 条（旧 issuer，全部已过期）
https://cobotrix.gataccay.com            242 条（现行 issuer）
```

`adopt_legacy` 对 `issuer_conflict` 硬拒（`IdentityError::Conflict`），所以这两家无法纳管。处置：

1. 断言 321 条旧 issuer 记录中**活跃 refresh token 数为 0**（脚本内强制断言，不为 0 即中止）；
2. 整行备份到新表 `oauth_token_legacy_issuer`（实测 321 = 321，数量不符即中止删除）；
3. 再从 `oauth_token` 删除这 321 条，保留 242 条现行 issuer 记录。

复扫后 4 个客户端均只剩 `orphan_client`，可正常纳管。**该表不要删除**，是这些历史 token 的唯一留存。

**B. 纳管会覆盖 `oauth_client.redirect_uris`（演练时踩中）**

演练中用了一个错误回调注册，纳管后 `oauth_client.redirect_uris` 被静默改写成注册值。**因此纳管 payload 的 `redirect_uris` 必须取自库里当前的真实值**，不能手写、不能取旧备份。生产执行时改为从 `oauth_client` 实时读取后再构造 payload。

### 10.3 纳管结果

4 个客户端全部纳管成功（`management_mode = admin_configured`），保留环境沿用原 `client_id`：

| 客户端 | 保留环境 | 保留 client_id | 回调（未改） |
| --- | --- | --- | --- |
| 叶叶笔记 | production | `ab9dde80-…` | `https://outline.gataccay.com/auth/oidc.callback` |
| 小Bytes的博客 | production | `1de14ede-…` | `https://artalk.gataccay.com/api/v2/auth/auth0/callback` |
| Binrag | development | `3a0cd7e2-…` | `http://localhost:8085/api/v1/auth/oidc/cobotrix/callback` |
| BinIM | development | `45ef2a0f-…` | `http://127.0.0.1:20800/api/auth/auth_public/v1/oidc/callback` |

`Binrag`/`BinIM` 的回调是明文 HTTP 回环地址，而生产环境强制 https（`parse_http_uri` 规则），因此只能保留在 `development`。

四个 `client_secret_hash` 全部保持原值 → **既有客户端 secret 无需更换**。

> 副作用：非保留环境需要非空 redirect_uris，且 `.invalid` 是保留 TLD（永不解析），故生成了 4 个占位客户端（`oauth_client` id 6—9，`app_*`，回调 `https://<前缀>.invalid/callback`）。它们无法用于登录，可在产品支持时清理。

### 10.4 端到端验收

用真实生产数据跑通完整授权链（经公网 HTTPS）：

```
authorize → 303 /oidc/consent?flow_id=…
         → SSO 登录（遗留密码）
         → POST /api/v1/identity/flows/{id}/complete {"approved":true}
         → 302 https://outline.gataccay.com/auth/oidc.callback?code=…&state=e2e-proof
```

code 正常签发，RP 用未变更的 secret 即可完成兑换。

### 10.5 回滚就位

`/home/ubuntu/cobotrix-deploy/` 保留：

- `cobotrix-precutover-20261115T065930Z.dump`（停写后备份，563 token / 16 迁移）
- `cobotrix-20261115T064251Z.dump`
- `cobotsrv.rollback-20261115T064251Z`（旧二进制）
- `cobotsrv.toml.20261115T064251Z`、`oidc_rsa_private.LIVE.pem`、`oidc_rsa_private.optcopy.pem`
- `yufu.toml`、`yufu-v0.1.1`

回滚步骤见 §7：`yufu` 停 → 还原 pre-cutover dump → `systemctl start cobotsrv`。

### 10.6 切换后问题：`/oidc/login` 兼容缺口（已修复）

**现象**：从叶叶笔记（Outline）发起登录，浏览器停在
`https://cobotrix.gataccay.com/oidc/login?response_type=code&redirect_uri=…&client_id=ab9dde80-…`，
页面报「授权流程缺失，请从应用重新发起登录」。

**定位**：该文案来自 `web/src/pages/oidc-consent/index.vue:17` —— 只要路由 query 里**没有 `flow_id`** 就抛此错。

- `/oidc/login` 在新版 SPA 里是 consent 组件的另一个路由（`routes.ts:90`），它期望的是 `?flow_id=` 这种**授权后**的参数；
- 但 Outline 把 `/oidc/login` 当作 **OAuth 入口**，直接发原始 authorizing 参数（`response_type`/`redirect_uri`/`client_id`…），没有 `flow_id`；
- 新版的服务端入口是 `/oidc/authorize`，`/oidc/login` 在服务端只在 `webui.rs:152` 的 SPA 深链白名单里出现，**服务端从不生成该跳转**；
- 该 URL 是 Outline 侧存储的旧配置（旧版 `cobotsrv` 时期就已存在），不在 discovery 文档里，因此换服务端后无人更新它。

**修复**（Caddy 边缘别名，`/home/ubuntu/caddy/Caddyfile`）：

```caddyfile
@legacyOAuthEntry {
    path /oidc/login
    not query flow_id=*
}
rewrite @legacyOAuthEntry /oidc/authorize
```

用 `not query flow_id=*` 做条件，保证 SPA 自己带 `flow_id` 的链接仍然正常返回页面。

**验证**：Outline 的原始 URL 现在返回 `303 /oidc/consent?flow_id=…`，完整链路
（登录 → 同意 → `complete`）返回
`https://outline.gataccay.com/auth/oidc.callback?code=…`；`/oidc/login?flow_id=…` 仍是 SPA 200。

**同源风险**:除 Outline 外的三个客户端也可能存有旧入口路径。切换后应逐个真实登录一次，并借助新增的
`/var/log/caddy/cobotrix.log` 核对实际请求路径。

### 10.7 新增诊断能力

`cobotrix.gataccay.com` 站点原本没有 `log` 指令，因此 Caddy 不产出访问日志。现已补充：

```caddyfile
log {
    output file /var/log/caddy/cobotrix.log {
        roll_size 20mb
        roll_keep 5
    }
    format json
}
```

Caddyfile 实际所在位置是宿主机 `/home/ubuntu/caddy/Caddyfile`（bind mount 到容器 `/etc/caddy/Caddyfile`），改动须写宿主机文件。备份见 `/home/ubuntu/cobotrix-deploy/Caddyfile*.bak*`。

另注：`/oidc/consent` 由 SPA 静态回退提供，**不经过** yufu 的请求日志中间件，所以只在 yufu 日志里查不到该页请求；`/api/v1/identity/flows/{id}` 会记录，可据此判断前端是否拿到了 `flow_id`。

### 10.8 切换后问题：userinfo 不返回 `email`，Outline 拒绝登录（已修复）

**现象**：授权与 code 兑换全部成功（`POST /oidc/token` 200、`GET /api/v1/oidc/userinfo` 200），但 Outline 前端显示「抱歉发生未知错误」。

**根因**（Outline 日志 `outline-outline-1`）：

```
UnauthorizedError: An email field was not returned in the profile or id_token
parameter, but is required.   at OIDCStrategy._verify (oidcRouter.js:82)
```

Outline 的判定（`build/plugins/oidc/server/auth/oidcRouter.js`）：

```js
const email = profile.email ?? token.email ?? null;
if (!email) throw AuthenticationError("An email field was not returned …");
```

而新版 `email` 的来源只有 `"user".email` 一列：

```sql
SELECT nickname,username,email,email_verified_at FROM "user" WHERE id=$1 AND enabled
```

**关键坑**：`[oidc] email_domain` 这个配置项在新版**只被 config.rs 声明，没有任何代码使用**——文档里承诺的 `{username}@{email_domain}` 回退并未实现。迁移给 `"user"` 加 `email` 列时全部为 `NULL`，于是 userinfo 省略 `email`。

**修复**：为 15 个账户回填 `"user".email`。但**不能统一套一个域名**：Outline 里既有账号分属两个域名（历史遗留，`email_domain` 曾从 `binhy.email` 改为 `binhy.cobotrix` 又改回），

```
@binhy.email      : Bin-hy, 504-user, xtbb, lby
@binhy.cobotrix   : lyk, wyq, wqy, zrh, huangxx, lpz, caojun, renyao, yhm
```

若统一成 `@binhy.email`，那 9 个 `@binhy.cobotrix` 的用户登录后 Outline 会**另建新账号**，原文档归属丢失。因此按 `user.username` 与 Outline 账号的 local part 精确匹配，逐个回填各自**既有**的邮箱；无匹配的两户（`guest`、`lcf`）用 `@binhy.email` 默认值。

结果：13/15 与既有 Outline 账号精确对齐，无重复邮箱。

`email_verified` 保持 `false`（`email_verified_at` 为 NULL）：Outline 只把它透传给账号 provisioning，**不用于拦截登录**，无需伪造验证状态。

验证（用纳管时生成的一次性测试客户端做完整兑换）：

```json
{"iss":"https://cobotrix.gataccay.com","sub":"1","aud":"app_…",
 "name":"sadmin","preferred_username":"Bin-hy",
 "email":"Bin-hy@binhy.email","email_verified":false}
```

**遗留待办**：`email_domain` 回退在新版缺失属于实现与文档不一致；若要恢复该行为，需在 profile 装载处补回退逻辑，否则新增用户必须先写 `email` 才能被下游 RP 接受。

### 10.9 单点退出（RP-initiated logout）接线

**目标端点**：`https://cobotrix.gataccay.com/oidc/end-session`（由 discovery 的 `end_session_endpoint` 公布，实现在 `route/identity_logout.rs`，**不是** `route/oidc.rs` 里的旧 `/oidc/logout`）。

实测行为：带上合法参数请求后返回 `303` 跳回 RP，并下发 `__Host-yufu-sso` / `__Host-yufu-center-csrf` 的清除 Cookie（`Max-Age=0`），即真正销毁中心 SSO 会话。

**两处配置缺一不可：**

1. **中心侧允许回跳地址。** `LogoutRequest` 会把 `post_logout_redirect_uri` 与客户端配置里的
   `post_logout_redirect_uris` 逐项比对（`service/identity/logout.rs`），不在白名单直接 `InvalidInput`。
   迁移纳管时该项为空，会导致退出被拒。补法（管理 API，勿手改库）：

   ```bash
   PUT  /api/v1/admin/apps/{application_id}/environments/{environment_id}/config
   POST /api/v1/admin/apps/{application_id}/environments/{environment_id}/activate
   ```

   PUT 只生成新版本，**必须在 `activate` 后才生效**（`active_version_id` 未变即为未生效）。请求体需带
   `expected_revision`（取自 `GET /api/v1/admin/apps/{id}`）与 `Idempotency-Key`；
   activate 需带 `version_id`、`expected_revision`、`accept_logout_limitation`。

2. **RP 侧结束会话地址。** Outline 处于 **manual config 模式**（`OIDC_AUTH_URI` 等四个变量齐全，所以
   它一直用 `/oidc/login`），此模式下 `logoutURL` **只取 `OIDC_LOGOUT_URI`**，不会读 discovery 的
   `end_session_endpoint`（`plugins/oidc/server/auth/oidc.js`）。缺该变量时 Outline 直接
   `ctx.redirect("/")`，看似"退出"实则中心会话仍在。

   在 `/home/ubuntu/outline/docker.local.env` 追加：

   ```
   OIDC_LOGOUT_URI=https://cobotrix.gataccay.com/oidc/end-session
   ```

   然后 `docker compose up -d`（`env_file` 变更必须重建容器，`docker restart` 不生效）。

**叶叶笔记实测链路**：

```
GET https://outline.gataccay.com/auth/oidc.logout
  302 → https://cobotrix.gataccay.com/oidc/end-session?client_id=ab9dde80-…&post_logout_redirect_uri=https%3A%2F%2Foutline.gataccay.com
  303 → https://outline.gataccay.com       并清除 __Host-yufu-sso
```

负向验证：把 `post_logout_redirect_uri` 换成未登记域名返回 `400`，白名单有效。

> 其余三个客户端（Artalk / Binrag / BinIM）如需单点退出，同样要各自把回跳地址加进其应用配置的
> `post_logout_redirect_uris` 并在客户端侧配置对应 logout URL。

---

## 11. 容器化部署（2026-11-15 切换完成）

为便于后续升级，生产已由 **systemd 裸二进制** 改为 **Docker Compose**，升级只剩 `pull` + `up -d`。

### 11.1 宿主机布局

```
/opt/cobotrix-docker/
├── compose.yml          # 单服务，对外发布 2644
├── .env                 # 0600：DATABASE_URL / AUTH_SECRET / LOG_LEVEL / YUFU_IMAGE
├── yufu.toml            # 0644：只读挂载到 /etc/yufu/yufu.toml
├── keys/
│   └── oidc_rsa_private.pem   # 0600 uid 10001：只读挂载，绝不由容器生成
└── state/               # uid 10001：容器 /var/lib/yufu（实测保持为空）
```

关键点：

- **端口仍发布 2644**（容器内 4000），因此 Caddy 的 `host.docker.internal:2644` **完全不用改**。
- **数据库**走 `host.docker.internal:5444`（`extra_hosts: host.docker.internal:host-gateway`），即宿主机上已有的 `postgres18` 容器，未另起数据库。
- **签名私钥 bind mount 只读**，容器以 uid 10001 运行，文件 `0600` 且属 10001。实测替换容器后 JWKS 不变。
- `read_only: true` + `cap_drop: ALL` + `no-new-privileges`。
- 镜像内建 HEALTHCHECK 打 `/api/v1/health/ready`，`restart: unless-stopped`，宿主 `docker` 开机自启 → 断电重启后自动拉起。

### 11.2 镜像来源

官方镜像 `ghcr.io/bin-hy/yufu`（**public，可匿名拉取**），与 release 归档同源、同 commit。

本机到 `ghcr.io` 可达但很慢（与 `github.com` 被墙一致的观感），而**南京大学镜像 `ghcr.nju.edu.cn` 是透明代理**：
实测 `docker pull ghcr.nju.edu.cn/bin-hy/yufu:0.1.1` 得到的 digest
`sha256:600250fe7421dffc575f64ff731ae81d5a8231a01f298dc5967fc274688020ff`
与 `ghcr.io` 官方 manifest **完全一致**，因此镜像完整性可信。`.env` 里默认用镜像站加速。

可用标签：`0.1.1`、`0.1`、`latest`、`sha-<commit>`。

### 11.3 日常升级

```bash
cd /opt/cobotrix-docker

# 方式一：升到某个具体版本（推荐，可回退）
sudo sed -i 's#^YUFU_IMAGE=.*#YUFU_IMAGE=ghcr.nju.edu.cn/bin-hy/yufu:0.2.0#' .env
sudo docker compose pull && sudo docker compose up -d

# 方式二：跟随最新
sudo sed -i 's#^YUFU_IMAGE=.*#YUFU_IMAGE=ghcr.nju.edu.cn/bin-hy/yufu:latest#' .env
sudo docker compose pull && sudo docker compose up -d
```

`up -d` 会自动重建容器；启动时 `Migrator::up` 自动跑数据库迁移，无需手工迁移。

升级后自检：

```bash
sudo docker ps --filter name=yufu
curl -s https://cobotrix.gataccay.com/api/v1/health/ready
curl -s https://cobotrix.gataccay.com/.well-known/jwks.json | head -c 120
```

> **JWKS 的 `n` 值必须和升级前一致**。若变了，说明私钥挂载丢了 —— 立即停止并检查 `keys/` 挂载。

### 11.4 回退

Docker 侧回退只需把 `YUFU_IMAGE` 改回上一个标签再 `up -d`（数据库迁移通常向后兼容）。

systemd 裸二进制路径**仍完整保留**作为最终回退：`/opt/cobotrix/yufu` + `/opt/cobotrix/yufu.toml`
+ `yufu.service`（已 `disable` 未删除）。若需退回：

```bash
cd /opt/cobotrix-docker && sudo docker compose down
sudo systemctl enable --now yufu.service
```

### 11.5 顺带修掉的安全问题

原先 systemd 部署既没有在配置里写 `[auth].secret`，也没有设 `AUTH_SECRET` 环境变量，于是
`default_auth_secret()` 回落到**硬编码的公开默认值**
（`qwertyuiop.asdfghjkl.zxcvbnm.1234567890-=`），CSRF 令牌实际用公开常量签名。

容器化时改为由 `.env` 注入 `openssl rand -base64 48` 生成的强随机值。副作用：切换瞬间旧的
控制台会话与 CSRF 令牌失效，需重新登录一次；OIDC 侧用 RS256 签名的 token 不受影响。

### 11.6 切换实测

| 项 | 结果 |
| --- | --- |
| 停机窗口 | 07:40:14Z → 07:40:29Z，约 15 s |
| 容器健康 | `healthy`，内建 healthcheck 通过 |
| JWKS | 与 systemd 版本**逐字节相同** |
| 数据 | 15 用户 / 8 客户端 / 43 迁移，未变 |
| 公网入口 | discovery / jwks / userinfo / `/oidc/login` 别名全部正常 |
| 控制台登录 | `identity/login` 200（AUTH_SECRET 轮换后） |
| 容器替换 | `--force-recreate` 后 JWKS 仍不变 |
| 时钟 | 容器内 `--check-security-clock` = 24 ms |
