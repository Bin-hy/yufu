# yufu Skill 独立分发 Tasks

> 输入：`spec.md` + `plan.md`（均已批准）
> 约定：`REPO=/Users/binhy/Binhy-Projects/cobotrix`，`SKILL=$REPO/.agents/skills/yufu-onboarding`。
> 所有脚本用 `bash` 调用；提交代码需用户明确同意，本文件不包含自动提交步骤。

## 文件清单

| 操作 | 文件 | 职责 |
| --- | --- | --- |
| 新建 | `.agents/skills/yufu-onboarding/assets/versions.env` | 版本、校验和、SDK 版本 |
| 新建 | `.agents/skills/yufu-onboarding/assets/compose.postgres.yaml` | 自包含 Postgres 定义 |
| 修改 | `.agents/skills/yufu-onboarding/scripts/lib.sh` | 版本/平台/源码/二进制/Docker 辅助 |
| 修改 | `.agents/skills/yufu-onboarding/scripts/preflight.sh` | 平台与二进制来源检查 |
| 修改 | `.agents/skills/yufu-onboarding/scripts/deploy-local.sh` | checkout、DB 生命周期、二进制解析 |
| 新建 | `.agents/skills/yufu-onboarding/scripts/onboard.sh` | 单一入口 |
| 修改 | `.agents/skills/yufu-onboarding/scripts/register-app.sh` | `bash` 调用 verify-app |
| 修改 | `.agents/skills/yufu-onboarding/scripts/scaffold-resource-server.sh` | SDK 版本来源 |
| 修改 | `.agents/skills/yufu-onboarding/SKILL.md` | 触发词、SKILL_DIR、快速路径 |
| 修改 | `.agents/skills/yufu-onboarding/references/deploy-local.md` | 无 cargo、下载与校验 |
| 修改 | `.agents/skills/yufu-onboarding/references/troubleshooting.md` | 路径与下载故障 |
| 修改 | `.agents/skills/yufu-onboarding/references/go-web.md` | 去掉 examples 路径 |
| 修改 | `.agents/skills/yufu-onboarding/references/go-resource-server.md` | SDK 版本来源 |
| 修改 | `README.md` | 安装命令、一句话示例、badge |
| 新建 | `scripts/tests/test_skill_distribution.py` | 版本与路径一致性测试 |

## T1: 版本与校验和文件

**文件：** `$SKILL/assets/versions.env`
**依赖：** 无
**步骤：**
1. 写入 `YUFU_VERSION=v0.1.1`。
2. 写入 `YUFU_SDK_VERSION=v0.0.0-20261114053601-877b814b66a1`（取自 `examples/sdk/go/go.mod`）。
3. 写入三个平台校验和（取自 v0.1.1 release 的 `SHA256SUMS`）：
   - `YUFU_SHA256_aarch64_apple_darwin=790f6861bf0104ab2ff28e865bff208b7503a53840d47800f3e6c2b8c9620a79`
   - `YUFU_SHA256_x86_64_apple_darwin=9f19598d7935f165e12587cfae57936b9d0f9d902aac98fb5b6f69e023934573`
   - `YUFU_SHA256_x86_64_unknown_linux_gnu=822a91616248cf256648e24fa4d673af79c00f8125ad902045b528750e44bb0f`
4. 文件只含键值，不含注释以外的敏感信息。

**验证：**
```bash
bash -c 'source "$SKILL/assets/versions.env"; \
  for k in YUFU_VERSION YUFU_SDK_VERSION YUFU_SHA256_aarch64_apple_darwin \
           YUFU_SHA256_x86_64_apple_darwin YUFU_SHA256_x86_64_unknown_linux_gnu; do \
    eval "v=\${$k}"; [ -n "$v" ] || { echo "missing $k"; exit 1; }; done; echo ok'
git -C "$REPO" tag --list "$(sed -n 's/^YUFU_VERSION=//p' "$SKILL/assets/versions.env")"
```
期望：输出 `ok`，且 tag 列表包含 `v0.1.1`。

## T2: 自包含 Postgres 定义

