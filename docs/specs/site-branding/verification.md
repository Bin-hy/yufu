# 站点品牌与认证页面配置实现核验记录

日期：2026-11-17，Asia/Shanghai。工作区包含未提交实现；本记录只对应命令执行时的源码。最终全量结果见文末“最终完整门禁”。

## 环境

- macOS 26.6.2（arm64），Docker 29.3.1。
- rustc / cargo 1.97.1；Node v22.23.4（当前 shell），pnpm 10.33.0。
- 集成测试使用 Testcontainers 启动的一次性 PostgreSQL；重启与恢复测试另建目标容器，未连接业务数据库。
- 浏览器回归使用真实 OP、Web/SPA 示例应用与 Playwright Chromium、Firefox、WebKit。

## 命令与实际结果

| 命令 | 实际结果 | 覆盖及限制 |
| --- | --- | --- |
| `cargo test -p yufu site_branding --lib` | 9 通过，0 失败（0.36 秒） | 默认值/未知字段、全部文字与链接边界、继承与引用、图片格式/动画/尺寸/字节边界、ICO、损坏容器、解码失败释放许可。 |
| `cargo test -p yufu-root --test mod site_branding -- --test-threads=1` | 5 通过，0 失败（16.81 秒） | 上传/发布/继承/恢复/条件可见性、权限与 CSRF/Origin 矩阵、权限授予不绕过超级管理员、审计回滚、并发 revision、清理与迁移、真实重启和数据库备份恢复。执行于最终源码。 |
| `cargo fmt --all -- --check` | 通过 | 最终源码。 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 通过（9.30 秒） | 仅保留既有 proc-macro-error2 future-incompat 提示。 |
| `cargo test --workspace --doc --all-features` | 8 通过、0 失败、1 既有 ignored；另 1 个 doctest 通过 | 文档测试。 |
| `cargo machete` | 未执行 | 环境未安装该子命令，仅输出安装提示。 |
| `cargo audit` | 未执行 | 环境未安装该子命令，仅输出安装提示。 |
| `pnpm --filter @yufu/web test` | 21 个文件、206 个测试通过 | 包含品牌解析/标题/favicon 回退、页脚纯文本与安全链接、设置页预览/保存/冲突/校验失败/恢复确认、角色守卫矩阵、工作区草稿机制、审计标签。 |
| `pnpm --filter @yufu/web check` | 通过 | vue-tsc。 |
| `pnpm --filter @yufu/web lint` | 通过 | eslint。 |
| `pnpm build:web` | 通过（2.7 秒） | 产物写入内嵌 WebUI 资产目录。 |
| `cargo build --bin yufu --features webui` | 通过 | 内嵌 WebUI 二进制。 |
| 聚焦 E2E：`COBOT_SSO_PW_ARGS='-g site-branding' cargo test -p yufu-root --features webui --test mod identity_browser_cross_site_three_engines_login_sso_and_logout -- --test-threads=1` | 3 通过、0 失败（26.62 秒） | Chromium 7.8 秒、Firefox 6.2 秒、WebKit 6.8 秒；真实登录、上传、保存、刷新、OIDC 覆盖/清除、公开配置失败回退、409 冲突草稿。执行于最终 UI/测试代码；随后新增的响应头改动由最终完整门禁覆盖。 |

证据截图位于 `docs/specs/site-branding/evidence/{chromium,firefox,webkit}/`：每个引擎含 `login-{1440,390,360}-{light,dark}.png` 与 `settings-1440-light.png`，共 21 张，由最终 E2E 生成。

## 逐项核验

状态说明：“已通过”仅表示该条所有子项都有本轮实际证据；“部分”与“未执行”保持清单未勾选。

