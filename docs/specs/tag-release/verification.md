# Tag 自动打包与发版验证记录

对应 [spec.md](spec.md)、[plan.md](plan.md)、[task.md](task.md) 与 [checklist.md](checklist.md)。
本文件只记录**实际执行过**的验证；未执行的项目明确标注为待远端验证，不以静态配置代替运行结果。

## 环境与范围

| 项目 | 值 |
| --- | --- |
| 记录日期 | 2026-11-12 |
| 提交 | `05f691255c4e9ef9195ce98e5d26202808fd377d`（工作区含未提交改动） |
| 平台 | macOS `macOS-26.6.2-arm64`（Apple Silicon） |
| Rust | `rustc 1.97.1 (8bab26f4f 2026-07-14)` |
| Python | `3.12.16`（系统自带 `python3` 为 3.9.6，不支持 `tomllib`） |
| Node / pnpm | Node `v22.22.2`，pnpm `11.8.0`（本地；CI 固定 pnpm `12.10.1`） |
| actionlint | `1.7.7` |
| Docker | `29.3.1`，Compose `v5.1.1` |

未在本机执行：Windows、Linux 构建，GitHub 托管 runner，真实 GitHub Release 写入。

## 已执行并通过

### 版本与触发（C01—C05）

`python3.12 -m unittest discover -s scripts/tests -v`：**18 个用例全部通过**，覆盖稳定/预发布版本、
非法 tag、manifest 与 lockfile 不一致、归档白名单、摘要与发布状态机。

对当前仓库数据直接调用 CLI：

```text
v0.0.0            -> version=0.0.0 prerelease=false
v0.0.1-rc.1       -> Release failed: Tag v0.0.1-rc.1 must match Cargo.toml [workspace.package].version; ...
1.2.3             -> Release failed: Expected vX.Y.Z or vX.Y.Z-{alpha,beta,rc}.N; no leading zeros
v01.2.3           -> Release failed: Expected vX.Y.Z or vX.Y.Z-{alpha,beta,rc}.N; no leading zeros
v1.2.3-rc         -> Release failed: Expected vX.Y.Z or vX.Y.Z-{alpha,beta,rc}.N; no leading zeros
v1.2.3-zb.1       -> Release failed: Expected vX.Y.Z or vX.Y.Z-{alpha,beta,rc}.N; no leading zeros
'v1.2.3;echo pwned' -> Release failed: Expected vX.Y.Z or vX.Y.Z-{alpha,beta,rc}.N; no leading zeros
```

结论：合法 tag 输出版本与 prerelease 标记；非法 tag 与版本不一致在构建前即被拒绝，且未进入 shell。
`v0.0.0` 能通过是因为它与当前 workspace 版本一致，属于正常的版本来源校验结果。

工作流触发为 `push: tags: ['v*']`，分支 push 与 PR 不进入本流程；`concurrency` 以 `release-${{ github.ref }}`
分组且 `cancel-in-progress: false`（静态核对）。

### 质量门禁与权限（C06、C31—C34）

- `verify` job 依次执行控制台 `build`/`test`/`check`/`lint`，Rust `fmt`、`clippy --all-targets --all-features`、
  串行 `test`、`doc` 测试，以及 `cargo machete`、`cargo audit`；无 `continue-on-error`。
- 顶层 `permissions: contents: read`，仅 `publish` 提升为 `contents: write`；所有 checkout 均
  `persist-credentials: false`。
- 固定的第三方 Action 提交 SHA **逐个核对到上游 tag**：

  | Action | SHA | 上游 tag |
  | --- | --- | --- |
  | `actions/checkout` | `d23441a4…` | `v6`, `v6.1.0` |
  | `actions/setup-node` | `249970729…` | `v6`, `v6.5.0` |
  | `actions/setup-python` | `ece7cb06…` | `v6`, `v6.3.0` |
  | `actions/upload-artifact` | `ea165f8d…` | `v4`, `v4.6.2` |
  | `actions/download-artifact` | `d3f86a10…` | `v4`, `v4.3.0` |
  | `dtolnay/rust-toolchain` | `b725eda…` | `stable`（移动分支） |

- 工具版本存在性核对：`pnpm@12.10.1`、`cargo-machete 0.9.2`、`cargo-audit 0.23.0` 均为当前可用版本。

### 本机构建与打包（C12—C18、C20、E04）

```sh
pnpm build:web
cargo build --locked --release --bin yufu --features webui
./target/release/yufu --version     # yufu 0.0.0
python3.12 scripts/release.py package --tag v0.0.0 --target aarch64-apple-darwin \
  --binary target/release/yufu --output /tmp/release-assets --commit "$(git rev-parse HEAD)"
```