**文件：** `$SKILL/assets/compose.postgres.yaml`
**依赖：** 无
**步骤：**
1. 从根 `compose.yaml` 复制 `postgres_yufu` 单服务与 `yufu_pg` 卷。
2. 保留 `container_name: postgres_yufu` 与 `"${POSTGRES_YUFU_PORT:-5433}:5432"`。
3. 不引入其他服务、构建上下文或环境文件。

**验证：**
```bash
POSTGRES_YUFU_PORT=55433 docker compose -f "$SKILL/assets/compose.postgres.yaml" config
```
期望：输出仅含 `postgres_yufu`，`container_name` 为 `postgres_yufu`，端口为 `55433:5432`。

## T3: lib 的版本与平台解析

**文件：** `$SKILL/scripts/lib.sh`
**依赖：** T1
**步骤：**
1. 新增 `load_versions()`：source `$YUFU_SKILL_DIR/assets/versions.env`；校验 `YUFU_VERSION`、`YUFU_SDK_VERSION` 非空。
2. 新增 `detect_target()`：支持 `YUFU_TARGET` 覆盖；默认 `uname -s` / `uname -m` 映射三种组合；其他值输出支持列表并 `die`。
3. 不改变现有日志、state、JSON、登录函数。

**验证：**
```bash
bash -c 'source "$SKILL/scripts/lib.sh"; load_versions; echo "$YUFU_VERSION"; detect_target'
YUFU_TARGET=windows bash -c 'source "$SKILL/scripts/lib.sh"; detect_target'; echo "exit=$?"
```
期望：第一行 `v0.1.1`，第二行为 `aarch64-apple-darwin`（本机）；非法覆盖退出码非零并列出支持项。

## T4: 源码仓库探测与缓存路径

**文件：** `$SKILL/scripts/lib.sh`
**依赖：** T3
**步骤：**
1. 新增 `detect_source_checkout()`：显式 `YUFU_ROOT` 必须同时包含 `Cargo.toml`、`compose.yaml`、`src/yufu.rs`，否则 `die`。
2. 未显式设置时，只检查 `$YUFU_SKILL_DIR/../../..`；三个标记文件齐全才输出该路径，否则输出空串。
3. 新增 `yufu_cached_bin()`：入参版本与目标，输出 `$YUFU_WORKDIR/bin/yufu-<version>-<target>/yufu`（存在且可执行时），否则空串。

**验证：**
```bash
bash -c 'source "$SKILL/scripts/lib.sh"; detect_source_checkout'          # 期望 $REPO
rm -rf /tmp/consumer && mkdir -p /tmp/consumer/.agents/skills
cp -R "$SKILL" /tmp/consumer/.agents/skills/
bash -c 'source /tmp/consumer/.agents/skills/yufu-onboarding/scripts/lib.sh; \
  out=$(detect_source_checkout); [ -z "$out" ] && echo empty'
YUFU_ROOT=/tmp/nope bash -c 'source "$SKILL/scripts/lib.sh"; detect_source_checkout'; echo "exit=$?"
```
期望：`$REPO`、`empty`、最后退出码非零。

## T5: 二进制解析与下载校验

**文件：** `$SKILL/scripts/lib.sh`
**依赖：** T1、T3、T4
**步骤：**
1. 实现 `resolve_yufu_bin()` 四级优先级：`YUFU_BIN` → checkout 产物/构建 → 缓存 → 下载。
2. 下载地址固定为 `https://github.com/Bin-hy/yufu/releases/download/$YUFU_VERSION/yufu-$YUFU_VERSION-$TARGET.tar.gz`。
3. 用 `shasum -a 256` 或 `sha256sum` 对照 `versions.env`；不符立即 `die`。
4. 解压后只取 `yufu-$VERSION-$TARGET/yufu`，`chmod 700`，运行 `--version` 自检，再原子移动到缓存。
5. 任一步失败清理临时目录，不留下可执行文件；网络失败提示 `YUFU_BIN` 兜底。
6. 开发模式不触发下载；cargo 缺失且无产物时直接失败。