| 条目 | 结论 | 实际证据与限制 |
| --- | --- | --- |
| C01 | 已通过 | Rust 集成以超级管理员登录，读取 revision 1、上传返回 201、保存返回 200；E2E 打开 `/admin/site-settings`，编辑并保存成功。 |
| C02 | 已通过 | 角色 2（管理员，`developer_enabled=true`）与角色 3（成员）对读取/保存/恢复/上传/预览均返回 403；普通管理员即使获得单个 GET 操作显式授权仍 403；Vue 守卫矩阵覆盖 developer/administrator 四种组合，均无入口。 |
| C03 | 已通过 | 匿名读取 401；删除 `identity_sso_session` 后读取 401；修改数据库角色后仍 403；失败请求后素材数 0、配置 revision 1 且审计 0；Vue 守卫不依赖前端可伪造标识。 |
| C04 | 已通过 | 公开配置仅含 revision、branding/login/oidc 展示字段；断言无 updated_by/updated_at/limits/document、无 `*_asset_id` 与覆盖标志；随机素材 ID 返回 404；公开响应 `Cache-Control: no-store`。 |
| C05 | 未勾选（部分） | 显式 `user_authority allow=false` 后保存 403；未构造“上传解码期间撤权”的并发时序。 |
| C06 | 已通过 | 保存缺 CSRF、保存错误 Origin、恢复缺 CSRF、恢复错误 Origin、上传错误 Origin、上传缺 CSRF 均 403；失败后审计事件为 0、素材为 0、配置 revision 1。 |
| C07 | 未勾选（部分） | E2E 验证普通登录、OIDC 登录、账户中心的站点名/页脚；未单独打开开发者与企业管理员页面（fixture 超级管理员不具备这些资格，需要第二身份）。 |
| C08 | 已通过 | Vue 单测覆盖延迟加载与路由切换后的 `document.title`；E2E 刷新后断言标题包含新站点名。 |
| C09 | 已通过 | E2E 上传浏览器图标后 `link[rel=icon]` 指向公开素材，实际请求返回 200 与 `image/png`；Vue 单测覆盖设置与加载失败回退 `/favicon.svg`。 |
| C10 | 未勾选（部分） | E2E 验证欢迎标题、封面加载；Vue 单测覆盖 Logo/封面加载失败回退与换 URL 重试；未单独演示“移除后回到内置背景”。 |
| C11 | 已通过 | 领域单测与 Vue 单测覆盖继承、单项覆盖、空字符串隐藏、null 继承；E2E 对照 OIDC 独立标题与全站继承标题。 |
| C12 | 已通过 | E2E 从示例 Web 应用发起真实授权，确认应用名、OIDC 欢迎标题、登录回调与本地 subject；清除覆盖后新一轮流程回到全站外观。 |
| C13 | 已通过 | E2E 在 1440/390/360 宽度与浅色/深色组合下检查登录表单可见、页面无横向溢出并生成截图；Logo/封面请求完成且 `naturalWidth > 0`。 |
| C14 | 已通过 | Vue 单测验证页脚纯文本与空页脚不渲染；E2E 在登录页与账户中心看到版权、备案号及“帮助中心”链接，链接地址正确。 |
| C15 | 已通过 | 领域测试拒绝 javascript/data/相对地址/无主机/含凭据链接；Vue 单测确认 HTML 只作文本、危险链接不生成 `<a>`，外链带 `noopener`。 |
| C16 | 已通过 | 领域测试覆盖全部文字上限、空站点名、链接数量 10/11；新增设置页单测确认保存校验失败时显示错误、保留输入与未保存状态，已发布品牌不变。 |
| C17 | 已通过 | 图片单测覆盖 PNG/JPEG/WebP 与 favicon ICO；HTTP 集成与 E2E 实际上传 Logo、封面、图标，并验证预览/更换/移除控件存在。 |
| C18 | 未勾选（部分） | 2 MiB+1 HTTP 请求返回 413；未单独构造无 Content-Length/分块超限请求。 |
| C19 | 已通过 | 图片单测覆盖宽/高 8192 接受、8193 拒绝、损坏/截断容器与 ICO 内嵌图像校验。 |
| C20 | 未勾选（部分） | 单测覆盖 SVG/GIF/APNG/动态 WebP/伪造类型拒绝；HTTP 集成验证 SVG 返回 400；设置页上传失败提示未单独测试。 |
| C21 | 未勾选（部分） | 单测验证解码失败释放许可、后续合规上传成功；未测量未授权大请求在解码前的拒绝路径。 |
| C22 | 已通过 | 上传后公开地址 404、管理预览 200 且 `no-store`；保存引用后公开 200；随机 ID 404；恢复默认后旧 ETag 请求 404。 |
| C23 | 已通过 | 公开图片返回实际 MIME、nosniff、摘要 ETag、`no-cache, must-revalidate`；匹配 ETag 返回 304，移除引用后同 ETag 返回 404。 |
| C24 | 未勾选（部分） | 管理读取/保存/恢复/上传/预览与公开配置均断言 `no-store`；设置页代码在会话重置、卸载、换图时撤销 Blob 并以 epoch/generation 丢弃迟到响应，但没有对应组件单测。 |
| C25 | 已通过 | 设置页单测确认草稿只影响预览、保存使用原始 revision；E2E 另一上下文在保存前仍读取旧配置，且编辑/保存期间设置页未发出登录或 OIDC 请求。 |
| C26 | 未勾选（部分） | 设置页单测确认脏状态登记；既有工作区测试覆盖页签关闭/批量关闭的确认与取消；未单独验证浏览器 beforeunload 提示。 |
| C27 | 已通过 | 触发器使审计写入失败时保存返回 503，配置与 revision 完全不变；并发保存仅 200/409 各一，成功审计 1 条且变更字段为 `branding.site_name`。未单独注入引用替换失败。 |
| C28 | 已通过 | 并发 HTTP 保存得到 200/409；Vue 单测与 E2E 均确认 409 时草稿保留、不自动重载/覆盖，并提示重新加载。 |
| C29 | 未勾选 | 未取得提交不确定、权限中途失效的前端/服务专项证据。 |
| C30 | 已通过 | Rust 恢复测试递增 revision 并清理失效素材；Vue 单测确认取消不请求、确认后调用恢复；E2E 通过恢复接口清理并验证旧素材地址 404；旧 revision 保存被 CAS 拒绝。 |
| C31 | 已通过 | 新库读取 revision 1 默认配置；管理读取失败时设置页不渲染表单；前后端默认值来自共享 `brand/site-branding.defaults.json`。 |
| C32 | 未勾选（部分） | 审计搜索返回 `change_summary.changed_fields`，审计页已有站点设置动作/对象中文标签单测；未在浏览器验证列表筛选与 CSV 导出内容。 |
| C33 | 已通过 | E2E 保存后另一浏览器上下文刷新即看到新品牌，无需重新登录；设置页保存响应同步新 revision。 |
| C34 | 未勾选（部分） | E2E 中止公开配置请求后登录页按默认品牌正常显示并完成密码登录；Vue 单测覆盖请求失败回退；未在该故障下完成 OIDC 授权闭环。 |
| C35 | 未勾选（部分） | 集成测试确认未引用素材立即清理为 0，恢复默认后用最大时间批量删除 1，仍有引用时保留；未构造超过 100 张的单批上限用例。 |
| C36 | 未勾选 | 未构造清理与保存并发、以及清理失败后重试的专项时序。 |
| C37 | 未勾选（部分） | 真实重启后 revision 2 与文档保持，重跑迁移不重置配置；未单独修改站点操作权限后重跑迁移验证权限不被恢复。 |
| C38 | 已通过 | 使用 `pg_dump --format=custom` 与 `pg_restore` 恢复到另一隔离容器；恢复后站点名、revision、Logo 字节与 sha256 一致。 |
| C39 | 已通过 | `cargo test -p yufu site_branding --lib`：9/9。 |
| C40 | 已通过 | `cargo test -p yufu-root --test mod site_branding -- --test-threads=1`：5/5。 |
| C41 | 已通过 | 最终 `cargo test --workspace --all-targets --all-features -- --test-threads=1` 结果见文末。 |
| C42 | 已通过 | 最终源码 `cargo fmt --all -- --check` 与 clippy `-D warnings` 通过。 |
| C43 | 未勾选（部分） | 文档测试通过；`cargo machete` 与 `cargo audit` 未安装，依赖使用与安全检查未执行。 |
| C44 | 已通过 | 206 个前端单测、vue-tsc 与 eslint 全部通过。 |
| C45 | 已通过 | `pnpm build:web` 与 `cargo build --bin yufu --features webui` 通过；E2E 由内嵌 WebUI 二进制提供页面与公开静态资源。 |
| C46 | 已通过 | 聚焦 E2E 三引擎 3/3；最终完整测试包含全部 30 个浏览器用例，结果见文末。 |
| C47 | 已通过 | `docs/operations/site-branding.md` 覆盖入口、字段限制、OIDC 继承、24 小时暂存、数据库持久化、备份恢复、审计与容量；E2E/集成按该路径实际完成设置、上传、重置、重启与恢复。 |
| C48 | 已通过 | 本文件逐项记录实际命令与结果；未将部分或未执行项标为通过；工作区检查见文末。 |

