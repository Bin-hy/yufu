# yufu Skill 独立分发 Plan

> 输入：`spec.md`（已批准）
> 输出：可执行的实现设计，供 `task.md` 拆解

## 架构概览

继续沿用四层结构，不新增第五层。变化集中在「运行期资源从仓库内改为 Skill 内 + 固定版本」：

| 层 | 载体 | 本次变化 |
| --- | --- | --- |
| 编排层 | `SKILL.md` | 增加快速路径与 `SKILL_DIR` 约定；描述补触发词 |
| 知识层 | `references/*.md` | 去掉仓库专有路径；部署前置条件改为「下载或本地二进制」 |
| 执行层 | `scripts/*.sh` | 新增 `onboard.sh`；`lib.sh` 增加版本、平台、二进制解析与 Docker 辅助；`preflight`/`deploy` 改用新解析 |
| 素材层 | `assets/` | 新增 `versions.env` 与 `compose.postgres.yaml`；其余模板不变 |

运行期只依赖 Skill 目录、Docker、curl、python3、openssl、Go；不再要求 yufu 源码或 Rust。

## 核心数据结构

### `assets/versions.env`

Shell 可直接 source 的键值文件，随 Skill 发布：

| 键 | 示例 | 用途 |
| --- | --- | --- |
| `YUFU_VERSION` | `v0.1.1` | 独立运行时下载的发布标签；必须是已发布 tag |
| `YUFU_SDK_VERSION` | `v0.0.0-20261114053601-877b814b66a1` | 生成 Go 代码时钉死的 SDK 版本 |
| `YUFU_SHA256_aarch64_apple_darwin` | 64 位十六进制 | macOS ARM 归档校验和 |
| `YUFU_SHA256_x86_64_apple_darwin` | 同上 | macOS Intel |
| `YUFU_SHA256_x86_64_unknown_linux_gnu` | 同上 | Linux x86_64 |

键名由目标三元组把 `-` 替换为 `_` 得到，便于用 `${!key}` 间接展开（bash 3.2 支持）。
不记录 Windows 平台；遇到未列出的平台直接失败并列出支持列表。

### `state.env` 新增键

| 键 | 写入者 | 含义 |
| --- | --- | --- |
| `YUFU_VERSION` | `deploy-local.sh` | 本次实际使用的 yufu 版本 |
| `YUFU_TARGET` | `deploy-local.sh` | 解析出的目标三元组 |
| `YUFU_BINARY` | `deploy-local.sh` | 实际执行的二进制绝对路径 |
| `YUFU_BINARY_SOURCE` | `deploy-local.sh` | `env` / `checkout` / `cache` / `download` |

原有键保持不变；`YUFU_ROOT` 在独立模式下为空，不再写入一个错误路径。

### `lib.sh` 新增函数契约

| 函数 | 输入 | 输出 / 行为 |
| --- | --- | --- |
| `load_versions` | 无 | source `assets/versions.env`；缺键即 `die` |
| `detect_target` | 可选 `YUFU_TARGET` | 默认按 `uname` 映射；显式覆盖必须是支持列表之一，否则 `die`。打印 `aarch64-apple-darwin` / `x86_64-apple-darwin` / `x86_64-unknown-linux-gnu` |
| `detect_source_checkout` | 可选 `YUFU_ROOT` | 打印源码仓库根或空串。显式设置但无效时 `die`；自动探测只认 `<repo>/.agents/skills/<skill>` 向上三级且同时存在 `Cargo.toml`、`compose.yaml`、`src/yufu.rs` |
| `yufu_cached_bin` | 版本、目标 | 打印缓存路径（存在且可执行）或空串 |
| `resolve_yufu_bin` | 无 | 打印最终二进制路径；按优先级选择并记录来源。独立模式下可能触发下载 |
| `compose_cmd` | 无 | 设置 `COMPOSE` 数组（`docker compose` 或 `docker-compose`） |
| `pg_running` | 无 | 容器 `postgres_yufu` 是否运行 |
| `pg_published_port` | 无 | 打印宿主机发布端口；运行中读 `.NetworkSettings.Ports`，已停止读 `.HostConfig.PortBindings` |
| `pg_ready` | 无 | `docker exec postgres_yufu pg_isready -U postgres` |
| `ensure_postgres` | compose 文件、项目名、端口 | 复用 / 启动 / 创建容器，等待就绪，按需建库；不静默重建 |

`resolve_yufu_bin` 优先级：

