# yufu 打包与发版操作手册

本手册说明如何把仓库构建成可分发的安装包，以及如何用这些安装包发布一个版本。
自动流程定义在 [`.github/workflows/release.yml`](../../.github/workflows/release.yml)，
打包与校验逻辑在 [`scripts/release.py`](../../scripts/release.py)；
收到安装包的部署者请阅读[二进制安装说明](installation.md)。

## 两种工作方式

| 方式 | 用途 | 产物 |
| --- | --- | --- |
| GitHub Actions（推送 tag） | 正式发版：四平台原生构建、质量门禁、校验后公开 Release | GitHub Release 附件 |
| 本机演练 | 验证打包链路、检查归档内容、评估升级影响 | 本机 `release-assets/` 目录，不上传 |

本机只能构建宿主平台的原生产物；另外三个平台由 Actions 的矩阵构建发出。
不要把本机演练产物当作跨平台验证结果。

## 前置条件

| 工具 | 版本要求 | 说明 |
| --- | --- | --- |
| Rust | stable（edition 2024） | 需要为每个构建目标安装对应 target |
| Node.js | ≥ 22.23.4 | 前端构建要求，与 CI 一致 |
| pnpm | 与 CI 保持一致（当前 `12.10.1`） | 不同版本会改写锁文件 |
| Python | **≥ 3.11** | 发布脚本使用 `tomllib`；3.9 会直接报 `ModuleNotFoundError` |
| Docker | 可选 | 仅集成测试与容器镜像构建需要 |

确认发布工具本身可用：

```sh
python3 --version                      # 需要 3.11 或更高
python3 -m unittest discover -s scripts/tests -v
python3 scripts/release.py --help
```

`python3` 低于 3.11 时改用 `python3.12 scripts/release.py ...`。

## 1. 准备版本

版本只有一个来源：根 `Cargo.toml` 的 `[workspace.package].version`。
tag 必须与它完全一致，否则预检直接失败。

1. 修改 `Cargo.toml` 中的 `version`（例如 `1.2.3`，预发布写 `1.2.3-rc.1`）。
2. 刷新锁文件中的 workspace 包版本：

   ```sh
   cargo check --workspace
   git diff Cargo.lock     # 应只有 yufu-root / yufu / auth 三个包的版本变化
   ```

3. 校验 tag 与清单、锁文件一致：

   ```sh
   python3 scripts/release.py validate --tag v1.2.3
   ```

   输出类似：

   ```text
   version=1.2.3
   prerelease=false
   ```

4. 提交版本变更，再继续构建。版本号规则：`v主版本.次版本.修订版本`，
   预发布只能追加 `-alpha.N`、`-beta.N` 或 `-rc.N`；不允许多余前导零（`v01.2.3` 非法）。

当前 workspace 版本是 `0.0.0`，因此任何真实 tag 都必须先完成上面这步版本变更。

## 2. 构建二进制

前端必须先构建：`--features webui` 会把 `assets/web` 嵌入二进制，
缺少前端产物时打包会失败。

```sh
pnpm install --frozen-lockfile
pnpm build:web
cargo build --locked --release --bin yufu --features webui
```

产物位置：

| 构建方式 | 二进制路径 |
| --- | --- |
| 本机原生构建 | `target/release/yufu` |
| 指定 `--target <triple>` | `target/<triple>/release/yufu` |

只改了前端、没改 Rust 源码时，先执行 `cargo clean -p yufu` 再构建，
避免把上一次的前端产物嵌进新的二进制。

## 3. 生成安装包

```sh
python3 scripts/release.py package \
  --tag v1.2.3 \
  --target aarch64-apple-darwin \
  --binary target/release/yufu \
  --output release-assets \
  --commit "$(git rev-parse HEAD)"
```

- `--target` 决定包名与归档格式，必须是下表四个之一；二进制本身必须能在当前机器运行。
- `--commit` 必须是完整 40 位提交 SHA，会写进包内的 `BUILD-INFO.json`。
- 脚本会亲自执行 `<binary> --version`，输出必须是 `yufu <version>` 且等于 tag 版本。

### 平台与产物对照

| Rust target | 归档 | 运行环境 |
| --- | --- | --- |
| `x86_64-unknown-linux-gnu` | `yufu-<tag>-x86_64-unknown-linux-gnu.tar.gz` | Ubuntu 22.04 构建，GNU/glibc 动态库 |
| `x86_64-pc-windows-msvc` | `yufu-<tag>-x86_64-pc-windows-msvc.zip` | Windows Server 2022，MSVC |
| `x86_64-apple-darwin` | `yufu-<tag>-x86_64-apple-darwin.tar.gz` | macOS 15 Intel |
| `aarch64-apple-darwin` | `yufu-<tag>-aarch64-apple-darwin.tar.gz` | macOS 15 Apple Silicon |

