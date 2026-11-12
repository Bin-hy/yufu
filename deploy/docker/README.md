# yufu 容器部署

本目录提供从本仓库构建镜像并运行完整栈的参考配置：PostgreSQL、身份服务、Caddy TLS 入口。
项目目前**不发布预构建镜像**，镜像由这里的 `Dockerfile` 从源码构建（先构建 Vue 控制台，
再嵌入 Rust 二进制）。面向阅读的图文说明见
[官网容器部署页](https://yufu-sso.pages.dev/operations/docker/)；发版与安装包见
[打包手册](../../docs/releases/packaging.md) 与 [二进制安装说明](../../docs/releases/installation.md)。

## 目录内容

| 文件 | 说明 |
| --- | --- |
| `Dockerfile` | 两阶段构建：Node 构建控制台 → Rust 构建并嵌入 → Debian 运行层 |
| `compose.yaml` | 完整栈：`postgres`、`yufu`、`caddy`，内部网络隔离 |
| `Caddyfile` | 反向代理与自动 HTTPS；`localhost` 自动使用 Caddy 本地 CA |
| `yufu.toml.example` | 容器内的服务配置模板，挂载到 `/etc/yufu/yufu.toml` |
| `env.example` | 复制为 `.env`，提供域名、数据库凭据、`AUTH_SECRET` 等变量 |

镜像构建使用 BuildKit（Docker 23+ 默认启用）：`Dockerfile` 里的 `RUN --mount=type=cache`
与 `COPY --link` 都依赖它。首次构建需要拉取 `node`、`rust`、`debian` 基础镜像并下载依赖，
耗时明显长于后续增量构建。

运行目录中会出现 `.env`、`yufu.toml`、`state/`，它们已被 `.gitignore` 排除，不要提交。

## 组建与网络

| 服务 | 镜像 | 暴露 | 职责 |
| --- | --- | --- | --- |
| `caddy` | `caddy:2-alpine` | 宿主机 `80`/`443` | 唯一对外入口，终止 TLS |
| `yufu` | 本目录 `Dockerfile` | 仅内部 `:4000` | 身份服务，同时提供内嵌 WebUI |
| `postgres` | `postgres:18-alpine` | 仅内部 `:5432` | 持久化数据 |

`edge` 网络承载 Caddy 与身份服务；`backend` 网络是 `internal: true`，数据库没有对外出口。
身份服务以非 root（uid `10001`）运行、根文件系统只读、丢弃全部 capability。

## 快速开始（生产）

```sh
cd deploy/docker
cp env.example .env
cp yufu.toml.example yufu.toml
```

1. 生成随机值并填入 `.env`：

   ```sh
   openssl rand -base64 48   # AUTH_SECRET
   openssl rand -base64 24   # POSTGRES_PASSWORD
   ```

   把 `YUFU_DOMAIN` 改成真实域名；`YUFU_HTTPS_PORT` 保持 `443` 时，
   公开 origin 就是 `https://<域名>`。

2. 编辑 `yufu.toml`：`[oidc].issuer` 与 `[auth].origins` 必须等于公开 origin，
   并设置 `[auth.super_admin]` 的管理员用户名与强密码。
   数据库连接不需要写在这里，`DATABASE_URL` 由 `.env` 提供。

3. 生成 ID Token 签名私钥。生产环境**不会**自动生成，缺失时服务拒绝启动：

   ```sh
   mkdir -p state
   openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 \
     -out state/oidc_rsa_private.pem
   chmod 600 state/oidc_rsa_private.pem
   sudo chown 10001:10001 state/oidc_rsa_private.pem   # 容器内服务账户
   ```

   必须是 PKCS#8 PEM（文件头 `-----BEGIN PRIVATE KEY-----`）；`openssl genpkey` 默认即此格式。
   用 Docker Desktop（macOS/Windows）时通常不需要 `chown`。

4. 构建并启动：

   ```sh
   docker compose up -d --build
   docker compose ps
   docker compose logs -f yufu
   ```

5. 验证：

   ```sh
   curl -fsS https://<域名>/api/v1/health/ready
   curl -fsS https://<域名>/.well-known/openid-configuration
   ```

   `health/ready` 返回 200 且 `security_clock_synchronized=true` 才应接入流量；
   认证时钟未同步时返回 503，这是设计行为。

## 本地试用

不准备域名时可以把入口换成 `localhost`（Caddy 自动签发本地证书，浏览器会提示自签名）：

```sh
YUFU_DOMAIN=localhost
YUFU_HTTPS_PORT=8443
```

对应 `yufu.toml` 中：

```toml
[auth]
origins = ["https://localhost:8443"]

[oidc]
issuer = "https://localhost:8443"
```

然后访问 `https://localhost:8443`。容器内**不能**使用 `--allow-insecure-loopback-cookies`：
该参数要求监听地址是 loopback，而容器必须监听 `0.0.0.0` 才能被 Caddy 访问。

## 硬性约束

- **issuer 必须等于浏览器地址**，且是 `https`；不一致时服务启动失败或登录后跳转异常。
- `[auth].origins` 必须包含同一个 origin，否则 CORS 与 CSRF 校验会拒绝浏览器请求。
- 数据库密码含 `@`、`:`、`/`、`?`、`#`、`%` 等字符时必须 URL 编码后写进 `DATABASE_URL`。
- 服务启动前会做一次 NTP 校验，失败即退出；容器需要出站 **UDP/123**。
  受限网络请把 `[security_clock].servers` 换成可信内网时间源，并用
  `docker compose run --rm yufu --config /etc/yufu/yufu.toml --check-security-clock` 先诊断。
- 不要发布 `yufu` 或 `postgres` 的端口；如需直接暴露给外部代理，去掉 `caddy`
  服务并只发布 `yufu:4000`，同时自行保证 HTTPS 与 `X-Forwarded-Proto`。

## 备份与恢复

```sh
docker compose exec postgres pg_dump --username yufu yufu > yufu-$(date +%F).sql
```

需要一起备份的还有 `state/oidc_rsa_private.pem`（换钥会导致已签发令牌与既有客户端校验失效）
和 `.env`、`yufu.toml`。`caddy-data` 卷保存证书，可重新签发，不必长期备份。

## 升级

```sh
git pull
cd deploy/docker
docker compose up -d --build
```

启动时自动执行版本化数据库迁移。回滚不能只替换旧镜像：先确认新版本没有执行不可逆迁移，
并按备份恢复数据库。新增的配置字段以 `yufu.toml.example` 为准，对比后合并。

## 排障

| 现象 | 常见原因 |
| --- | --- |
| 启动即退出，日志含 `persistent RSA key unavailable` | `state/oidc_rsa_private.pem` 不存在或容器用户不可读 |
| `persistent RSA key invalid` | 私钥不是 PKCS#8 PEM，或文件被截断 |
| `production issuer requires HTTPS` | `issuer` 仍是 `http://` 或为空 |
| `security clock` 相关退出或 503 | 容器出站 UDP/123 被阻断，或时间源不可达 |
| 页面能打开但接口 403/登录失败 | `[auth].origins` 与浏览器地址不一致；注意端口 |
| 数据库连接失败 | `DATABASE_URL` 密码未 URL 编码，或 `postgres` 尚未 ready |
| `caddy` 反复申请证书失败 | 域名未解析到本机，或 80/443 被其他进程占用 |
