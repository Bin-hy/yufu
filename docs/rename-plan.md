# cobot-for / cobotusr → yufu · 改名执行记录

> **前提变更**：本仓库处于开发阶段、无存量部署。经确认后，**不再做兼容层、不做数据迁移，一次到位**。
> 原规划中的 P0~P6 分阶段与全部迁移脚本因此取消，本文档同时取代 `docs/rename-checklist.md`（已删除）。
> 执行日期：2026-11-13

---

## 1. 结论

| 范围 | 状态 |
| --- | --- |
| 目录、crate、二进制、CLI、npm scope | ✅ 完成 |
| 运行时配置、环境变量、systemd、容器 | ✅ 完成 |
| 数据库标识（`app_key` / `audience` / `application`）、默认邮箱域 | ✅ 完成（无存量数据，直接改） |
| 界面（Logo / favicon / 标题 / 文案） | ✅ 完成（Logo 于上一轮落地） |
| Go SDK module 路径 | ✅ 完成 |
| 文档、官网、README、CLAUDE.md | ✅ 完成 |
| **GitHub 仓库名、Cloudflare Pages 域名** | ⬜ **由用户自行处理**（按要求未动） |

---

## 2. 决策记录

| # | 决策项 | 结论 | 落地结果 |
| --- | --- | --- | --- |
| D1 | 中心自身令牌 `aud`（原 `cobotusr`） | **改** | `lifecycle.rs` / `repository.rs` / `revocation.rs` / `browser.rs` 全部改为 `yufu` |
| D2 | 默认邮箱域 `cobotrix.local` | **改** | 随 `cobotrix → yufu` 一并变为 `yufu.local` |
| D3 | SSO Cookie 名 | **改** | `__Host-yufu-sso` / `__Host-yufu-flow` / `__Host-yufu-action` / `yufu-dev-*` |
| D4 | localStorage 键 | **改** | `yufu:theme` / `yufu:sidebar` |
| D5 | Go SDK module | **改** | `github.com/Bin-hy/yufu-sdk-go` |
| D6 | 数据库名 / Docker 卷名 | **改** | DB `yufu`、服务 `postgres_yufu`、卷 `yufu_pg`、env `POSTGRES_YUFU_*` |
| D7 | GitHub 仓库 / Pages 域名 | **不动** | 见第 4 节保护名单 |

---

## 3. 改动清单

### 3.1 目录与包

| 现状 | 目标 |
| --- | --- |
| `cobotusr/` | `yufu/` |
| `src/cobotusr.rs` | `src/yufu.rs` |
| `cobotweb/apps/cobotusr/` | `web/`（取消 `cobotweb` 中间层，该目录已整体移除） |
| `tests/cobotusr/` | `tests/yufu/` |
| `conf/cobotusr.example.toml` / `.service` / `.local.toml` | `conf/yufu.*` |
| `deploy/docker/cobotusr.toml.example` | `deploy/docker/yufu.toml.example` |
| `docs/specs/cobotusr-frontend-redesign/` | `docs/specs/yufu-frontend-redesign/` |
| `assets/cobotusr/` | `assets/web/` |
| 删除 | `cobotimx/`、`cobotweb/`（含 `apps/cobotlet`、`apps/cobotsrv`、`packages/shared` 空壳）、`assets/cobotlet`、`assets/cobotrix` |

### 3.2 标识

| 项 | 结果 |
| --- | --- |
| 根 crate | `yufu-root` |
| 服务 crate | `yufu` |
| 二进制 / CLI | `yufu`（clap `name`、`--version` 前缀、默认配置 `yufu.toml`） |
| 前端包 | `@yufu/web` |
| 官网包 | `@yufu/website` |
| 根 npm 脚本 | `dev:web` / `build:web` |
| 环境变量 | `YUFU_*`（14 个，含 `YUFU_IT_*` 与部署用的 `YUFU_DOMAIN`/`YUFU_HTTPS_PORT`） |
| Docker env | `POSTGRES_YUFU_*` |