本机演练请使用宿主平台对应的 target；其余三个目标的名字相同但内容不对应，不能用于发布。

### 包内内容

| 文件 | 说明 |
| --- | --- |
| `yufu` / `yufu.exe` | 内嵌 WebUI 的可执行文件 |
| `yufu.example.toml` | 配置模板 |
| `README.md` | 即 [installation.md](installation.md) 的副本 |
| `yufu.service` | 仅 Linux 包提供，systemd 单元模板 |
| `BUILD-INFO.json` | tag、版本、提交、target 与构建环境记录 |

打包脚本按白名单复制文件，不会打包本地配置、私钥、数据库、`node_modules` 或其他工作区文件；
归档条目、顶层目录名、可执行权限和元数据都会在生成后立即回读校验。

## 4. 汇总校验和

`checksums` 要求目录中恰好是四个平台的安装包：

```sh
python3 scripts/release.py checksums --tag v1.2.3 --output release-assets \
  --commit "$(git rev-parse HEAD)"
```

生成 `release-assets/SHA256SUMS`。它会重新校验每个归档，并要求四包来自同一提交。
手工核对与发布后复核方式：

```sh
# Linux
sha256sum --check --ignore-missing SHA256SUMS
# macOS
shasum -a 256 --check --ignore-missing SHA256SUMS
```

## 5. 发布版本

正式发版只需推送 tag，其余由 Actions 完成：

```sh
git tag -a v1.2.3 -m "Release v1.2.3"
git push origin v1.2.3
```

workflow 顺序：

1. `validate`：跑发布工具单测，校验 tag、清单与锁文件版本。
2. `verify`：控制台 build/test/check/lint、Rust fmt/clippy/全量测试/doctest、
   `cargo machete`、`cargo audit`。任一失败都不会进入构建。
3. `build`：四个平台各自构建前端与二进制，执行 `--version`/`--help` 冒烟测试，打包并上传 artifact。
4. `publish`：下载四个 artifact，生成 `SHA256SUMS`，核对远端 tag 指向的提交，
   创建草稿、上传五个附件、下载回读校验摘要，通过后才公开。

预发布 tag（`-alpha`/`-beta`/`-rc`）自动标记为 prerelease。

### 失败与重试

- 失败后从 Actions 重新运行同一个 tag 的工作流；不要删除或移动 tag。
- 同一提交、同一 tag、带本次流程标记的草稿会被复用，旧附件先清理再上传。
- 已经公开的版本不会被重跑覆盖；需要修复时发新版本号。
- 若草稿被其他流程或人工改动，脚本会拒绝修改并要求人工复核。

## 6. 发布后核对

下载 `SHA256SUMS` 与四个安装包到同一目录并校验，然后：

```sh
tar -xzf yufu-v1.2.3-aarch64-apple-darwin.tar.gz
./yufu-v1.2.3-aarch64-apple-darwin/yufu --version   # 应输出 yufu 1.2.3
cat yufu-v1.2.3-aarch64-apple-darwin/BUILD-INFO.json    # 核对 tag/commit/target
```

## 7. 常见失败

| 现象 | 原因与处理 |
| --- | --- |
| `Tag vX must match Cargo.toml [workspace.package].version` | 未改版本或未提交；按第 1 步修改后重试 |
| `Cargo.lock workspace package ... must match` | 没跑 `cargo check --workspace`，锁文件版本还停在旧值 |
| `Missing WebUI index.html` | 忘记 `pnpm build:web`，或前端构建失败 |
| `Binary version must match tag` | 二进制是改版本之前构建的，重新 `cargo build --release` |
| `ModuleNotFoundError: No module named 'tomllib'` | Python < 3.11，改用 3.11+ |
| `Missing regular package input` | 归档输入缺失，检查 `conf/` 与 `docs/releases/` 是否完整 |
| `Expected exactly four release archives` | `checksums` 只接受四包齐全的目录 |
| `cargo machete` / `cargo audit` 失败 | 依赖未使用或存在公告；修复后再发版，不要跳过门禁 |

## 8. 容器镜像

项目目前不发布预构建镜像。需要容器部署时用
[`deploy/docker/Dockerfile`](../../deploy/docker/Dockerfile) 从本仓库构建，
步骤与 compose 组合见[容器部署](https://yufu-sso.pages.dev/operations/docker/)与
[`deploy/docker/README.md`](../../deploy/docker/README.md)。
