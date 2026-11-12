# 本地登录 503 与浏览器认证链路修复

日期：2026-11-08。承接 [登录路由修复](login-routing-fix.md)，记录本轮实际后端与浏览器联调。

重启后仍出现 403 的后续复现、localhost 入口兼容与诊断见 [403 续修](login-csrf-403-fix.md)，以下测试计数保留前一轮记录。

## 原因与修复

`POST /api/v1/login` 在 0ms 内返回 503：独立启动调用普通 `serve`，没有注入安全时钟，`SecurityBudget::new(state.security_clock()?)` 在访问数据库前拒绝。`yufu`、`yufu`、`cobotnil` 现在启动 NTP 健康监控并调用 `serve_with_security_clock`。没有注入时钟的库入口仍失败关闭。

监控并行查询配置的 NTP 来源，校验请求时间戳回显、响应模式、层级、同步标记、处理时间与误差；按不同 IP 去重，至少要求配置数量的误差区间具有公共交集，总误差不得超过一秒。首次检查失败拒绝启动；运行后检查失败、样本过期、宿主时间跳变则停止认证签发，恢复健康后可继续。只观测宿主时间，不设置 OS 时间。默认来源与超时见 `conf/yufu.example.toml`。普通 NTP 不具备传输认证；生产需要受信网络与 NTP 来源，或宿主注入具有可信健康证据的 Clock。

前端此前没有适配后端 CSRF：写请求现在先获取 `GET /api/v1/csrf`，以 `X-CSRF-Token` 携带证明。请求共用进行中的挑战，登录与刷新返回的证明替换缓存，退出清理缓存；会话 epoch 改变后禁止旧证明提交请求。挑战失败阻止业务写请求；403 不自动重放写操作，手动重试会重新获取证明；嵌套请求同一错误只提示一次。

本机 HTTP 下 WebKit 不接收 Secure Cookie。新增 `--allow-insecure-loopback-cookies` 与对应配置字段，仅允许 loopback 监听、issuer 与请求 Origin；不安全配置在数据库初始化前拒绝。此模式 CSRF 使用 `yufu-dev-csrf`，刷新 Cookie 继续 HttpOnly、SameSite=Lax、Path=/。默认保持 Secure 与 `__Host-yufu-csrf`，生产浏览器入口使用 HTTPS。

真实浏览器还复现了长访问 Token 的存储问题：本轮隔离普通账户登录返回 **7,341 字节** JWT，管理员为 **18,715 字节**，单个 Cookie 没有写入成功，登录后不发当前身份请求，刷新页面回到登录。访问 Token 改为当前标签页的 `sessionStorage` 和内存回退，旧短 Token Cookie 可以迁移并删除。Bearer 请求与后端 HttpOnly 刷新 Cookie 的传输契约保持。访问 Token 不写入 localStorage、URL、工作台快照；CSRF 证明只在请求客户端内存中缓存。此存储仍可由同源 JavaScript 读取，不能视为 HttpOnly 保护；禁用存储时当前页可用，重新加载不保证恢复。

退出原先先重置本地会话，异步请求拦截器因此取消尚未发送的退出请求。现在先等待后端退出，再清理本地会话；服务失败仍清理本地，不宣称服务器撤销成功。

## 验证范围

后端测试使用一次性 PostgreSQL 18-alpine、隔离 HTTP 监听与本机 UDP NTP 响应。真实浏览器测试由 Rust 夹具启动，只能指向夹具随机 loopback 端口；使用构建后的实际 WebUI，模拟一次 GET /user 的 401，其余注册、登录、挑战、刷新、退出均访问隔离真实后端。三种浏览器检查 HttpOnly 刷新 Cookie、CSRF 轮换、长访问 Token 恢复和退出清 Cookie。

第一次并发执行后端测试、编译和五个浏览器 worker 时，两项注册超出原两秒预算返回 503；没有延长安全预算。长 Token 问题则在单独执行时稳定复现，修复后重新验证。读取接口权限 403 不再错误清除仍然有效的 CSRF 缓存；拒绝的写请求仍清除并要求下次手动操作重新取证。隔离普通账户缺当前身份读取权限时正确返回 403，未扩大其授权；完整身份恢复链使用夹具管理员验证。

最终执行结果：

- 前端 60 个单元测试全部通过，覆盖 API 默认/自定义地址、CSRF 并发、轮换、故障与取消、长 Token 的恢复/迁移、存储禁用回退；类型检查、lint 和生产构建通过。
- `cargo test -p yufu --lib --features webui`：42 项通过，包括时钟协议/失健康/恢复/过期/别名、Cookie 配置与实际监听拒绝、静态路由与日志处理。
- 隔离后端认证/撤销语义 16 项通过；随后更新过的 loopback 夹具两项再次独立通过。所有原两秒预算与 missing-clock/unhealthy/HMAC 拒绝场景保持。
- `security_browser_real_frontend_login_refresh_logout_all_engines`：Chromium、Firefox、WebKit 均通过实际注册、管理员 UI 登录、200 当前身份读取、长 Token 页面恢复、真实刷新、UI 退出与清 Cookie，实际管理员 JWT 长度 18,715 字节。
- 六份功能 E2E 初次执行 174 项中 171 项通过，3 项因原夹具同时禁用 sessionStorage 而无法恢复访问 Token。将工作台故障夹具限定为 localStorage，并新增所有 Storage 禁用时内存登录/导航/重新登录/退出的显式场景后，6 项定向回归全部通过；最终覆盖 177 个功能浏览器场景。没有重跑视觉截图，也未覆盖原有截图或 213 项历史基线报告。
- `cargo clippy -p yufu --features webui --lib --tests -- -D warnings` 与根集成测试 `cargo clippy --test mod --features webui -- -D warnings` 均通过，Rust 格式检查、diff 检查通过；`cargo build --bin yufu --bin yufu --bin cobotnil --features webui` 通过。现有 proc-macro-error2 future-incompat 提示仍存在。

本轮当时的 WebUI 入口为 `assets/index-embpe-eF.js`，后续 403 续修入口为 `assets/index-CtkJUYqG.js`。Vite 构建输出目录保留其他已有资源；其他服务的 `.gitkeep` 删除属于原工作区改动，本轮未操作。

只对用户配置执行 `--check-security-clock` 只读诊断，实测 synchronized=true、保守误差约 175ms。没有向用户正在运行的业务服务提交登录、注册、退出或其他写请求，没有修改用户的真实配置或重启旧进程。

完整网关授权整改仍为 T01–T27 完成；本轮仅覆盖 T67、T70、T71 的 yufu 登录适配部分。普通 API 当前凭据状态、跨服务撤权、完整 OIDC 与 MCP 迁移及生产环境验收仍按原计划推进。

## 本地运行

前端已重新构建到 `assets/web`。用户停止旧进程后运行：

```sh
cargo run --bin yufu --features webui -- \
  --config conf/yufu.local/yufu.toml \
  --allow-insecure-loopback-cookies
```

使用本机 loopback 地址访问并强制刷新浏览器。通过无凭据 GET 检查认证时钟状态：

```sh
curl -i http://127.0.0.1:4000/api/v1/health/ready
```

健康时 200 JSON：`{"ready":true,"security_clock_synchronized":true}`；缺失或失健康时 503 JSON。此接口只检查安全时钟，不代表全部业务依赖健康。独立检查时钟而不启动数据库或监听 HTTP：

```sh
cargo run --bin yufu --features webui -- \
  --config conf/yufu.local/yufu.toml --check-security-clock
```

未来修改前端源码后先执行 `pnpm build:web`；`cargo run --features webui` 不会自动构建 Vue/Vite。
