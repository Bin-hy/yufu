# 本机部署

目标：在开发机上得到一个可用的回环 issuer，供注册与接入使用。
不涉及 TLS、域名、反向代理和多机部署。

## 前置

| 依赖 | 用途 | 缺失时 |
| --- | --- | --- |
| `docker` + `docker compose` | 运行 PostgreSQL | 阻塞 |
| `curl`、`python3`、`openssl`、`tar` | 校验、JSON、签名私钥、解包 | 阻塞 |
| 网络或 `YUFU_BIN` | 获取 yufu 二进制 | 二选一 |
| 出站 UDP/123 | 安全时钟校验 | 服务启动失败或 health 503 |

先运行：

```bash
bash "$SKILL_DIR/scripts/preflight.sh"
```

退出码 `0` 可继续，`2` 表示有阻塞项，输出会列出具体缺项。

## 部署

```bash
bash "$SKILL_DIR/scripts/deploy-local.sh"
bash "$SKILL_DIR/scripts/verify-issuer.sh"
```

`deploy-local.sh` 的顺序：

1. 若 `http://127.0.0.1:14000` 的 discovery 已返回同一 issuer，直接复用。
2. 启动（或复用）`postgres_yufu`：仓库内用根 `compose.yaml`，安装副本用 Skill 自带的
   `assets/compose.postgres.yaml`；宿主机端口 `55433`。
3. 数据库不存在则创建 `yufu_onboarding`。
4. 用 `assets/yufu.local.toml.tmpl` 渲染配置到 `.yufu-onboarding/yufu.toml`。
5. 缺少签名私钥时用 `openssl genpkey` 生成 2048 位 PKCS#8 私钥。
6. 选择二进制：`YUFU_BIN` → 检测到的源码产物（不存在时用本地 Rust 工具链构建）
   → 本地缓存 → 按平台下载 `versions.env` 中的固定版本并校验 SHA-256。
7. 后台启动并轮询 discovery，最多 60 秒。

## 来源与校验

二进制和 Go 模块都固定版本，完整性由校验和保证：

- 二进制来自固定地址
  `https://github.com/Bin-hy/yufu/releases/download/<version>/yufu-<version>-<target>.tar.gz`；
  版本与每个平台的 SHA-256 固定在 `assets/versions.env`。脚本只访问该 release 地址，
  不会拉取或执行远程脚本。
- 下载后先校验 SHA-256，不一致时不会写入 `bin/`、不会启动服务。
- 仓库内检测到本地产物时优先使用，不触发下载。
- Go SDK 版本固定在 `assets/versions.env`，脚手架写入生成的 `go.mod`，模块校验和记录在 `go.sum`。
- 压缩包内的 `BUILD-INFO.json` 记录 tag、版本、commit 与目标平台；需要核对来源时，
  把它记录的 commit 与 releases 页面上对应 tag 的提交比对。

当前发布者是个人 GitHub 账号（`Bin-hy`）。校验和能防止内容被替换，但不能替使用者确认发布者身份；
在受监管环境中使用前，请按组织要求评估该来源。

## 可覆盖变量

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `YUFU_PORT` | `14000` | yufu 监听端口 |
| `YUFU_PG_PORT` | `55433` | Postgres 宿主机端口 |
| `YUFU_DB_NAME` | `yufu_onboarding` | 数据库名 |
| `YUFU_ADMIN_USERNAME` | `onboard-admin` | 引导管理员 |
| `YUFU_ADMIN_PASSWORD` | `onboard-admin-password` | 仅限本机 |
| `YUFU_BIN` | 空 | 指定已有二进制（离线可用） |
| `YUFU_TARGET` | 按 `uname` 推导 | 覆盖平台；必须是支持列表之一 |
| `YUFU_WORKDIR` | `./.yufu-onboarding` | 状态、配置、日志、二进制缓存 |

## 就绪判据

三项同时成立才算完成：

- discovery 返回的 `issuer` 与配置逐字符一致；
- JWKS 中至少有一把密钥；
- `/api/v1/health/ready` 返回 200，且 `security_clock_synchronized = true`。

`verify-issuer.sh` 会逐项输出 `[ok]` / `[fail]`。

## 停止与清理

```bash
kill "$(sed -n 's/^YUFU_SERVER_PID=//p' .yufu-onboarding/state.env)"
docker stop postgres_yufu
```

数据库卷保留；需要彻底清理时再删除卷，这属于显式的破坏性操作，先确认。

## 复用规则

重复执行 `deploy-local.sh` 不会重复建库、不会覆盖已有配置，也不会重启已就绪的服务。
只有 discovery 不可用时才会重新启动进程。