**验证：**
```bash
YUFU_BIN=/bin/echo bash -c 'source "$SKILL/scripts/lib.sh"; resolve_yufu_bin'
rm -rf /tmp/yufu-bin-test
YUFU_WORKDIR=/tmp/yufu-bin-test bash -c \
  'source /tmp/consumer/.agents/skills/yufu-onboarding/scripts/lib.sh; \
   p=$(resolve_yufu_bin); echo "$p"; "$p" --version'
ls -l /tmp/yufu-bin-test/bin/*/yufu
```
期望：第一个输出 `/bin/echo`；第二个输出缓存路径，`--version` 以 `yufu ` 开头；文件权限 `700`。

## T6: Docker 与 Postgres 辅助

**文件：** `$SKILL/scripts/lib.sh`
**依赖：** 无
**步骤：**
1. 新增 `compose_cmd()`：设置 `COMPOSE` 数组，优先 `docker compose`，回退 `docker-compose`。
2. 新增 `pg_running()`、`pg_published_port()`、`pg_ready()`。
3. `pg_published_port` 运行中读 `.NetworkSettings.Ports`，已停止读 `.HostConfig.PortBindings`。
4. 新增 `ensure_postgres(compose_file, project, port)`：
   - 运行且端口匹配 → 复用；
   - 运行但端口不匹配 → `die`，提示用 `YUFU_PG_PORT` 对齐；
   - 已停止且配置端口匹配 → `docker start`；
   - 不存在 → `compose -p <project> up -d postgres_yufu`（project 为空时不传 `-p`）；
   - 等待 60 秒 `pg_isready`，按需 `createdb`。
5. 不使用 `--force-recreate`。

**验证：**
```bash
bash -c 'source "$SKILL/scripts/lib.sh"; compose_cmd; echo "${COMPOSE[*]}"; \
  pg_running && echo running || echo stopped; pg_published_port'
docker inspect -f '{{.State.Running}} {{range $p, $conf := .NetworkSettings.Ports}}{{range $conf}}{{println .HostPort}}{{end}}{{end}}' postgres_yufu 2>/dev/null
```
期望：两者对容器的运行状态与端口报告一致；容器不存在时两者都为空/`stopped`。

## T7: preflight 改造

**文件：** `$SKILL/scripts/preflight.sh`
**依赖：** T3、T4、T6
**步骤：**
1. 调用 `load_versions`、`detect_target`、`detect_source_checkout`。
2. 输出 `yufu: <version> (<target>)` 与预期二进制来源（env / checkout / cache / download）。
3. 删除 cargo 警告；新增 `tar` 检查。
4. `YUFU_BIN` 已设置但不可执行 → 阻塞。
5. 平台不支持 → 阻塞并列出支持项。
6. Postgres 复用判断改用 `pg_published_port`。
7. JSON 输出增加 `yufu_version`、`target`、`binary_source`。

**验证：**
```bash
bash "$SKILL/scripts/preflight.sh"
YUFU_BIN=/nonexistent bash "$SKILL/scripts/preflight.sh"; echo "exit=$?"
YUFU_TARGET=windows bash "$SKILL/scripts/preflight.sh"; echo "exit=$?"
```
期望：正常路径以 `[ok] preflight passed` 结束；后两条退出码 `2` 且列出对应阻塞项。

## T8: deploy 的数据库与 compose 改造

**文件：** `$SKILL/scripts/deploy-local.sh`
**依赖：** T2、T4、T6
**步骤：**
1. 删除默认 `YUFU_ROOT=../../../..`，改用 `detect_source_checkout`。
2. 选择 compose 文件：checkout 存在用 `$YUFU_ROOT/compose.yaml`；否则用 `$YUFU_SKILL_DIR/assets/compose.postgres.yaml`，项目名 `yufu-onboarding`。
3. 数据库段落整体替换为 `ensure_postgres`。
4. 建库改用 `docker exec postgres_yufu psql/createdb`。
5. 保持配置渲染、私钥生成、60 秒等待逻辑不变。

**验证：**
```bash
pid=$(sed -n 's/^YUFU_SERVER_PID=//p' "$REPO/.yufu-onboarding/state.env" 2>/dev/null); [ -n "$pid" ] && kill "$pid" || true
docker stop postgres_yufu 2>/dev/null || true
bash "$SKILL/scripts/deploy-local.sh"
bash "$SKILL/scripts/verify-issuer.sh"
bash "$SKILL/scripts/deploy-local.sh"
```
期望：第一次启动容器并输出 `yufu ready`；verify 三项 `[ok]`；第二次输出 `reusing yufu already listening`。