1. `YUFU_BIN`（必须可执行，否则 `die`）
2. 检测到源码 checkout：`target/debug/yufu` → `target/release/yufu` → 有 cargo 则 `cargo build --bin yufu`；**不下载**
3. `$YUFU_WORKDIR/bin/yufu-$YUFU_VERSION-$TARGET/yufu`
4. 下载 `https://github.com/Bin-hy/yufu/releases/download/$YUFU_VERSION/yufu-$YUFU_VERSION-$TARGET.tar.gz`，校验后安装到第 3 步路径

下载流程：临时目录 → curl → `shasum -a 256` / `sha256sum` 对照 `versions.env` → `tar -xzf` → 只取归档内 `yufu-$VERSION-$TARGET/yufu` → `chmod 700` → `--version` 自检 → 原子移动到缓存。任一步失败清理临时目录，不留下可执行文件。

### `onboard.sh` 命令接口

```text
onboard.sh --mode web|api
           [--redirect-uri URL] [--post-logout-uri URL]
           [--audience AUD] [--scopes "openid profile email"]
           [--app-key KEY] [--project-dir DIR]
```

- `--mode` 必填；其余参数原样传给 `register-app.sh`。
- `--project-dir` 默认当前目录，用于派生稳定 `app_key`。
- 顺序执行四个子步骤，任一失败立即停止并标注阶段；`preflight` 退出码 2 视为阻塞。
- 成功时从 `state.env` 读取并打印交接块（issuer、client_id、secret 文件、state 路径、下一步 reference）。

## 模块设计

### `scripts/onboard.sh`（新增）

**职责：** 单一非交互入口，串联探测 → 部署 → issuer 校验 → 注册与配对校验。
**对外接口：** 上述 CLI。
**依赖：** 其余脚本，全部用 `bash` 显式调用，不依赖可执行位。

阶段标注：

```text
[info] stage: preflight
[info] stage: deploy
[info] stage: verify-issuer
[info] stage: register
```

失败时输出 `[fail] stage: <name>`，保留子脚本原始错误与排查提示。

### `scripts/lib.sh`（修改）

**职责：** 承载所有跨脚本的公共逻辑。新增版本加载、平台识别、源码仓库探测、二进制解析、Docker 与 Postgres 辅助。
**不变：** 日志、state 读写、JSON 解析、`browser_login`。

### `scripts/preflight.sh`（修改）

- 加载 `versions.env`，输出 `yufu: $YUFU_VERSION ($TARGET)`。
- 删除 cargo 阻塞式警告；改为报告二进制预期来源（env / checkout / cache / download）。
- `YUFU_BIN` 已设置但不可执行 → 阻塞。
- 新增 `tar` 与平台支持检查；平台不支持 → 阻塞并列出支持项。
- Postgres 复用判断改用 `pg_published_port`，不再假设 compose 项目归属。
- JSON 输出增加 `yufu_version`、`target`、`binary_source` 字段。

### `scripts/deploy-local.sh`（修改）

- 不再默认 `YUFU_ROOT=../../../..`；改为 `detect_source_checkout`。
- compose 文件：开发模式用 `$YUFU_ROOT/compose.yaml`；独立模式用 `$YUFU_SKILL_DIR/assets/compose.postgres.yaml`，并固定项目名 `yufu-onboarding`。
- 数据库生命周期改用 `ensure_postgres`：
  - 运行中且宿主机端口匹配 → 复用；
  - 运行中但端口不匹配 → 失败并提示用 `YUFU_PG_PORT` 对齐，不强制重建；
  - 已停止且配置端口匹配 → `docker start`；
  - 不存在 → compose 创建。
- 建库改用 `docker exec ... psql/createdb`，与 compose 项目无关。
- 二进制改用 `resolve_yufu_bin`，不再硬编码 `target/debug/yufu`。
- 写入新增 state 键；复用分支同样补齐版本与二进制信息。

### `scripts/register-app.sh` / `verify-app.sh` / `verify-issuer.sh`（微调）

- 内部 `exec "$SCRIPT_DIR/verify-app.sh"` 改为 `exec bash "$SCRIPT_DIR/verify-app.sh"`。
- 其余逻辑不变；注册协议、幂等键、secret 处理保持现状。

### `scripts/scaffold-resource-server.sh`（修改）

- 删除 `examples/sdk/go/go.mod` 读取。
- `YUFU_SDK_VERSION` 未显式设置时从 `assets/versions.env` 读取；文件缺失或无值 → `die`。
- `go mod edit` 与 `go test` 行为不变。

