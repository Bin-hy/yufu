# CoBotrix

CoBotrix (Co-Robotics Matrix) 协熵云枢

一个针对智能设备的云平台框架，通常为远程智能设备，比如 Robotics。

提供统一的网络通信，统一的 API 和用户权限和设备管理

## 构建依赖
- [cargo](https://www.rust-lang.org/)
- [nodejs](https://nodejs.org/)
- [pnpm](https://pnpm.io/)

## 运行

本项目支持三种部署形态，按"开发 → 嵌入式单二进制 → 前后端分离"三条路径分别说明。三种形态共用一套配置模板派生流程。

### 1. 首次 clone 后的准备（所有部署形态都需要）

仓库里所有 `*.example` 和 `*.local.*` 文件都是**模板**，真实运行时用到的配置文件需要在本地从模板复制派生，复制后的副本不会被 Git 跟踪（见 `.gitignore`）。

```bash
# 根目录 env（compose.yml 会读取 .env）
cp .env.example .env

# cobotsrv 后端配置
cp conf/cobotsrv.example.toml cobotsrv.toml   # 默认配置文件（已被 .gitignore 忽略）

# 前端 env
cp cobotweb/.env.example cobotweb/.env.local
```

安全提示：

- `conf/oidc_rsa_private.pem` 无需手动创建：`cobotsrv` 首次启动时若不存在会自动生成 2048-bit RSA 密钥并落盘，后续启动复用；该文件已被 `.gitignore` 忽略，**切勿提交**。
- `cobotsrv.toml` 请只留在本机，不要提交也不要分享。
- `cobotweb/.env.example` 默认把 `VITE_BASE_URL` 设为相对路径 `/api/v1`，适用于模式 A / C 和同域反代的模式 B；跨域 split 部署需要改为绝对 URL（见下文"模式 B"）。

### 2. 启动基础设施

开发/测试环境（只起 Postgres，`cobotnil` 自带 MQTT broker 不需要额外跑 EMQX）：

```bash
docker compose up
```

生产环境（Postgres + EMQX，依赖根目录 `.env`）：

```bash
docker compose -f conf/compose.prod.yaml up -d
```

---

### 模式 A：嵌入式单二进制（推荐生产默认）

前端产物由 `rust-embed` 编译期内联到 `cobotsrv` 二进制里，**一个进程同时提供 SPA 静态资源和 API**，适合单机、边缘网关、离线发行。

**构建顺序：必须先构建前端，再构建后端**。这是因为 `cobotsrv/src/lib.rs` 在 `webui` feature 下把 `../assets/cobotsrv/` 目录通过 `rust-embed` 静态嵌入，缺失即编译失败。

```bash
# Step 1 前端构建 → 产物落到 assets/cobotsrv/
pnpm install
pnpm run build

# Step 2 后端构建（开启 webui feature 触发 embed）
cargo build --release --features webui --bin cobotsrv

# Step 3 运行
./target/release/cobotsrv -c cobotsrv.toml
```

前端 env 建议：`cobotweb/.env.local`（或 `.env.production.local`）保持 `VITE_BASE_URL=/api/v1`。请求走同源相对路径，无需 CORS。

浏览器访问 `http://<cobotsrv-host>:4000/`（默认端口，可在 `cobotsrv.toml` `[http] listen` 修改）。

---

### 模式 B：前后端分离部署

前端以纯静态资源独立托管（Nginx / CDN / S3 / Vite preview），后端以纯 API 进程形式运行（**不启用 `webui` feature**）。

```bash
# 后端（不开 webui，体积更小）
cargo build --release --bin cobotsrv
./target/release/cobotsrv -c cobotsrv.toml

# 前端
pnpm install
pnpm run build        # 产物在 assets/cobotsrv/（路径由 vite.config.ts 的 build.outDir 决定）
```

然后把 `assets/cobotsrv/` 目录里的静态文件部署到你的静态托管方案。两种子场景：

#### B.1 同域反代（推荐）

Nginx 同一个域名下，`/` 走前端静态文件，`/api/v1`、`/oidc/*`、`/.well-known/*` 反代到 `cobotsrv`。前端 `VITE_BASE_URL=/api/v1` 就够用，无 CORS 问题。

```nginx
server {
    listen 80;
    server_name cobotrix.example.com;

    root /var/www/cobotrix;           # 拷贝 assets/cobotsrv/ 的内容到这里
    index index.html;

    location / {
        try_files $uri $uri/ /index.html;   # SPA 回退
    }

    location ~ ^/(api/v1|oidc|\.well-known)/ {
        proxy_pass http://cobotsrv-backend:4000;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

#### B.2 跨域部署（前端与后端分别在不同域名）

比如前端发布到 `app.example.com`，后端 API 在 `api.example.com`。此时前端 `VITE_BASE_URL` 必须是**绝对 URL**：

```bash
# cobotweb/.env.production.local
VITE_BASE_URL=https://api.example.com/api/v1
```

注意 `VITE_BASE_URL` 在 `vite build` 时被固化，切换目标 API 需重新构建。

后端 `cobotsrv.toml` 必须把前端域名加入 CORS 白名单：

```toml
[auth]
origins = [
    "https://app.example.com",
]
```

---

### 模式 C：本地开发（热重载）

后端用 `cobotnil` 启动的虚拟集群，前端用 Vite dev server，Vite 自带的 proxy 会把 `/api/v1`、`/oidc/*`、`/.well-known` 都转发到后端（`cobotweb/vite.config.ts` 已配置）。

```bash
# 终端 1：后端开发集群（1 个 cobotsrv + 3 个虚拟 cobotlet + 内置 MQTT broker）
cargo run -- -c conf/cobotnil

# 或只跑后端单服务
cargo run --bin cobotsrv -- -c cobotsrv.toml

# 终端 2：前端 dev server
pnpm run dev
```

前端 env：`cobotweb/.env.local` 保持 `VITE_BASE_URL=/api/v1`（走 Vite proxy），或者设成绝对 URL（跳过 proxy 直连后端，比如 `http://localhost:4000/api/v1`）。两种都能跑，前者更接近生产形态。

---

### 部署形态选型速查

| 维度 | A. 嵌入式 | B.1 同域反代 | B.2 跨域 split | C. 开发 |
|---|---|---|---|---|
| cargo feature | `webui` | 默认（无） | 默认（无） | 默认（无） |
| 前端构建必须 | ✅（编译期嵌入） | ✅ | ✅ | ❌（Vite dev） |
| 需要 Nginx/CDN | ❌ | ✅ | ✅ | ❌ |
| `VITE_BASE_URL` | `/api/v1` | `/api/v1` | `https://api.xxx/api/v1` | `/api/v1` |
| CORS 配置 | 不需要 | 不需要 | 需要 | 不需要 |
| 运维复杂度 | 最低 | 中 | 高 | — |

## 项目结构

采用服务和启动器分离模式，大部份情况代码会运行在三种不同类型的机器上（比如：Server，Client，Device）

部分组件会同时存在不同类型的机器上，比如 Map（地图）在 Server，Client 上，Net（网络）在 Server，Device 上


```mermaid
flowchart LR

cobotlet ----- Device
cobotsrv ----- Server
cobotweb ----- Webui
cobotimx ------- Webui
cobotimx ------- Server
cobotrtc ------- Webui
cobotrtc ------- Server
cobotrtc ------- Device
cobotmap ------- Server
cobotmap ------- Webui
cobotnet ------- Server
cobotnet ------- Server
cobotset ------- Device
cobotlog ------- Device
cobotlog ------- Webui
cobotdoc ------- Webui
```

### 目录说明

- 云平台
  - `cobotweb` 云服务前端代码
  - `cobotsrv` 命令灵感来自 Linux 各发行版提供的 `/srv/` 路径，云端服务端后端
  - `cobotlet` 命令灵感来自 `kubelet`，运行在各 device 上的通信服务，主要负责和 `cobotsrv` 通信
- 核心组件
  - `cobotimx` Instant Messaging eXchange（即时消息交换平台）
  - `cobotrtc` 实时交互音视频（基于 `Live777`）
  - `cobotmap` 地图服务
  - `cobotnet` 基于 WireGuard 配合 ICE 通过云平台辅助启动一个 P2P VPN 来实现大量数据交互（类似 Tailscale）
  - `_cobotkvm` `TODO` KVM 扩展
- 开发部署工具链
  - `cobotnil` 和 Live777 的 `livenil` 一样，只在 开发/测试 模式里使用，快速启动一个集群
  - `cobotset` 通用设备初始化设置工具
  - `cobotlog` 通用设备服务异步日志管理
  - `cobotdoc` 文档
  - `cobotxxx` 遵守命名规则 `cobot` + `xxx`

## API 说明

```bash
/remote/<Device Name>/<Module Name>
```


`<Device Name>` 就是全局唯一的命名，`<Module Name>` 为个项目自定义名称，通常为 HTTP 一级子路径


### 内置云模块

对内置云模块名称特殊处理（通常 `<Module Name>` 由 `Device` 提供）。调用者无法感知是云模块还是边缘模块

```bash
/remote/<Device Name>/rtc
```

## `CoBotSrv` 和 `CoBotLet` 的通信说明

使用 Net4mqtt 来完成反向代理，依赖 Mqtt Broker

Net4mqtt 工作原理参考 [Live777/Net4mqtt](https://live777.pages.dev/zh/guide/net4mqtt)

**Net4mqtt 主要用于保障核心通信，无状态水平扩容，CoBotSrv 和 CoBotLet 实例数量比 `M:N`。不适合大规模数据传输和高性能网络**

## OIDC 实现

### 设计目标

为 CoBotrix 平台提供标准的 OpenID Connect 身份认证能力，支持：

- **Authorization Code + PKCE** 流程 — 面向前端 (cobotweb) 和第三方集成
- **Client Credentials** 流程 — 面向 M2M (cobotlet → cobotsrv) 机器间认证
- **Refresh Token** 轮换 — 安全的无感续期
- **RSA 签名** — ID Token 使用 RS256 签名，符合 OIDC 标准验证流程

### 整体架构

```
┌─────────────┐     ┌──────────────────────────────────────────────────┐
│  cobotweb   │     │                  cobotsrv                        │
│  (Vue SPA)  │────▶│                                                  │
└─────────────┘     │  ┌─────────────┐  ┌──────────┐  ┌────────────┐  │
                    │  │  OIDC Route  │  │ Auth Lib │  │  SeaORM    │  │
┌─────────────┐     │  │             │  │          │  │  Entities  │  │
│  cobotlet   │────▶│  │ authorize   │  │ Token    │  │            │  │
│  (Device)   │     │  │ token       │  │ Manager  │  │ oauth_     │  │
└─────────────┘     │  │ userinfo    │  │          │  │ client     │  │
                    │  │ revoke      │  │ IdToken  │  │ scope      │  │
┌─────────────┐     │  │ introspect  │  │ Signer   │  │ token      │  │
│ Third-party │────▶│  │ jwks        │  │ (RSA)    │  │ auth_code  │  │
│   Client    │     │  │ discovery   │  │          │  │            │  │
└─────────────┘     │  └─────────────┘  └──────────┘  └────────────┘  │
                    │                                                  │
                    │                  PostgreSQL                       │
                    └──────────────────────────────────────────────────┘
```

### 实现思路 (分层说明)

#### 1. 数据层 — Entity + Migration

四张核心表，通过 SeaORM 管理：

| 表名 | 作用 | 关键字段 |
|------|------|----------|
| `oauth_client` | 注册的 OAuth 客户端 | `client_id` (唯一), `client_secret_hash` (bcrypt), `redirect_uris`, `allowed_scopes`, `grant_types` |
| `oauth_scope` | 可用的 scope 定义 | `name` (唯一, 如 openid/profile/email), `description` |
| `oauth_token` | 已发放的 token 记录 | `access_token`, `id_token`, `refresh_token` (唯一), `revoked`, 过期时间 |
| `oauth_authorization_code` | 授权码 (持久化到 DB) | `code` (唯一), `user_id`, PKCE 信息, `used`, `expires_at` |

**设计决策：**
- 授权码持久化到 DB 而非内存 → 支持多实例水平扩展 (无状态 cobotsrv)
- Token 记录包含 `revoked` 字段 → 支持主动撤销和刷新轮换
- `client_secret_hash` 使用 bcrypt → 注册时返回明文 secret 一次，之后只存哈希

#### 2. 认证层 — libs/auth

`libs/auth` 提供 JWT 签发与验证，OIDC 扩展了两个核心能力：

**IdTokenClaims** — ID Token 的 JWT 载荷：
```rust
pub struct IdTokenClaims<T> {
    pub iss: String,         // 签发者
    pub sub: T,              // 用户标识 (泛型，支持 i32 / String)
    pub aud: Option<String>, // 客户端 client_id
    pub iat: u64,            // 签发时间
    pub exp: u64,            // 过期时间
    pub nonce: Option<String>, // 防重放
    pub at_hash: Option<String>, // access_token 的 SHA-256 左半部分
}
```

**IdTokenSigner** — RSA / HMAC 双模签名器：
```rust
// 优先使用 RSA (RS256)
let signer = IdTokenSigner::from_rsa_pem(pem_bytes, expire_secs)?;
// 回退到 HMAC (HS256)
let signer = IdTokenSigner::from_hmac(secret, expire_secs);

// 签发 ID Token
signer.sign_id_token(user_id, issuer, audience, nonce, access_token)?;
// 导出 JWKS JSON (仅 RSA 模式可用)
signer.jwks_json(); // -> Option<String>
```

**RSA 密钥管理：** 启动时自动加载或生成 2048-bit RSA 密钥对，保存到 `conf/oidc_rsa_private.pem`，可通过 `auth.rsa_private_key_path` 配置自定义路径。

#### 3. 路由层 — OIDC Endpoints

分为三组路由，按认证要求挂载：

##### 公开端点 (无需认证)

| 方法 | 路径 | 作用 | 规范 |
|------|------|------|------|
| `GET` | `/.well-known/openid-configuration` | OIDC Discovery | RFC 8414 |
| `GET` | `/.well-known/jwks.json` | RSA 公钥 (JWK Set) | RFC 7517 |
| `POST` | `/oidc/token` | Token 端点 (统一入口) | RFC 6749 |
| `POST` | `/oidc/revoke` | Token 撤销 | RFC 7009 |
| `POST` | `/oidc/introspect` | Token 内省 | RFC 7662 |

##### 认证端点 (需要用户登录)

| 方法 | 路径 | 作用 |
|------|------|------|
| `GET` | `/api/v1/oidc/authorize` | 授权码发放 (Authorization Code + PKCE) |
| `GET` | `/api/v1/oidc/userinfo` | 用户信息 (scope 驱动返回字段) |

##### 管理端点 (需要认证 + 管理员)

| 方法 | 路径 | 作用 |
|------|------|------|
| `POST` | `/api/v1/oidc/client` | 注册 OAuth 客户端 (返回 client_secret) |
| `POST` | `/api/v1/oidc/scope` | 添加自定义 scope |
| `GET`  | `/api/v1/oidc/scope` | 列出所有 scope |

#### 4. 授权流程详解

##### Authorization Code + PKCE 流程

```
   Client                   cobotsrv                    DB
     │                         │                         │
     │  GET /authorize         │                         │
     │  ?client_id=...         │                         │
     │  &code_challenge=...    │                         │
     │  &code_challenge_method │                         │
     │  =S256                  │                         │
     ├────────────────────────▶│  验证 client_id          │
     │                         │  验证 redirect_uri       │
     │                         │  验证 scope              │
     │                         │  生成 code               │
     │                         ├────────────────────────▶│ INSERT auth_code
     │  302 redirect_uri       │                         │
     │  ?code=xxx&state=yyy    │                         │
     │◀────────────────────────┤                         │
     │                         │                         │
     │  POST /oidc/token       │                         │
     │  grant_type=            │                         │
     │   authorization_code    │                         │
     │  &code=xxx              │                         │
     │  &code_verifier=...     │                         │
     │  &client_id=...         │                         │
     │  &client_secret=...     │                         │
     ├────────────────────────▶│  验证 client_secret      │
     │                         │  (bcrypt)               │
     │                         ├────────────────────────▶│ take_code (标记 used)
     │                         │  验证 PKCE              │
     │                         │  (S256/plain)           │
     │                         │  签发 access_token (JWT) │
     │                         │  签发 id_token (RS256)   │
     │                         │  生成 refresh_token      │
     │                         ├────────────────────────▶│ INSERT oauth_token
     │  {access_token,         │                         │
     │   id_token,             │                         │
     │   refresh_token}        │                         │
     │◀────────────────────────┤                         │
```

##### Client Credentials 流程 (M2M)

```
   cobotlet                 cobotsrv
     │                         │
     │  POST /oidc/token       │
     │  grant_type=            │
     │   client_credentials    │
     │  &client_id=...         │
     │  &client_secret=...     │
     │  &scope=...             │
     ├────────────────────────▶│  验证 client_secret
     │                         │  检查 grant_types 包含 client_credentials
     │                         │  验证 scope 允许
     │                         │  签发 access_token (JWT, sub=client_id)
     │  {access_token}         │
     │◀────────────────────────┤  (无 id_token, 无 refresh_token)
```

##### Refresh Token 轮换

```
   Client                   cobotsrv                    DB
     │                         │                         │
     │  POST /oidc/token       │                         │
     │  grant_type=            │                         │
     │   refresh_token         │                         │
     │  &refresh_token=old_rt  │                         │
     │  &client_id=...         │                         │
     │  &client_secret=...     │                         │
     ├────────────────────────▶│  验证 client_secret      │
     │                         ├────────────────────────▶│ 查找 old_rt (未撤销)
     │                         │  检查未过期              │
     │                         │  检查 client_id 一致     │
     │                         ├────────────────────────▶│ 撤销 old_rt (revoked=true)
     │                         │  签发新 access_token     │
     │                         │  签发新 id_token         │
     │                         │  生成新 refresh_token    │
     │                         ├────────────────────────▶│ INSERT 新 token 记录
     │  {access_token,         │                         │
     │   id_token,             │                         │
     │   new_refresh_token}    │                         │
     │◀────────────────────────┤                         │
```

> 轮换策略：每次刷新都撤销旧 refresh_token 并发放新的，防止 token ���露后被反复使用

#### 5. 安全设计

| 机制 | 说明 |
|------|------|
| **PKCE** | 强制 S256 / plain，防止授权码拦截攻击 |
| **Client Secret** | bcrypt 哈希存储，注册时仅返回明文一次 |
| **RS256 签名** | ID Token 使用 RSA 非对称签名，第三方可通过 JWKS 验证 |
| **Refresh Token 轮换** | 每次使用后旧 token 立即撤销 |
| **授权码一次性** | DB 中 `used` 字段标记，防止重放 |
| **授权码 5 分钟过期** | 短时效，减少泄露风险 |
| **Token 撤销** | 支持主动撤销 access_token 和 refresh_token |
| **Client Grant 限制** | `grant_types` 字段控制客户端可用的授权方式 |
| **Scope 校验** | 请求 scope 必须在客户端 `allowed_scopes` 范围内且在系统中注册 |

#### 6. 配置

`cobotsrv.toml` 中 OIDC 相关配置：

```toml
[auth]
secret = "your-hmac-secret"       # HMAC 密钥 (access_token / refresh_token 签名)
exp_seconds = 7200                # access_token 过期时间 (秒)
refresh_exp_seconds = 604800      # refresh_token 过期时间 (秒)
rsa_private_key_path = "conf/oidc_rsa_private.pem"  # RSA 私钥路径 (可选, 不设则自动生成)

[http]
public = "https://your-domain.com" # 用于 OIDC Discovery 中的 issuer
```

#### 7. 标准合规

支持的 OIDC/OAuth 规范：

- **OpenID Connect Core 1.0** — Discovery, Authorization Code, ID Token, UserInfo
- **RFC 6749** — OAuth 2.0 Authorization Framework
- **RFC 7636** — PKCE (Proof Key for Code Exchange)
- **RFC 7517** — JSON Web Key (JWK) / JWKS endpoint
- **RFC 7009** — Token Revocation
- **RFC 7662** — Token Introspection
- **RFC 6750** — Bearer Token Usage

#### 8. 文件结构

```
cobotsrv/src/
├── route/
│   ├── oidc.rs              # OIDC 核心端点 (discovery, authorize, token, revoke, introspect, userinfo)
│   └── oidc_admin.rs        # 管理端点 (client 注册, scope 管理)
├── entity/
│   ├── oauth_client.rs      # 客户端实体 (secret 验证, scope/grant 检查)
│   ├── oauth_scope.rs       # scope 实体
│   ├── oauth_token.rs       # token 实体 (撤销, 查找)
│   └── oauth_authorization_code.rs  # 授权码实体 (一次性消费, 过期清理)
├── migration/
│   ├── m20260331_000001_create_oauth_client.rs
│   ├── m20260331_000002_create_oauth_scope.rs
│   ├── m20260331_000003_create_oauth_token.rs
│   └── m20260331_000004_create_oauth_authorization_code.rs
└── seed/
    └── m20260331_000001_seed_oidc_scopes.rs  # 预置标准 OIDC scope

libs/auth/src/
├── claims.rs          # IdTokenClaims 结构体
└── token_manager.rs   # IdTokenSigner (RSA/HMAC 双模���名, JWKS 导出)
```