## T9: deploy 的二进制与状态改造

**文件：** `$SKILL/scripts/deploy-local.sh`
**依赖：** T5、T8
**步骤：**
1. 用 `resolve_yufu_bin` 替换硬编码 `target/debug/yufu` 与 cargo 构建分支。
2. 记录 `YUFU_VERSION`、`YUFU_TARGET`、`YUFU_BINARY`、`YUFU_BINARY_SOURCE`。
3. 复用 issuer 的分支同样补齐上述键。
4. `remember()` 不再写入无效的 `YUFU_ROOT`。

**验证：**
```bash
bash "$SKILL/scripts/deploy-local.sh"
grep -E '^YUFU_(VERSION|TARGET|BINARY|BINARY_SOURCE)=' "$REPO/.yufu-onboarding/state.env"
"$(sed -n 's/^YUFU_BINARY=//p' "$REPO/.yufu-onboarding/state.env")" --version
```
期望：`SOURCE=checkout`（仓库内），路径指向 `target/debug/yufu`，`--version` 以 `yufu ` 开头。

## T10: 注册、校验与脚手架调整

**文件：** `$SKILL/scripts/register-app.sh`、`verify-app.sh`、`scaffold-resource-server.sh`
**依赖：** T1、T5
**步骤：**
1. `register-app.sh` 的两处 `exec "$SCRIPT_DIR/verify-app.sh"` 改为 `exec bash "$SCRIPT_DIR/verify-app.sh"`。
2. `scaffold-resource-server.sh` 删除 `examples/sdk/go/go.mod` 读取。
3. SDK 版本：`YUFU_SDK_VERSION` 环境变量优先，否则从 `assets/versions.env` 读取；缺失即 `die`。
4. `verify-issuer.sh` 不改。

**验证：**
```bash
bash -n "$SKILL/scripts/register-app.sh" "$SKILL/scripts/verify-app.sh" "$SKILL/scripts/scaffold-resource-server.sh"
tmp=$(mktemp -d); (cd "$tmp" && go mod init example.com/auth >/dev/null && \
  bash "$SKILL/scripts/scaffold-resource-server.sh" --dir ./internal/auth --pkg auth)
grep 'github.com/Bin-hy/yufu-sdk-go' "$tmp/go.mod"
```
期望：语法检查通过；`go test` 通过；`go.mod` 中的版本等于 `versions.env` 的 `YUFU_SDK_VERSION`。

## T11: 单一入口 onboard.sh

**文件：** `$SKILL/scripts/onboard.sh`
**依赖：** T7、T9、T10
**步骤：**
1. 解析 `--mode web|api`、redirect/post-logout/audience/scopes/app-key/project-dir。
2. 依次执行 `preflight` → `deploy` → `verify-issuer` → `register`，每步前打印 `[info] stage: ...`。
3. 子脚本失败时输出 `[fail] stage: <name>` 并透传退出码；preflight 退出码 2 直接停止。
4. 成功时从 state 读取并打印 issuer、client_id、secret 文件、state 路径、下一步 reference。
5. 不执行任何代码接入，不修改使用方文件。

**验证：**
```bash
bash "$SKILL/scripts/onboard.sh" --mode bogus; echo "exit=$?"
mkdir -p /tmp/onboard-verify && cd /tmp/onboard-verify
bash "$SKILL/scripts/onboard.sh" --mode api --project-dir /tmp/onboard-verify
```
期望：第一条非零并提示 `--mode`；第二条完成四阶段并打印交接块，state 中出现 `YUFU_CLIENT_ID`。

## T12: SKILL.md 触发与快速路径

**文件：** `$SKILL/SKILL.md`
**依赖：** T11
**步骤：**
1. frontmatter 描述增加：「部署 yufu 并接入这个项目」「给项目加 SSO 登录」「用 yufu 保护 API」。
2. 开头声明 `SKILL_DIR` 为当前文件目录，所有脚本用 `bash "$SKILL_DIR/scripts/..."`。
3. 新增「快速路径」：确定形态后直接 `onboard.sh --mode web|api`，再加载对应 reference。
4. 阶段 0–4 保留为分步与排障路径。
5. 「禁止」中的 SDK 版本来源改为 `assets/versions.env`。