## 端到端场景

| 场景 | 结论 | 实际证据与限制 |
| --- | --- | --- |
| E01 | 已通过 | 三引擎 E2E：超级管理员登录、上传 Logo/封面/图标、修改站点名/简介/欢迎文案/版权/备案/自定义链接、保存，另一浏览器上下文刷新普通登录与工作台后显示新品牌并完成登录。 |
| E02 | 已通过 | 三引擎 E2E：设置 OIDC 独立欢迎标题，从 Web 示例应用发起真实授权并回到应用取得本地 subject；清除覆盖后新流程显示全站标题。 |
| E03 | 未勾选（部分） | E2E 以同一管理员会话的外部 HTTP 写入制造 revision 冲突，验证 409、草稿保留；未使用两个独立超级管理员浏览器会话，恢复默认的取消/确认只在 Vue 单测中验证。 |
| E04 | 未勾选（部分） | Rust HTTP 权限矩阵覆盖管理员/成员及五种接口；未建立普通管理员的浏览器上下文验证页面直达与上传。 |
| E05 | 未勾选（部分） | E2E 在公开配置请求中止时以默认品牌完成密码登录，恢复后刷新显示已保存品牌；未在该故障下完成 OIDC 授权。 |
| E06 | 已通过 | 真实重启、重跑迁移、`pg_dump`/`pg_restore` 到另一实例后配置、revision、图片字节与摘要保持；恢复默认后旧素材公开地址 404。页面级渲染由 E2E 单独覆盖，未在恢复实例上启动浏览器。 |