### 3.3 文本替换

**227 个文件 / 2883 处**，按顺序执行以避免相互咬到：

```
@cobotweb/cobotusr → @yufu/web
@cobotrix/website  → @yufu/website
cobotweb/apps/cobotusr → web
assets/cobotusr    → assets/web
dev:cobotusr / build:cobotusr → dev:web / build:web
cobot-sdk-go       → yufu-sdk-go
COBOTUSR / CobotUsr / cobotusr → YUFU / Yufu / yufu
COBOTRIX / CoBotrix / Cobotrix / cobotrix → YUFU / Yufu / Yufu / yufu
cobotweb           → yufu
```

替换前用占位符保护了受保护串，替换后还原。

### 3.4 需要手工修正的结构项

文本替换处理不了、已单独修正的：

| 项 | 原因 |
| --- | --- |
| `web/vite.config.ts` `ProjectRoot` | `../../../` → `..`：`web/` 从三层深变成一层 |
| `deploy/docker/*.toml.example` 文件名 | 正文已改，文件名需同步 |
| `libs/auth/src/jwks.rs` 负向测试夹具 | 见第 5 节 |
| `yufu/Cargo.toml` 与 `conf/yufu.service` 的 description | 原名拼接后语义不通，重写 |
| `README.md` 项目结构注释列 | 名称变短导致对齐错位 |

---

## 4. 刻意保留 / 排除

**保护名单（按要求未改）**

- `cobotusr.pages.dev` —— 官网域名，共 28 处（**后续已随用户改名迁移到 `yufu-sso.pages.dev`**）
- `Bin-hy/cobot-for`、`cobot-for.git`、`/path/to/cobot-for` —— GitHub 仓库名，共 72 处

**排除在替换之外**

| 路径 | 原因 |
| --- | --- |
| `docs/cobotlet-feat.local/` | **另一个已退役兄弟产品**的历史规格。`cobotlet`/`cobotsrv`/`cobotimx` 不是本平台，改名会篡改历史 |
| `.claude/`、`.reasonix/`、`dsh-usage/` | 本机工具与权限配置 |
| `brand/` | 品牌资产目录，内容与新名一致，无需替换 |
| `.codegraph/codegraph.db` | 二进制索引文件 |
| 各 `node_modules`、`dist`、`.astro`、`target`、`assets/web` | 构建产物 |

---

## 5. 过程中的关键修正

这些问题不是机械替换能解决的，值得记下来：

**1. 负向测试夹具会静默失去意义。**
`libs/auth/src/jwks.rs` 里 `("aud", json!("cobotusr"))` 是"错误 audience 应被拒绝"的负向断言，而该测试的期望值是 `legacy-other-audience`。机械替换成 `"yufu"` 虽然仍能通过，但等于"拿产品名当错误值"，语义含混且日后改名又会踩一次。已改为 `"other-audience"`，与品牌解耦。

**2. Vite 的 `ProjectRoot` 依赖目录深度。**
前端从 `cobotweb/apps/cobotusr`（3 层）移到 `web`（1 层）后，`resolve(import.meta.dirname, '../../../')` 会指到仓库外。已改为 `'..'`。这类路径不会报错，只会静默产出错误目录。

**3. `.gitignore` 的资产白名单必须跟着改。**
`assets/*` + `!assets/web/` + `assets/web/*` + `!assets/web/.gitkeep` 这套规则保证 CI 的 `--all-features` 构建不会因 rust-embed 目录缺失而失败。`assets/cobotusr/.gitkeep` 已换成 `assets/web/.gitkeep`。

**4. rustfmt 需要重跑。**
标识符从 `cobotusr`(8) 缩到 `yufu`(4) 后，原本被迫换行的调用可以单行放下，`cargo fmt --check` 报差异。已执行 `cargo fmt --all` 收敛。