**验证：**
```bash
grep -nE 'onboard\.sh|SKILL_DIR|versions\.env' "$SKILL/SKILL.md"
grep -n 'examples/sdk' "$SKILL/SKILL.md"; echo "exit=$?"
```
期望：命中快速路径与版本来源；`examples/sdk` 无命中（退出码 1）。

## T13: 部署类 reference 同步

**文件：** `references/deploy-local.md`、`references/troubleshooting.md`
**依赖：** T9、T11
**步骤：**
1. `deploy-local.md` 前置条件删除 cargo，改为「下载固定版本或 `YUFU_BIN`」。
2. 部署步骤改为二进制解析四级优先级与 Skill 自带 compose。
3. 停止命令改为 `docker stop postgres_yufu`。
4. `troubleshooting.md` 把 `<repo>/compose.yaml` 改为 `docker logs postgres_yufu`。
5. 新增四行：下载失败、校验和不符、平台不支持、端口被旧容器占用。

**验证：**
```bash
grep -nE '<repo>/compose|cargo|examples/sdk' "$SKILL/references/deploy-local.md" "$SKILL/references/troubleshooting.md"; echo "exit=$?"
grep -n 'docker logs postgres_yufu' "$SKILL/references/troubleshooting.md"
```
期望：第一条无旧表述（退出码 1）；第二条命中。

## T14: 接入类 reference 同步

**文件：** `references/go-web.md`、`references/go-resource-server.md`
**依赖：** T10
**步骤：**
1. `go-web.md` 删除「完整可运行版本见 `examples/sdk/go/main.go`」，改为「本文件的代码即完整参考实现」。
2. `go-resource-server.md` 把 SDK 版本来源改为 `assets/versions.env`，保留 `YUFU_SDK_VERSION` 覆盖说明。

**验证：**
```bash
grep -n 'examples/sdk' "$SKILL/references"/*.md; echo "exit=$?"
grep -n 'versions.env' "$SKILL/references/go-resource-server.md"
```
期望：第一条无命中；第二条命中。

## T15: README 安装入口与 badge

**文件：** `README.md`
**依赖：** T11、T12
**步骤：**
1. 标题区增加 `[![skills.sh](https://skills.sh/b/Bin-hy/yufu)](https://skills.sh/Bin-hy/yufu)`。
2. 新增「用 AI Agent 接入（Skill）」小节，给出：
   ```bash
   npx skills add Bin-hy/yufu@skill-v0.1.0 --skill yufu-onboarding
   ```
3. 给出非交互写法（`-a claude-code --copy -y`）与一句话示例。
4. 不承诺生产部署，不写未发布的 tag。

**验证：**
```bash
grep -n 'npx skills add Bin-hy/yufu@skill-v0.1.0' README.md
grep -n 'skills.sh/b/Bin-hy/yufu' README.md
```
期望：两条均命中，且安装命令中的 tag 与 T23 创建的标签一致。

## T16: 一致性测试

**文件：** `scripts/tests/test_skill_distribution.py`
**依赖：** T1、T11、T13、T14
**步骤：**
1. 新增 unittest，定位仓库根与 Skill 目录。
2. 用例一：`versions.env` 的 `YUFU_SDK_VERSION` 等于 `examples/sdk/go/go.mod` 中的版本。
3. 用例二：`YUFU_VERSION` 在 `git tag --list` 中。
4. 用例三：三个平台校验和存在且匹配 `^[0-9a-f]{64}$`。
5. 用例四：Skill 目录内（脚本、文档、模板）不出现 `examples/sdk`。
6. 不引入第三方依赖。

**验证：**
```bash
python3 -m unittest discover -s "$REPO/scripts/tests" -v
python3 "$REPO/scripts/readiness.py" run tool-tests
```
期望：全部通过；新增用例出现在输出中。

## T17: 本地安装冒烟（无网络依赖）

**文件：** 无（临时目录）
**依赖：** T12、T15
**步骤：**
1. 在 `/tmp/skill-install-check` 初始化空项目。
2. 从本地仓库安装：
   ```bash
   npx skills add "$REPO" --skill yufu-onboarding -a claude-code --copy -y
   ```