## 未勾选原因汇总

- 并发/故障时序未覆盖：C05（解码期间撤权）、C29（提交不确定）、C36（清理与保存并发）。
- 浏览器角色或故障矩阵不完整：C07（开发者/企业管理员页面）、E03（双超级管理员会话）、E04（普通管理员浏览器）、E05（故障下 OIDC）。
- 子项未单独测量：C10、C18、C20、C21、C24、C26、C32、C34、C35、C37。
- 环境缺失：C43（`cargo machete`、`cargo audit` 未安装）。

以上条目保持清单未勾选；现有实现有代码路径与局部测试，但不能据此宣称完整验收。

## 工作区与改动范围（C48）

- 功能改动集中在 `yufu/src/{domain,service,route,migration,seed}/…site_*`、`web/src/{branding,components/brand,pinia/stores/branding.ts,pages/admin/...}`、品牌默认值 `brand/site-branding.defaults.json`、测试与 `docs/operations/site-branding.md`。
- 最终 `git status --short` 仍显示本功能未提交文件；`docs/operations/cloudsh-cobotrix-migration-plan.md` 为 2026-11-16 已存在的无关未跟踪文档，`docs/specs/yufu-onboarding-skill/` 与 `.agents/skills/yufu-onboarding/` 为本轮验收期间由外部并行工作创建的无关未跟踪目录；以上均未被本功能修改、删除或纳入提交。
- 未写入任何真实密钥、Cookie、令牌或个人信息；E2E 截图只包含测试站点名与示例文案。

## 最终完整门禁

最终命令：`cargo test --workspace --all-targets --all-features -- --test-threads=1`。

结果：**253 通过，0 失败，7 既有 ignored，892.88 秒，退出码 0**。其中浏览器回归 30/30 通过（Chromium、Firefox、WebKit），耗时 4 分 30 秒，包含站点品牌场景与既有 SSO 场景。

首次执行同一命令时出现 2 个与本功能无关的基础设施抖动：一个容器未暴露 5432 端口，一个测试数据库 SSLRequest 响应异常；两例单独复跑均通过（2.44 秒、2.31 秒），清理被中断测试遗留的两个 disposable 容器后完整复跑得到上述 253/0 结果。没有功能断言失败或产品代码失败。