### `SKILL.md`（修改）

1. frontmatter 描述增加中文触发词：「部署 yufu 并接入这个项目」「给项目加 SSO 登录」「用 yufu 保护 API」。
2. 开头声明：本文件所在目录为 `SKILL_DIR`，所有脚本用 `bash "$SKILL_DIR/scripts/..."` 调用。
3. 新增「快速路径」：确认接入形态后直接 `onboard.sh --mode web|api`，再按模式加载 reference；不先解释背景。
4. 阶段 0–4 保留，作为手动分步与排障路径。
5. 「禁止」中 SDK 版本来源改为 `assets/versions.env`。

### `references/*.md`（修改）

| 文件 | 改动 |
| --- | --- |
| `deploy-local.md` | 前置条件去掉 cargo；步骤改为「解析二进制（env / checkout / 缓存 / 下载）」与「Skill 自带 compose」；停止命令改用 `docker stop postgres_yufu`；补充版本与校验和说明 |
| `troubleshooting.md` | `<repo>/compose.yaml` → `docker logs postgres_yufu`；新增下载失败、校验和不符、平台不支持、端口被旧容器占用四行 |
| `go-resource-server.md` | SDK 版本来源改为 `assets/versions.env`，保留 `YUFU_SDK_VERSION` 覆盖 |
| `go-web.md` | 删除指向 `examples/sdk/go/main.go` 的句子，改为「本文件的代码即完整参考实现」 |
| `register-app.md` / `verification.md` | 无需改动 |

### `assets/`（新增）

- `compose.postgres.yaml`：与根 `compose.yaml` 单服务等价；保留 `container_name: postgres_yufu` 与 `${POSTGRES_YUFU_PORT:-5433}` 映射。
- `versions.env`：填入 `v0.1.1` 与三个平台校验和（已从 release 的 `SHA256SUMS` 取得）。

### 仓库级文件（修改 / 新增）

- `README.md`：新增「用 AI Agent 接入」小节，给出带 Skill 标签的安装命令与一句话示例；标题区加 skills.sh badge。
- `scripts/tests/test_skill_distribution.py`（新增）：
  - `YUFU_SDK_VERSION` 必须等于 `examples/sdk/go/go.mod` 中的版本；
  - `YUFU_VERSION` 必须是仓库中存在的 tag；
  - 三个平台校验和齐全且为 64 位十六进制；
  - Skill 目录内不得出现 `examples/sdk` 引用。
  该文件会被现有 `tool-tests`（CI 与 release gate）自动发现。

## 模块交互

### 独立模式（安装到使用方项目）

```text
onboard.sh --mode web
  ├─ preflight.sh（版本/平台/端口/二进制来源）
  ├─ deploy-local.sh
  │    ├─ detect_source_checkout → 空
  │    ├─ ensure_postgres(assets/compose.postgres.yaml, yufu-onboarding, 55433)
  │    └─ resolve_yufu_bin → cache → download → 校验 → --version
  ├─ verify-issuer.sh（issuer / JWKS / health）
  ├─ register-app.sh --mode web [--project-dir]
  │    └─ verify-app.sh（授权端点 + end-session）
  └─ 打印交接块
```

### 开发模式（在 yufu 仓库内）

```text
detect_source_checkout → <repo>
  ├─ compose 文件 = <repo>/compose.yaml
  ├─ 二进制 = <repo>/target/{debug,release}/yufu，缺失且有 cargo 则构建
  └─ 不发起下载；state 记录 source=checkout
```

### 失败路由

| 阶段 | 观测量 | 后续 |
| --- | --- | --- |
| preflight | 平台不支持 / `YUFU_BIN` 无效 / 端口被占 | 直接输出阻塞项，退出 2 |
| deploy · 下载 | HTTP 失败 / 校验和不符 / 归档缺文件 | 提示网络、版本、平台与 `YUFU_BIN` 兜底 |
| deploy · 数据库 | 容器端口不一致 / pg 未就绪 | 输出 `docker inspect` 与 `YUFU_PG_PORT` 对齐建议，不重建 |
| deploy · 服务 | 启动即退出 / discovery 不一致 | 保留现有日志尾部输出 |
| register | 401 / 403 / 409 | 沿用现有提示 |
| verify | redirect / audience / end-session 不符 | 沿用现有具体字段报错 |

## 文件组织