---

## 6. 验证结果

| 检查 | 结果 |
| --- | --- |
| `cargo check --workspace --all-features` | ✅ 56.9s，通过 |
| `cargo fmt --all -- --check` | ✅ 通过 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | ✅ 通过 |
| `cargo test --workspace --doc --all-features` | ✅ 9 用例（8 通过 / 1 忽略） |
| `cargo test --workspace --all-targets --all-features`（串行，testcontainers） | ⚠️ 241 通过 / 1 失败 / 7 忽略 —— 失败项是浏览器 E2E，**环境缺浏览器二进制**，与改名无关，见下 |
| `pnpm --filter @yufu/web check` | ✅ 通过 |
| `pnpm --filter @yufu/web lint` | ✅ 通过 |
| `pnpm --filter @yufu/web test` | ✅ 12 文件 / 100 用例 |
| `pnpm build:web` | ✅ 产出 `assets/web/` |
| `pnpm check:examples` | ✅ 通过 |
| `pnpm test:examples` | ✅ 62 + 59 用例 |
| `pnpm check:website` / `build:website` | ✅ 0 error；27 页 |
| `go test ./...` + `go vet ./...`（sdk/go） | ✅ 通过 |
| `go test ./...` + `go vet ./...`（examples/sdk/go） | ✅ 通过 |

> 注：Go 检查在本机需放宽沙箱，因为 Go 构建缓存位于 `~/Library/Caches/go-build`（工作区外）。

**关于 `identity_browser_cross_site_three_engines_login_sso_and_logout` 失败**

失败点在 `browserType.launch`，早于任何被测代码执行：

```
Error: browserType.launch: Executable doesn't exist at
  ~/Library/Caches/ms-playwright/chromium_headless_shell-1254/chrome-headless-shell-mac-arm64/chrome-headless-shell
  ~/Library/Caches/ms-playwright/webkit-2388/pw_run.sh
```

判定为**环境问题、非改名引入**，依据：

1. 报错发生在启动浏览器阶段，此时还没加载任何页面或应用代码；
2. `pnpm-lock.yaml` 相对基线的差异**只有一行**（`cobotweb/apps/cobotusr:` → `web:`），依赖版本零变动；
3. 本机缓存只有 `chromium-1091` / `chromium-1234` / `chromium_headless_shell-1234` / `firefox-1538`，**没有 webkit**，而 Playwright 需要 `1254` 与 `webkit-2388`；
4. `pnpm-workspace.yaml` 显式设了 `playwright-webkit: false`，本来就禁止下载 webkit 二进制。

要跑通该门禁需先在本机安装匹配版本的浏览器：

```sh
cd web && npx playwright install chromium firefox webkit
```

其余 241 个 Rust 用例（含大量 testcontainers 集成测试）全部通过。

---

## 7. 回滚

执行前已留非侵入式快照（未改动工作区）：

```sh
git stash create   # → 883b35fea3e268efbede45e4a64638da6721663b
```

需要整体回退时：

```sh
git checkout 05f6912 -- .     # 回到改名前的提交内容
git clean -fd                 # 清理新目录（谨慎）
```

---

## 8. 后续手工项（用户自理）

- [x] GitHub 仓库改名为 `Bin-hy/yufu`，README/官网/SDK 文档里的 22 处旧地址已同步
- [x] 官网域名迁移到 `yufu-sso.pages.dev`，24 处引用与 `wrangler.jsonc` 项目名已同步
- [ ] Cloudflare Pages 侧仍需：新建（或改名）项目为 `yufu-sso` 并绑定该域名，
      旧 `cobotusr.pages.dev` 保留 301 一段时间
- [ ] `docs/cobotlet-feat.local/` 属退役产品历史，确认是否长期保留
- [ ] `brand/logo-lockup-*.svg` 的 `<text>` 转轮廓路径（换平台渲染会飘）