3. 确认安装目录与 `skills-lock.json`。
4. 用 `bash` 运行安装副本的 `preflight.sh`。

**验证：**
```bash
cd /tmp/skill-install-check && npx skills list
grep -rnE 'examples/sdk|\.\./\.\./\.\./\.\.' .agents/skills/yufu-onboarding | head
bash .agents/skills/yufu-onboarding/scripts/preflight.sh
```
期望：`skills list` 显示 `yufu-onboarding`；静态扫描无命中；preflight 通过并报告版本与下载来源。

## T18: 独立部署端到端（下载路径 + 幂等）

**文件：** 无（临时目录）
**依赖：** T17
**步骤：**
1. 停止仓库内正在运行的 yufu 进程，确保部署不会提前复用 issuer。
2. 清空 `/tmp/skill-install-check` 的 `.yufu-onboarding`。
3. 执行 `bash .agents/skills/yufu-onboarding/scripts/onboard.sh --mode api --project-dir /tmp/skill-install-check`。
4. 记录二进制来源、版本、issuer、client_id。
5. 重复执行同一条命令，确认复用。

**验证：**
```bash
cd /tmp/skill-install-check
bash .agents/skills/yufu-onboarding/scripts/onboard.sh --mode api --project-dir "$PWD"
grep -E '^YUFU_(BINARY_SOURCE|VERSION|BINARY)=' .yufu-onboarding/state.env
bash .agents/skills/yufu-onboarding/scripts/onboard.sh --mode api --project-dir "$PWD"
```
期望：首次 `BINARY_SOURCE=download`，`VERSION=v0.1.1`；四阶段全部 `[ok]`；第二次输出复用且不新建应用。
若首次失败：记录具体阶段，按「风险」处理 v0.1.1 兼容性，不跳过。

## T19: API 路径接入验证

**文件：** `internal/auth/`（临时项目内）
**依赖：** T18
**步骤：**
1. 在 `/tmp/skill-install-check` 执行 `go mod init example.com/check`（若尚未初始化）。
2. 运行安装副本的 `scaffold-resource-server.sh --dir ./internal/auth --pkg auth`。
3. 运行 `go test ./...`。
4. 用已注册 client 启动一个最小受保护接口（可直接用测试覆盖，不强制起服务）。

**验证：**
```bash
cd /tmp/skill-install-check
bash .agents/skills/yufu-onboarding/scripts/scaffold-resource-server.sh --dir ./internal/auth --pkg auth
go test ./...
```
期望：测试覆盖无令牌、有效令牌、audience 不符、过期四种情况且全部通过。

## T20: Web 路径构建验证

**文件：** `/tmp/yufu-web-smoke/`（临时项目）
**依赖：** T18
**步骤：**
1. 按 `references/go-web.md` 落四个文件，`go mod init` 并 `go get github.com/Bin-hy/yufu-sdk-go@<versions.env 版本>`。
2. 从 state 读取 issuer、client id、secret 启动服务。
3. 构建并访问 `/login`。
4. 人工完成一次浏览器登录，确认 `/api/me` 返回稳定 `sub`；不自动驱动浏览器。

**验证：**
```bash
cd /tmp/yufu-web-smoke && go build ./...
curl -sS -o /dev/null -w '%{http_code} %{redirect_url}\n' http://127.0.0.1:8080/login
```
期望：构建通过；`/login` 返回 `302` 且跳转到 issuer 的 `/oidc/authorize`；浏览器登录结果人工记录。

## T21: 失败路径验证

**文件：** 无（临时副本）
**依赖：** T5、T7
**步骤：**
1. 校验和不符：复制 Skill 到 `/tmp/skill-bad-sum`，把当前平台校验和改为 64 个 `0`，清空 workdir，执行 `resolve_yufu_bin`。
2. 网络失败：设置 `HTTPS_PROXY=http://127.0.0.1:9`，清空 workdir，执行 `resolve_yufu_bin`。
3. 平台不支持：`YUFU_TARGET=windows` 运行 `preflight.sh`。
4. `YUFU_BIN` 无效：指向不存在的文件运行 `preflight.sh`。