```text
.agents/skills/yufu-onboarding/
├── SKILL.md                       修改：触发词、SKILL_DIR、快速路径
├── references/
│   ├── deploy-local.md            修改：无 cargo、下载与校验、Skill 自带 compose
│   ├── troubleshooting.md         修改：路径与下载故障
│   ├── go-web.md                  修改：去掉 examples 路径
│   └── go-resource-server.md      修改：SDK 版本来源
├── scripts/
│   ├── lib.sh                     修改：版本、平台、二进制、Docker 辅助
│   ├── preflight.sh               修改：来源与平台检查
│   ├── deploy-local.sh            修改：checkout 探测、ensure_postgres、resolve_yufu_bin
│   ├── onboard.sh                 新增：单一入口
│   ├── register-app.sh            修改：bash 调用 verify-app
│   ├── verify-app.sh              不变
│   ├── verify-issuer.sh           不变
│   └── scaffold-resource-server.sh 修改：版本来源
└── assets/
    ├── versions.env               新增
    ├── compose.postgres.yaml      新增
    ├── registration.json.tmpl     不变
    ├── yufu.local.toml.tmpl       不变
    └── go-resource/               不变

README.md                          修改：Skill 安装与一句话示例、badge
scripts/tests/test_skill_distribution.py  新增：版本与路径一致性
docs/specs/yufu-skill-distribution/       新增：本组文档
```

## 技术决策

| 决策点 | 选择 | 理由 |
| --- | --- | --- |
| 仓库布局 | 保留在 yufu 仓库 `.agents/skills/` | CLI 标准扫描路径；单份源码，部署细节与 Skill 同 PR 更新 |
| Skill 发布标签 | 独立标签 `skill-v0.1.0`，与服务器 `v*` 解耦 | Skill 改动不必触发 90 分钟发布流水线；安装命令可固定 |
| 运行时 yufu 版本 | `v0.1.1`（当前唯一已发布版） | 有正式发布归档与三平台校验和；接口兼容性在验收阶段第一步验证 |
| 版本兼容兜底 | 若验收发现 v0.1.1 不满足，先发布 `v0.1.2` 再更新 `versions.env` | 不把未发布版本写进 Skill；发版需单独确认 |
| 完整性校验 | 校验和写入 `versions.env`，而不是运行时取 `SHA256SUMS` | 同源校验和无法防替换；内嵌值可离线验证 |
| 缓存位置 | `<workdir>/bin/yufu-<version>-<target>/` | 项目隔离、随项目清理，已在 `.gitignore` 覆盖 |
| 数据库定义 | Skill 自带单服务 compose | 去掉对仓库 `compose.yaml` 的依赖 |
| 数据库生命周期 | `docker inspect/exec/start`，compose 只负责创建 | 不依赖 compose 项目名，能复用其他来源创建的 `postgres_yufu` |
| 端口不匹配 | 失败并要求对齐，不 `--force-recreate` | 避免摧毁使用者已有数据 |
| 源码探测 | 只认 Skill 上三级 + 三个标记文件 | 不会在使用方项目中误判 |
| 脚本调用 | 一律 `bash <path>` | 安装可能是复制或软链，可执行位不可靠 |
| 一致性检查 | 放进现有 `scripts/tests` | 被 CI `tool-tests` 与 release gate 自动执行，无需改工作流 |
| 部署冒烟 | 仅本机人工验收，不进 CI | health 要求 UDP/123 时钟同步，云 runner 不可靠 |
| Windows | 不承诺 | 脚本基于 bash；发布归档保留但不写进 `versions.env` |
| skills.sh 收录 | 公开仓库 + 安装量自然收录 | 官方无提交接口；README 提供 badge 与安装命令 |

## Spec 覆盖

| 需求 | 归属 |
| --- | --- |
| F1 自包含运行 | `detect_source_checkout`、Skill 内资源引用、README/references 改写 |
| F2 二进制获取 | `resolve_yufu_bin` 四级优先级 |
| F3 下载校验 | `resolve_yufu_bin` 的校验与失败清理 |
| F4 自包含数据库 | `compose.postgres.yaml` + `ensure_postgres` |
| F5 版本固定 | `versions.env` + `test_skill_distribution.py` |
| F6 单一入口 | `onboard.sh` |
| F7 一句话触发 | `SKILL.md` frontmatter + 快速路径 |
| F8 开发模式 | checkout 探测 + 仓库 compose + 本地构建 |
| F9 文档同步 | references 改动表 |
| F10 发布与发现 | `skill-v0.1.0` 标签、README 安装命令与 badge |
| F11 安装完整性 | 统一 `bash` 调用 + 静态路径检查 |
