# Tag 自动打包与发版 Plan

## 输入与范围

依据已批准的 [spec.md](spec.md)。新增独立发布工作流，不修改业务 Rust 代码和现有 CI。
本文件为技术设计，待批准后再编写任务清单。

## 架构概览

发布流程由四层组成：版本预检 → 质量门禁 → 原生平台构建与打包 → Release 草稿上传与公开。
每个 job 都检出事件指向的提交，禁用 checkout 凭据持久化；默认仓库权限为 `contents: read`。
仅最终发布 job 获得 `contents: write`，使用自动提供的 `GITHUB_TOKEN`，不要求配置个人访问令牌。

### 工作流：`.github/workflows/release.yml`

- 监听 `push.tags: ['v*']`，预检脚本进一步严格校验版本，拒绝非法输入。
- 不提供手动发版入口；失败重试使用 GitHub Actions 的 Re-run。
- `concurrency` 按工作流名与完整 tag ref 分组，`cancel-in-progress: false`。
- 任一前置 job 失败，发布 job 不执行；所有 job 设置合理超时。
- checkout、setup-node、setup-python、artifact 等 Actions 在实现时选定官方版本并固定提交 SHA，在注释中保留版本标识。
- Rust stable 与 Node 24；pnpm 固定为现有 CI 的 `12.10.1`。依赖安装用 `--frozen-lockfile`，Cargo 构建及检查用 `--locked`。

### 发布工具：`scripts/release.py`

Python 3.11 标准库实现版本解析、打包、解包检查及校验文件生成；发布 API 操作由最终 job 内的 GitHub CLI 执行。
脚本采用显式子命令，输入经参数解析后处理，不拼接可执行 shell 字符串。

| 子命令 | 输入 | 输出与约束 |
| --- | --- | --- |
| `validate` | tag、仓库根目录 | 版本、是否 prerelease；校验 workspace 版本以及 Cargo.lock 内三个 workspace 包的版本 |
| `package` | tag、target、二进制路径、输出目录 | 单个平台压缩包；执行二进制 `--version`，检查 WebUI 入口及静态资源已构建 |
| `checksums` | tag、产物目录 | 只接受预期四个安装包，验证目录内容并生成 `SHA256SUMS` |

版本解析只接受 `vX.Y.Z`、`vX.Y.Z-alpha.N`、`vX.Y.Z-beta.N`、`vX.Y.Z-rc.N`，数字不允许多余前导零。
去掉 `v` 后须与 `[workspace.package].version` 完全一致。
当前 workspace 版本为 `0.0.0`；本次不改版本，发布前由维护者准备真实版本并更新锁文件。

### 自动化验证：`scripts/tests/test_release.py`

使用 `unittest`，通过临时目录和虚构文件验证合法/非法 tag、workspace/lockfile 不一致、归档白名单、Unix 可执行位、路径安全、缺包/多包、损坏归档及校验和。
用可控的替身二进制或 mock 覆盖版本输出匹配和失败情况；真实二进制 smoke test 在平台构建 job 执行。

## Job 设计

### 1. `validate`：版本预检

Ubuntu runner 安装 Python 3.11，运行发布工具测试与 tag/manifest/lockfile 校验。
将已校验版本和 prerelease 布尔值写到 job outputs。
这里不申请写权限，也不创建 Release。

### 2. `verify`：质量门禁

依赖 `validate`，在 Ubuntu 执行：

1. 安装 Rust（rustfmt、Clippy）、Node、pnpm 与 Python，安装锁定前端依赖。
2. 构建前端，执行控制台 test/check/lint。
3. 安装现有集成测试所需 Playwright 三引擎及系统依赖；隔离 PostgreSQL 由 testcontainers 管理。
4. 执行 Rust fmt、Clippy、workspace 全 target/feature 测试及 doctest，集成测试串行运行。
5. 执行 cargo-machete 与 cargo-audit；发现问题时阻止发布，不新增忽略名单。

不重构现有 main/PR CI，也不把历史 CI 状态作为本 tag 的放行依据。

### 3. `build`：四平台原生构建

依赖 `verify`，矩阵 `fail-fast: false`，每个平台独立构建前端并嵌入。

| Runner | Rust target | 安装包后缀 |
| --- | --- | --- |
| `ubuntu-22.04` | `x86_64-unknown-linux-gnu` | `.tar.gz` |
| `windows-2022` | `x86_64-pc-windows-msvc` | `.zip` |
| `macos-15-intel` | `x86_64-apple-darwin` | `.tar.gz` |
| `macos-15` | `aarch64-apple-darwin` | `.tar.gz` |