**验证：**
```bash
bash -c 'source /tmp/skill-bad-sum/scripts/lib.sh; resolve_yufu_bin'; echo "exit=$?"
HTTPS_PROXY=http://127.0.0.1:9 bash -c \
  'source /tmp/consumer/.agents/skills/yufu-onboarding/scripts/lib.sh; \
   YUFU_WORKDIR=/tmp/yufu-offline; resolve_yufu_bin'; echo "exit=$?"
```
期望：均非零；校验和不符发生在启动服务前；网络失败提示 `YUFU_BIN`；无缓存二进制残留。

## T22: 全量检查

**文件：** 无
**依赖：** T16、T19、T21
**步骤：**
1. 所有 shell 脚本 `bash -n`。
2. 运行 `python3 scripts/readiness.py run tool-tests`。
3. 搜索 Skill 内是否残留 monorepo 专有引用。
4. 确认 `.gitignore` 仍覆盖 `.yufu-onboarding/`。

**验证：**
```bash
bash -n "$SKILL"/scripts/*.sh
python3 "$REPO/scripts/readiness.py" run tool-tests
grep -rnE 'examples/sdk|<repo>/compose' "$SKILL" --include='*.sh' --include='*.md' --include='*.env'; echo "exit=$?"
grep -n '.yufu-onboarding/' "$REPO/.gitignore"
```
期望：语法与测试全部通过；静态搜索无命中；gitignore 命中。

## T23: 发布标签与公网安装验证

**文件：** 仓库标签
**依赖：** T22
**步骤：**
1. 本地创建标签：`git tag -a skill-v0.1.0 -m "yufu-onboarding skill v0.1.0"`。
2. **推送到远端前必须获得用户确认**（共享状态操作）。
3. 推送后，在空目录执行标签安装：
   ```bash
   npx skills add Bin-hy/yufu@skill-v0.1.0 --skill yufu-onboarding -a claude-code --copy -y
   ```
4. 记录 skills.sh 页面状态；索引可能延迟，记录实际结果。
5. 若 T18 证明 v0.1.1 不兼容，本任务改为先发布 `v0.1.2` 并更新 `versions.env`，同样需确认。

**验证：**
```bash
git -C "$REPO" show skill-v0.1.0:.agents/skills/yufu-onboarding/SKILL.md | head -4
cd /tmp/tag-install-check && npx skills add Bin-hy/yufu@skill-v0.1.0 --skill yufu-onboarding -a claude-code --copy -y
npx skills list
```
期望：标签内容包含新 Skill；公网安装成功；`skills list` 显示 `yufu-onboarding`。

## T24: 按 checklist 验收

**文件：** `docs/specs/yufu-skill-distribution/checklist.md`（待生成）
**依赖：** T1–T23
**步骤：**
1. 四份文档全部批准后，逐项执行 checklist。
2. 记录命令输出、退出码与观察结果作为证据。
3. 不通过的项修复后重跑。

**验证：** 最终报告逐项列出通过/未通过与证据。

## 执行顺序

```text
T1 → T2 → T3 → T4 → T5 → T6
              ↘   ↘   ↓
               T7 → T8 → T9 → T10 → T11 → T12
                                      ↓      ↓
                                     T13    T15
                                      ↓
                                     T14 → T16
                                            ↓
                                           T17 → T18 → T19
                                                   ↘   ↓
                                                    T20 T21 → T22 → T23 → T24
```

T13/T14 与 T15 可并行；T19/T20 共用 T18 的环境，但需分别取证。

## 风险与回退

| 风险 | 触发 | 回退 |
| --- | --- | --- |
| v0.1.1 缺少 Skill 依赖的接口 | T18 注册或校验失败 | 发布 `v0.1.2`，更新 `versions.env` 与校验和，重跑 T18 |
| 下载被墙或受限 | T18 失败 | 文档与错误信息指向 `YUFU_BIN`；不改为源码构建 |
| 安装器丢弃可执行位 | T17 运行失败 | 全链路已统一 `bash <path>`，应不受影响；若仍失败，检查调用方式 |
| 标签推送或 skills.sh 索引延迟 | T23 | 标签安装不依赖索引；索引状态如实记录，不强求即时可见 |
| 旧容器端口不一致 | T8/T18 | 提示用 `YUFU_PG_PORT` 对齐；不自动重建 |
