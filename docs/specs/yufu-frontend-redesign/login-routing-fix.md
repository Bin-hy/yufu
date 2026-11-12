# 登录请求返回 HTML 的集成修复

日期：2026-11-08。

本记录保留路由修复当时的证据与构建入口。随后发现的 0ms 503、CSRF、WebKit Cookie 与长 JWT 问题已继续修复，最新验证和本机启动选项见 [认证集成修复](login-security-integration.md)。

用户使用 `cargo run --bin yufu --features webui -- --config conf/yufu.local/yufu.toml` 启动服务后，登录请求得到首页 HTML。

## 证据与原因

只提取用户配置中的 HTTP 设置：服务监听 `127.0.0.1:4000`，未覆盖 API 前缀，因此使用后端默认 `/api/v1`。

修复前，`assets/web/index.html` 引用的主包是 `index-OoAmo4Ml.js`，普通请求和自动刷新均编译为 `baseURL:void 0`。登录 API 的相对地址为 `login`，在登录页会请求 `/login`，无法匹配实际的 `POST /api/v1/login`。后端静态兜底没有区分 HTTP 方法或 API 路径，将未匹配请求返回为 `200 text/html`。

对运行中服务仅发送无凭据 GET 的结果：

| 路径 | 状态 | 响应类型 |
| --- | --- | --- |
| `/login` | 200 | `text/html`，首页 |
| `/api/v1/login` | 405 | 空响应；该接口只接受 POST |
| `/api/v1/__diagnostic_missing__` | 200 | `text/html`，首页 |

前面的浏览器验收显式指定了 `VITE_BASE_URL=/api/v1`，覆盖了默认构建缺失配置的问题。本次将浏览器测试构建改为空值，检验代码默认行为。

## 修复

- 普通 API 和自动刷新共用默认同源 `/api/v1`；有效的自定义 `VITE_BASE_URL` 继续生效。
- HTTP 层识别成功响应及失败响应中的 HTML，提示检查 API 地址或代理配置；登录页保留整理后的错误信息。
- 静态兜底只处理 GET/HEAD。配置的 API 前缀下未匹配路径、未匹配的写请求、缺失的 `/assets/` 资源返回 JSON 404。SPA 深链接、OIDC 登录/同意页面以及静态资源 MIME 保持可用。
- 新增根目录 `dev:web` 和 `build:web` 脚本，修正环境配置示例中的服务名称。
- 已执行 `pnpm build:web` 更新本地 `assets/web`；保留目录中的 `.gitkeep`。新入口为 `index-Fw9DWsyI.js`，不再含请求客户端的空 `baseURL`，默认 `/api/v1` 已写入构建。无凭据 GET 确认运行中的 debug WebUI 已读到新入口。

## 验证

- 新增 HTTP 回归测试在实现修改前复现 6 处失败；实现修改后前端全部 49 个单测通过。
- `pnpm --filter @yufu/web check`、`lint` 和生产构建通过。
- `cargo test -p yufu --features webui --lib webui::tests`：7 个隔离路由测试通过，覆盖实际接口与方法匹配、默认/自定义 API 前缀、SPA 深链接、HEAD、资源 MIME 和资源缺失。
- `cargo test -p yufu --lib`：31 个测试通过。
- `cargo clippy -p yufu --features webui --lib -- -D warnings` 通过；修改的 Rust 文件格式检查通过。
- `cargo build --bin yufu --features webui` 通过，确认用户原启动命令使用的二进制可以编译。
- `playwright test tests/e2e/auth.spec.ts --reporter=line`：11 个认证场景 × Chromium/Firefox/WebKit，共 33 项通过。验证默认构建真实请求路径 `/api/v1/login`，以及 HTML 响应提示、保留输入和可重试行为。

路由测试使用隔离的内存服务，浏览器 API 使用测试响应。未对真实业务后端提交登录、注册或其他写请求；真实账号登录未验收。

## 本地使用

前端源码修改后先从仓库根目录运行：

```sh
pnpm build:web
```

再停止旧进程，按原命令启动并强制刷新浏览器：

```sh
cargo run --bin yufu --features webui -- --config conf/yufu.local/yufu.toml
```

`cargo run --features webui` 使用 `assets/web`，不会自动运行 Vue/Vite 构建。后端路由修复需要重启生效。浏览器 Network 中登录请求应为 `POST /api/v1/login`；账号校验结果应为业务 JSON 或相应 HTTP 错误，不能是首页 HTML。