每个平台安装对应 Rust target，执行 `cargo build --locked --release --bin yufu --features webui --target <target>`。
执行二进制 `--version` 与 `--help`，前者须输出 `yufu <tag 去掉 v>`，无需启动数据库或认证时钟。
记录操作系统、Rust/Node/pnpm 版本、tag 与提交；Linux 记录动态依赖，发布说明明确为 GNU/glibc 构建，未提供静态链接保证。

安装包命名：`yufu-<tag>-<target>.<tar.gz|zip>`。
每个包有同名顶层目录，内容白名单如下：

```text
yufu-<tag>-<target>/
├── yufu[.exe]
├── yufu.example.toml
├── README.md                  # 面向二进制使用者的安装说明
├── BUILD-INFO.json            # tag、版本、提交、target、构建环境
└── yufu.service           # 仅 Linux，附路径与权限配置说明
```

`README.md` 从受版本管理的 `docs/releases/installation.md` 复制，配置模板沿用 `conf/yufu.example.toml`。
不把整个仓库、conf 目录或 assets 目录打进包；前端已嵌入可执行文件。
打包后重新检查归档条目、必需文件、Unix 权限及 JSON 元数据，上传单个平台 workflow artifact。

### 4. `publish`：汇总与发布

依赖全部 `build`，在 Ubuntu 下载当前 run 的四个 artifact。通过 `checksums` 拒绝缺失、多余或版本/target 不一致的包。

1. 检查远端 tag 解析出的提交仍等于本 run 提交，发现 tag 被移动则失败。
2. 查询 tag 对应 Release：404 才视作不存在；认证或网络失败直接终止。
3. 若已公开则失败，禁止覆盖。若为本流程创建且含相同提交标识的草稿，则复用；其他草稿拒绝修改。
4. 创建或更新草稿，说明中写入来源 tag、提交、自动变更记录、安装与校验提示，并标注四平台及未签名状态。
5. 清理本流程相同提交的草稿旧附件，上传四包和 `SHA256SUMS`，复核附件名称与数量；下载附件到临时目录并校验 SHA-256。
6. 完整性验证成功后公开草稿，设置 stable/prerelease 类型。失败保留草稿，允许同一提交重新运行。

草稿流程防止半套产物被公开；GitHub API 上传不是事务操作，公开前必须显式复核。
写权限仅在本 job 生效，创建草稿后才进行附件写入。

## 文档与集成

- 根 README 保留已经完成的专业版改写，增加 Release 下载入口、平台表与 tag 发布步骤。
- `docs/releases/installation.md`：二进制解包、配置、初次启动、SHA-256 校验、Linux systemd 路径、Windows/macOS 手动运行及无签名说明。
- README 明确：先修改 workspace 版本并更新锁文件，通过检查并提交，再创建和推送匹配 tag；本次不实际执行这些发布动作。
- 当前 `conf/yufu.service` 使用 `/usr/bin/yufu` 和 `/etc/yufu/yufu.toml`，安装说明与现有路径保持一致。

## 需求归属

| Spec | 设计归属 |
| --- | --- |
| F1、F2 | `validate` 与发布工具版本校验 |
| F3 | `verify` 质量门禁 |
| F4、F5、F6 | `build` 矩阵、归档白名单与安装说明 |
| F7 | 各平台 smoke test、归档复核与 `checksums` |
| F8、F9、F10 | `publish` 草稿流程、完整性校验、并发组与拒绝覆盖 |
| F11 | 根 README 与安装说明 |
| N1—N5 | job 权限、锁文件、参数隔离、元数据及独立工作流 |

## 验证与交付边界

本地执行发布工具单测、actionlint、配置静态检查及可用平台的实际打包检查。
跨平台构建、完整 Rust 门禁和 GitHub Release API 的实际运行，以首次匹配版本 tag 的 Actions 日志为准；本次不伪造 tag 或创建 Release 来声称已完成远端验收。
若本地依赖、安全审计或其他门禁发现阻塞，记录实际结果，不擅自放宽检查。

## 官方参考

- [GitHub-hosted runner labels 与架构](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
- [GitHub Release API](https://docs.github.com/en/rest/releases/releases)
- [GitHub CLI release create](https://cli.github.com/manual/gh_release_create)