产物 `yufu-v0.0.0-aarch64-apple-darwin.tar.gz` 内容：

```text
yufu-v0.0.0-aarch64-apple-darwin/BUILD-INFO.json     (0644)
yufu-v0.0.0-aarch64-apple-darwin/README.md           (0644)
yufu-v0.0.0-aarch64-apple-darwin/yufu            (0755)
yufu-v0.0.0-aarch64-apple-darwin/yufu.example.toml (0644)
```

`BUILD-INFO.json` 的 `tag`/`version`/`commit`/`target` 与发布上下文一致，`environment` 记录了
`os`/`python`/`rust`（本地未设 `RELEASE_NODE_VERSION`/`RELEASE_PNPM_VERSION`，故为 `not recorded`）。
macOS 包不含 systemd 模板，符合白名单。解包后执行 `--version` 输出 `yufu 0.0.0`。
独立计算摘要：`e402f3db7444965448c08678bc140337dc4c4ba05ef6aa386978666e15ead63f`。
只放一个包时 `checksums` 按预期失败：`Expected exactly four release archives; found [...]`。

### 静态检查（C36）

```text
python3.12 -m py_compile scripts/release.py scripts/tests/test_release.py   # OK
python3.12 scripts/release.py --help                                        # OK
git diff --check                                                            # OK
```

`actionlint .github/workflows/release.yml`：初次运行时只报告
`label "macos-15-intel" is unknown`。这是 actionlint 1.7.7 的内置标签表落后于 GitHub：
`macos-15-intel` 是 GitHub 在 `macos-13` 退役后提供的 Intel macOS 托管 runner 标签，
更新的 actionlint 版本已收录。工作流引用的 runner 没有问题，因此新增
`.github/actionlint.yaml` 声明该标签（配置文件是 actionlint 文档给出的扩展方式），
之后 `actionlint`（全部工作流）退出码为 0。

## 未执行（待验证）

| 项目 | 原因 |
| --- | --- |
| C07、C08、C09 的实际执行结果 | 本机未跑完整 workspace 测试、Playwright 与 `cargo audit`；由 Actions 执行 |
| C13/C19 的 Windows `.zip` 与 Linux `tar.gz` 产物 | 需在对应平台或 Actions 矩阵构建 |
| C20 四包汇总 `SHA256SUMS` | 需四个真实平台二进制；本机只能生成一个 |
| C21—C28 的真实 GitHub 写入 | 由单元测试的 `gh` 替身覆盖分支逻辑；真实 API 权限与草稿流程待发版验证 |
| E05 真实四平台 Actions 与 Release | 尚未创建版本 tag |

## 交付边界与待办

1. workspace 版本仍为 `0.0.0`；首个正式版本必须先按
   [打包与发版手册](../../releases/packaging.md) 第 1 步修改版本并刷新锁文件。
2. 建议在复发版前用 actionlint ≥ 1.7.8（或 1.7.12）复核，确认 `macos-15-intel` 不再报错。
3. `verify` 中的 `cargo audit` 依赖 RustSec 网络数据；公告更新可能阻塞与代码无关的发版，
   属预期风险，需在发版窗口重跑。

## 附录：容器部署交付的验证

本次同时新增 `deploy/docker`（Dockerfile、Compose、Caddy 入口、配置模板）与官网容器部署页，
不属于上述 tag 发版范围，单独记录实际执行结果：

| 项目 | 结果 |
| --- | --- |
| `docker compose --env-file env.example config` | 退出码 0，服务、网络、卷解析正确 |
| `deploy/docker/yufu.toml.example` TOML 解析 | 通过 `tomllib` 解析；`[security_clock]` 取值满足服务端 `validate_config` 约束 |
| `pnpm check:website` / `pnpm build:website` | 0 error；构建 27 个页面，含新增 `/operations/docker/` 与 `/en/operations/docker/` |
| 私有派生的 `Dockerfile` 前端阶段 | **未完成**：本机访问 Docker Hub 被阻断，镜像加速器在下载基础镜像层时反复 `EOF`/超时。只能用缓存到的 `node:24-bookworm-slim` 执行到 `pnpm install`，随后因镜像内 npm registry 每次请求约 16 秒而中止 |

镜像构建本身仍需在可用网络中复核（`docker build -f deploy/docker/Dockerfile .`）；
其中 `pnpm install --frozen-lockfile` 与 `cargo build --locked --release --bin yufu --features webui`
两条核心命令已在本机与现有 CI 中实际执行通过。
