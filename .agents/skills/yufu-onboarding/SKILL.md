---
name: yufu-onboarding
description: |
  Deploy a local yufu identity provider and integrate it into an existing application.
  Use this skill whenever the user wants to:
  - Deploy yufu and connect this project (部署 yufu 并接入这个项目)
  - Add SSO login to a Go service (给项目加 SSO 登录)
  - Protect an API with yufu bearer tokens (用 yufu 保护 API)
  - Register a yufu application and verify redirect/audience parameters
  - Diagnose why a yufu login or token verification fails
  Triggers: yufu onboarding, 接入 yufu, 部署 yufu, go sdk 接入, OIDC 接入, SSO 登录, 令牌校验.
  Do not use for production TLS/hosting setups, M2M client credentials, Python/Node SDK work,
  or changes to the yufu server or yufu-sdk-go themselves.
---

# yufu 接入

一个入口，四个阶段：探测 → 部署 → 注册 → 接入 → 验收。

先确定 Skill 目录，后续命令都用绝对路径调用，不依赖可执行位：

```bash
SKILL_DIR="<directory containing this SKILL.md>"
```

`SKILL.md` 保持语言无关；语言细节全部在 `references/`，按阶段加载。

## 快速路径（一句话触发）

听到「部署 yufu 并接入这个项目」时，不要先讲背景，直接执行。

**只问一个问题**，不要重复询问：

> 你的应用需要让用户通过 yufu 登录，还是只在自己的 API 上校验 yufu 令牌？
> 1. 需要登录 → Web 后端
> 2. 只校验令牌 → API 资源服务器

然后一条命令完成前置检查、部署、注册与参数校验：

```bash
bash "$SKILL_DIR/scripts/onboard.sh" --mode web --project-dir "$PWD"
bash "$SKILL_DIR/scripts/onboard.sh" --mode api --project-dir "$PWD"
```

需要自定义回调、audience 或 scopes 时追加参数，语义见 `references/register-app.md`。
成功后读取交接块，再加载阶段 3 对应的 reference 写接入代码。

## 阶段 0：探测

```bash
bash "$SKILL_DIR/scripts/preflight.sh"
```

- 退出码 `0`：继续。
- 退出码 `2`：列出阻塞项。修完再继续，不要绕过。

然后读取状态文件（不存在则视为全新）：

```bash
cat .yufu-onboarding/state.env 2>/dev/null || echo "no state"
```

并运行：

```bash
bash "$SKILL_DIR/scripts/verify-issuer.sh"
```

- 通过：记下 `YUFU_ISSUER`，跳到阶段 2。
- 失败：进入阶段 1。

## 阶段 1：部署

加载 `references/deploy-local.md`，然后执行：

```bash
bash "$SKILL_DIR/scripts/deploy-local.sh"
bash "$SKILL_DIR/scripts/verify-issuer.sh"
```

两项都通过才算完成。`deploy-local.sh` 幂等：已就绪时只输出「复用」。
没有源码 checkout 时会下载固定版本并校验，不要求 Rust 或 cargo；
离线环境用 `YUFU_BIN` 指定本地二进制。

## 阶段 2：注册

**只问一个问题**，不要重复询问：

> 你的应用需要让用户通过 yufu 登录，还是只在自己的 API 上校验 yufu 令牌？
> 1. 需要登录 → Web 后端
> 2. 只校验令牌 → API 资源服务器

按回答执行其一：

```bash
bash "$SKILL_DIR/scripts/register-app.sh" --mode web
bash "$SKILL_DIR/scripts/register-app.sh" --mode api
```

脚本会自动完成 csrf → 登录 → 创建应用 → 配对校验。
需要自定义 redirect / audience / scopes 时，先读 `references/register-app.md`。

## 阶段 3：接入

| 选择 | 加载 | 完成判据 |
| --- | --- | --- |
| Web 后端 | `references/go-web.md` | `go build ./...` 通过，登录后能读到 `sub` |
| API 资源服务器 | `references/go-resource-server.md` | `go test ./...` 通过，401/200 行为正确 |

接入时只新增必要文件，并让使用方确认改动范围；
不要重排既有目录，不要引入 Web 框架，不要修改 yufu 服务端或 SDK。

## 阶段 4：验收

加载 `references/verification.md`，按三条行为取证：

1. 登录：`verify-app.sh` 通过，且人工完成一次浏览器登录看到 `sub`。
2. 令牌：生成的 `go test` 通过；无令牌 401、有效令牌 200。
3. 退出：访问 `/logout` 后受保护页重新要求登录。

## 失败处理

加载 `references/troubleshooting.md`，按「阶段 → 症状」定位。
先跑 `verify-issuer.sh` 排除 issuer 层问题，再查应用层。
每个脚本失败时输出阶段名与至少一条可执行排查命令。

## 状态与文件

- 状态：`.yufu-onboarding/state.env`（权限 600，不要提交；把 `.yufu-onboarding/` 加入项目 `.gitignore`）。
- 日志：`.yufu-onboarding/yufu.log`。
- 覆盖目录用 `YUFU_WORKDIR`。

## 禁止

- 不打印、不提交 client secret；secret 只存在于 `.yufu-onboarding/client_secret` 或服务端环境。
- 不使用 `@latest`；SDK 版本固定，来源是 `assets/versions.env`。
- 二进制只从 `assets/versions.env` 固定的 GitHub Release 地址下载并校验 SHA-256，不执行远程脚本。
- 不修改 `yufu/` 服务端代码与 `yufu-sdk-go` 仓库。
- 不把本机回环配置当作生产配置使用。
- 不在未征得同意时删除数据库卷、配置或既有应用。

## 扩展新语言

新增 `references/<lang>-web.md`、`references/<lang>-resource-server.md` 与必要的 `assets/<lang>/`，在本文件的阶段 3 表格加一行。
不要改动阶段 0–2 与验收行为。
