# 站点品牌与认证页面配置 Tasks

> 依据已批准的 spec.md 和已认可的 plan.md。用户已确认本任务清单；checklist.md 已批准，开始执行并记录进度。每项为约 2–5 分钟的聚焦编辑单元；命令运行、数据库和浏览器启动时间不计入编辑时长。验证失败先修复该任务，再继续依赖任务。

## 文件清单

路径相对仓库根目录。新文件只能在四份文档全部批准后创建，本文档自身除外。

| 操作 | 文件 | 职责 |
| --- | --- | --- |
| 新建 | `brand/site-branding.defaults.json` | 共用默认值 |
| 新建 | `yufu/src/domain/identity/site_branding.rs` | 配置类型、纯规则与领域测试 |
| 修改 | `yufu/src/domain/identity/mod.rs`、`account.rs`、`admission.rs`、`session.rs`、`operation.rs` | 模块、新标识、既有测试快照、新操作 |
| 新建 | `yufu/src/service/identity/site_settings.rs`、`site_asset.rs`、`repository_site_settings.rs` | 服务、解码、存储 |
| 修改 | `yufu/src/service/identity/mod.rs`、`repository.rs`、`repository_pg.rs`、`repository_outbox.rs`、`workers.rs` | 注册、事务接口、角色读取、审计、清理 |
| 新建 | `yufu/src/route/site_branding.rs`、`site_settings.rs` | 公开及管理接口 |
| 修改 | `yufu/src/route/mod.rs`、`identity_browser.rs`、`account.rs`、`yufu/src/error.rs` | 挂载、请求限制、会话字段、超限错误映射 |
| 新建 | `yufu/src/migration/m20261117_000001_site_branding.rs`、`yufu/src/seed/m20261117_000002_site_branding_operations.rs` | 数据与操作迁移 |
| 修改 | `yufu/src/migration/mod.rs`、`yufu/src/seed/mod.rs` | 追加迁移 |
| 修改 | `Cargo.toml`、`yufu/Cargo.toml`、`Cargo.lock` | image 依赖 |
| 新建 | `web/src/branding/types.ts`、`defaults.ts`、`resolve.ts`、`api.ts` | 配置契约、解析、公开读取 |
| 新建 | `web/src/pinia/stores/branding.ts` | 已发布状态 |
| 新建 | `web/src/components/brand/site-logo.vue`、`site-footer.vue`、`auth-visual.vue` | 共用展示组件 |
| 新建 | `web/src/pages/admin/api/site-settings.ts`、`web/src/pages/admin/views/site-settings.vue` | 管理 API 与设置页 |
| 新建 | `web/src/pages/admin/components/site-branding-preview.vue`、`site-asset-field.vue` | 预览、上传、继承 |
| 修改 | `web/src/main.ts`、`web/src/router/guard.ts`、`navigation.ts`、`routes.ts` | 初始化、标题、权限和入口 |
| 修改 | `web/src/pages/account/api/type.ts`、`web/src/types/router.d.ts` | 会话和路由类型 |
| 修改 | `web/src/layouts/auth-layout.vue`、`web/src/layouts/components/Logo/index.vue`、`web/src/layouts/mode/left-top-mode.vue` | 真实页面接入 |
| 修改 | `web/src/components/workspace/login-form.vue`、`web/src/pages/oidc-consent/index.vue`、`web/src/style.css`、`web/src/lib/identity-labels.ts` | 外观上下文、布局、审计名称 |
| 新建 | `tests/yufu/routes/site_branding.rs`、`tests/fixtures/site-branding/` | HTTP/数据库/权限/事务测试和图片素材 |
| 修改 | `tests/yufu/routes/mod.rs` | 注册 Rust 集成测试 |
| 新建 | `web/tests/unit/site-branding.test.ts`、`site-settings.test.ts` | 前端状态、表单与展示测试 |
| 修改 | `web/tests/unit/admin-console.test.ts`、`identity-ui.test.ts`、`identity-session.test.ts`、`approval-snapshot.test.ts`、`admin-feedback.test.ts` | 新会话标识的原有 fixture 兼容 |
| 新建 | `web/tests/e2e/enterprise-sso-site-branding.spec.ts` | 真实完整用户场景 |
| 修改 | `web/tests/e2e/enterprise-sso-fixture.ts`、`enterprise-sso-webkit.mjs` | 隔离账户/测试辅助函数与旧 macOS WebKit |
| 新建 | `docs/operations/site-branding.md`、`docs/specs/site-branding/verification.md` | 运维说明和实际验证证据 |

现有 `web/public/favicon.svg` 和 YufuMark 继续用作回退资源，不改变原文件。既有用户配置、密钥、业务数据库及无关的迁移计划文档不在任务范围内。

## 验证命令约定

- **R**：`cargo check -p yufu`，新增后端类型/接口编译通过。
- **U**：`cargo test -p yufu site_branding --lib`，领域和解码测试通过；新增测试名含 site_branding。
- **I**：`cargo test --test mod site_branding -- --test-threads=1`，新 Rust 集成测试通过，数据库使用现有隔离 fixture。
- **V**：`pnpm --filter @yufu/web test -- tests/unit/site-branding.test.ts tests/unit/site-settings.test.ts`，新增前端测试通过。
- **C**：`pnpm --filter @yufu/web check`，Vue/TypeScript 类型检查通过。
- **E**：`COBOT_SSO_PW_ARGS='--project=chromium -g site-branding' pnpm --filter @yufu/web test:e2e`，新增浏览器测试通过；新增用例标题含 site-branding。

未实现的接口不使用无条件成功或静默回退的占位实现。新增 trait 方法与 PostgreSQL 实现一起接入；未完成阶段允许只运行对应领域测试，最终不得遗留占位逻辑。

## T1：默认值和领域类型

**文件：** `brand/site-branding.defaults.json`、`yufu/src/domain/identity/site_branding.rs`、`yufu/src/domain/identity/mod.rs`。
**依赖：** 无。
**步骤：**
1. 定义 plan 中的配置、请求、图片用途、引用、解析结果和响应类型，拒绝未知请求字段。
2. 写入共用默认 JSON，加载为类型化默认配置；保留现有品牌和内置背景的初始效果。
3. 添加默认配置可反序列化、请求未知字段被拒绝的测试并注册模块。
**验证：** R、U；默认值测试通过，默认 OIDC 各项均继承。

## T2：字段与链接规则

**文件：** `yufu/src/domain/identity/site_branding.rs`。
**依赖：** T1。
**步骤：**
1. 先写文字长度、空白网站名、控制字符、链接数与协议边界测试。
2. 实现 validate_settings，按 Unicode 字符数验证已批准的上限；HTTP/HTTPS 地址必须包含主机且无凭据。
3. 验证空备案链接和空可选文字可用，拒绝危险链接。
**验证：** U；每项上限和上限加一的结果明确，非法输入返回 InvalidInput。

## T3：继承、引用与变更字段

**文件：** `yufu/src/domain/identity/site_branding.rs`。
**依赖：** T1。
**步骤：**
1. 先写逐项继承、空文字隐藏、封面覆盖和清除测试。
2. 实现 resolve_branding、referenced_assets 和 changed_fields。
3. 固定字段路径和顺序；测试同一素材可被两个封面槽引用，审计只输出变化字段。
**验证：** U；单项覆盖不影响其他项，空字符串区别于 null。

## T4：超级管理员标识与兼容

**文件：** `yufu/src/domain/identity/account.rs`、`admission.rs`、`session.rs`、`yufu/src/service/identity/repository_pg.rs`、`yufu/src/route/account.rs`；文件清单中的五个既有 Vue fixture 和 `web/src/pages/account/api/type.ts`。
**依赖：** 无。
**步骤：**
1. 增加 super_administrator，由当前数据库关联角色判定，并加入会话响应。
2. 更新现有 Rust AccountSnapshot 构造和前端会话 fixture，普通成员默认 false。
3. 为超级管理员与普通企业管理员区别增加测试，不改变 administrator 现有含义。
**验证：** R、C；`cargo test -p yufu --lib` 及五个受影响 Vue 测试通过。

## T5：表结构和初始行迁移

**文件：** 新增 site_branding 数据迁移、`yufu/src/migration/mod.rs`、`tests/yufu/routes/site_branding.rs`、`tests/yufu/routes/mod.rs`。
**依赖：** T1。
**步骤：**
1. 创建 settings、asset、ref 三张表、约束和清理索引；固化初始默认 JSON。
2. 追加迁移，禁止更改历史迁移或重置历史数据。
3. 添加新库初始化、二次启动不覆盖配置、引用阻止误删的隔离测试。
**验证：** I；唯一配置行、外键和图片大小约束生效。

## T6：新管理操作注册

**文件：** `yufu/src/domain/identity/operation.rs`、新增 site_branding_operations seed、两个迁移注册文件、`tests/yufu/routes/site_branding.rs`。
**依赖：** T4、T5。
**步骤：**
1. 添加五个精确操作并按 plan 注册方法和路径。
2. 新 seed 只插入新操作，不复制普通管理员授权，不恢复被修改的历史权限。
3. 添加操作存在、普通管理员不自动获权、重复迁移保持拒绝的测试。
**验证：** R、I；未知操作拒绝，既有显式拒绝保持。

## T7：配置读取与图片元数据仓储

**文件：** `yufu/src/service/identity/repository.rs`、`repository_pg.rs`、新增 `repository_site_settings.rs`、`mod.rs`、`tests/yufu/routes/site_branding.rs`。
**依赖：** T1、T5。
**步骤：**
1. 成对增加 site_settings、site_asset_metadata 事务接口和参数化查询实现。
2. 支持 settings 行锁读取，校验 schema_version、revision 和数据库配置结构。
3. 添加读默认行、数据库坏配置报错、不读取图片 BYTEA 的测试。
**验证：** R、I；读取故障返回依赖错误，不伪装成默认配置成功。

## T8：图片写入与内容读取仓储

**文件：** T7 仓储文件、`tests/yufu/routes/site_branding.rs`。
**依赖：** T7。
**步骤：**
1. 成对增加 insert_site_asset、site_asset_content 接口及实现。
2. 初始记录 unreferenced_since；公开读取必须存在当前 ref，管理内容读取不应用公开筛选。
3. 测试存储原始内容及元数据、未发布图片公开读取不存在。
**验证：** R、I；图片字节往返一致，公开读取不泄漏未发布素材。

## T9：版本更新和引用原子替换

**文件：** T7 仓储文件、`tests/yufu/routes/site_branding.rs`。
**依赖：** T3、T8。
**步骤：**
1. 成对增加 replace_site_settings 接口和实现，要求版本匹配且递增。
2. 在原事务内验证所有图片用途/存在性并替换 ref；正确设置和清除失去引用时间。
3. 添加并发旧版本、图片错用途、引用更新失败整体回滚测试。
**验证：** R、I；旧版本产生 Conflict，失败后文档、版本、引用和时间均保持原值。

## T10：图片验证依赖与测试素材

**文件：** 三个 Cargo 文件、`tests/fixtures/site-branding/`、新增 `site_asset.rs`、服务模块注册。
**依赖：** T1。
**步骤：**
1. 增加工具链兼容的 image 0.25 系列依赖，仅启用必要 codec，并记录锁文件。
2. 准备小尺寸静态 PNG/JPEG/WebP/ICO、损坏文件、APNG/动态 WebP 素材及可重复生成方式。
3. 先添加格式、用途、动画和字节数的失败测试。
**验证：** `cargo check -p yufu`；确认启用的依赖特性；测试素材无真实用户数据。

## T11：格式、动画与 ICO 验证

**文件：** `yufu/src/service/identity/site_asset.rs`。
**依赖：** T10。
**步骤：**
1. 实现真实内容识别和完整解码，拒绝不支持格式及动画。
2. ICO 仅允许 favicon，验证容器与内嵌图像，不只检查文件头。
3. 计算 MIME、尺寸、SHA-256，保留验证通过的原始字节。
**验证：** U；支持格式通过、伪造 MIME/后缀不能绕过、损坏内嵌内容被拒绝。

## T12：大小、尺寸与解码资源边界

**文件：** `site_asset.rs`、`tests/fixtures/site-branding/`。
**依赖：** T11。
**步骤：**
1. 增加 2/10 MiB 与 8192 像素的边界测试，使用测试生成素材避免大文件入库。
2. 解码前检查尺寸，配置 512 MiB 分配预算，并使用 blocking 任务和单并发许可。
3. 为等待超时和解码失败释放许可增加测试，记录分配预算并非进程硬上限。
**验证：** U；上限可接受、超限拒绝，失败不占住后续解码许可。

## T13：公开与管理读取服务

**文件：** 新增 `site_settings.rs`、服务模块注册、`tests/yufu/routes/site_branding.rs`。
**依赖：** T4、T6、T7。
**步骤：**
1. 定义 SiteSettingsService 并注入仓储与时钟。
2. 实现公开解析及管理读认证；公开响应仅含展示信息和同源图片地址。
3. 添加普通管理员即使单独获操作授权也被拒绝的测试。
**验证：** R、I；公开响应无管理元数据，超级管理员显式拒绝有效。

## T14：保存与审计服务

**文件：** `site_settings.rs`、`repository_outbox.rs`、`tests/yufu/routes/site_branding.rs`。
**依赖：** T2、T3、T9、T13。
**步骤：**
1. 用现有事务执行器实现保存，按 Policy/用户/会话锁后重验身份权限，再锁设置行。
2. 审计白名单增加 changed_fields，保存事件与设置共事务提交。
3. 添加失效会话、并发版本、审计失败回滚及提交结果不确定的服务测试。
**验证：** R、I；失败不产生成功审计，重试不改变 expected_revision。

## T15：恢复默认服务

**文件：** `site_settings.rs`、`tests/yufu/routes/site_branding.rs`。
**依赖：** T14。
**步骤：**
1. 独立校验 SiteSettingsReset 权限，使用当前默认文档执行同一原子替换流程。
2. 清除覆盖和全部图片引用，递增 revision，记录 reset 事件。
3. 测试旧版本恢复被拒绝、恢复后旧草稿不能保存。
**验证：** I；恢复结果与默认文档一致，图片当前公开引用清空。

## T16：上传和预览服务

**文件：** `site_settings.rs`、`tests/yufu/routes/site_branding.rs`。
**依赖：** T8、T12、T13。
**步骤：**
1. 实现上传前权限预检和解码后的事务内重新认证授权。
2. 生成随机素材 ID，插入素材并返回管理预览元数据。
3. 实现管理预览精确授权，测试解码期间撤权、未发布素材及无效图片不落库。
**验证：** R、I；上传不改变公开配置，撤权后不写入图片。

## T17：公开 HTTP 接口与缓存

**文件：** 新增 `yufu/src/route/site_branding.rs`、`route/mod.rs`、`tests/yufu/routes/site_branding.rs`。
**依赖：** T13、T16。
**步骤：**
1. 挂载 GET branding 和 GET asset，不依赖用户登录。
2. 配置 no-store、图片重验证、实际 MIME、nosniff、ETag；检查引用后才能返回 304。
3. 测试公开 DTO 字段和素材取消引用后旧 ETag 返回 404。
**验证：** I；公开请求状态和响应头符合 plan，未发布 ID 与不存在 ID 均 404。

## T18：设置管理 HTTP 接口

**文件：** 新增 `yufu/src/route/site_settings.rs`、`route/mod.rs`、`identity_browser.rs`、`tests/yufu/routes/site_branding.rs`。
**依赖：** T14、T15。
**步骤：**
1. 添加 GET/PUT 设置和 POST reset，复用现有写凭据与统一错误体。
2. 设置 JSON 单独采用 128 KiB，认证接口继续 16 KiB；管理响应 no-store。
3. 测试未知字段、缺字段、错误 CSRF/Origin、无登录、403、409 和最长合规文案。
**验证：** I；普通认证请求限制未放宽，管理请求错误带 request_id。

## T19：上传/预览 HTTP 接口与有界读

**文件：** `site_settings.rs` 路由、`identity_browser.rs`、`yufu/src/error.rs`、`tests/yufu/routes/site_branding.rs`。
**依赖：** T16、T18。
**步骤：**
1. 添加原始文件上传和管理图片 GET；手动读取 body 前完成权限预检。
2. 按 kind 限流式字节数，覆盖无 Content-Length/伪造长度，统一超限错误为 413。
3. 测试 201、400、401、403、404、413、响应 MIME 和 no-store。
**验证：** I；未经授权的大请求不能进入解码；超限请求不产生素材。

## T20：素材清理仓储与调度

**文件：** `repository.rs`、`repository_pg.rs`、`repository_site_settings.rs`、`workers.rs`、`tests/yufu/routes/site_branding.rs`。
**依赖：** T9、T16。
**步骤：**
1. 成对实现 cleanup_site_assets 接口和适配，独立事务锁设置行后重新检查引用。
2. 接入已有周期调度，每批最多 100 张，删除未引用超过 24 小时的素材。
3. 测试期限边界、双槽引用、最后引用移除重新计时、清理与保存竞争。
**验证：** R、I；当前引用图片永不误删，清理失败不改变身份保留策略。

## T21：前端类型、默认值与解析测试

**文件：** 新建 `web/src/branding/types.ts`、`defaults.ts`、`resolve.ts`、`web/tests/unit/site-branding.test.ts`。
**依赖：** T1、T3。
**步骤：**
1. 定义与后端一致的类型，导入共用默认 JSON，允许深拷贝后编辑。
2. 先写再实现 OIDC 继承、null/空文字差异和公开响应结构校验。
3. 测试源默认文档不会被草稿修改。
**验证：** V、C；前后端默认字段一致，坏响应被拒绝。

## T22：公开 API 和品牌状态

**文件：** `web/src/branding/api.ts`、`web/src/pinia/stores/branding.ts`、`web/src/main.ts`、`site-branding.test.ts`。
**依赖：** T17、T21。
**步骤：**
1. 使用独立公开请求加载 branding，设置超时，不触发中心会话失效流程。
2. store 初始化默认值，异步加载不阻塞挂载；隔离迟到请求避免覆盖刚保存的新状态。
3. 测试网络失败、错误结构、慢请求时页面仍可使用默认配置。
**验证：** V、C；失败不登出、不阻塞登录页展示。

## T23：站点 Logo 与页脚组件

**文件：** 新建 `site-logo.vue`、`site-footer.vue`、`site-branding.test.ts`。
**依赖：** T21。
**步骤：**
1. 实现通过 props 接收配置的 Logo，坏图回退 YufuMark，保持 contain。
2. 实现纯文本页脚和安全链接，空项不占位，外链行为明确。
3. 测试 HTML 文本不执行、坏图不无限重试、空页脚不输出多余分隔符。
**验证：** V、C；Logo/页脚可同时用于真实布局与预览。

## T24：认证视觉组件与 OIDC 上下文

**文件：** 新建 `auth-visual.vue`；修改 `auth-layout.vue`、`login-form.vue`、`pages/oidc-consent/index.vue`、`site-branding.test.ts`。
**依赖：** T22、T23。
**步骤：**
1. 抽取认证侧栏视觉，接受普通或 OIDC 解析配置，保留默认背景。
2. 登录与授权显式传入外观上下文，替换硬编码品牌及页脚。
3. 测试应用名、请求 scope、登录表单和授权事件仍来自原流程。
**验证：** V、C；两种外观正确，原认证行为测试通过。

## T25：导航、工作台页脚与标题图标

**文件：** `layouts/components/Logo/index.vue`、`layouts/mode/left-top-mode.vue`、`router/guard.ts`、`pinia/stores/branding.ts`、`site-branding.test.ts`。
**依赖：** T22、T23。
**步骤：**
1. 工作台使用共用 Logo 和页脚，折叠菜单保留可访问名称。
2. 路由与站点名共同更新 document.title，异步加载后立即重算。
3. 更新 favicon，并在加载失败时恢复 `/favicon.svg`；测试来回修改和默认恢复。
**验证：** V、C；页面标题不丢失，预览草稿不修改真实浏览器图标。

## T26：超级管理员页面入口

**文件：** `router/navigation.ts`、`guard.ts`、`routes.ts`、`types/router.d.ts`、`web/tests/unit/site-settings.test.ts`。
**依赖：** T4、T21。
**步骤：**
1. 扩展 qualification 与导航判断，注册 `/admin/site-settings`。
2. 先添加角色矩阵测试，再实现入口显示和路由守卫。
3. 普通管理员直达页面返回现有 403；超级管理员不依赖普通管理员布尔值获得前端入口资格。
**验证：** V、C；四类账户导航和守卫结果正确。

## T27：管理 API 封装与 Blob 生命周期

**文件：** 新建 `web/src/pages/admin/api/site-settings.ts`、`site-settings.test.ts`。
**依赖：** T18、T19、T21。
**步骤：**
1. 封装读取、完整保存、恢复、原始文件上传和 Blob 预览，复用 identityRequest。
2. 类型化 revision、kind 和响应，错误局部展示，禁止自动重放不确定写操作。
3. 测试请求保留 CSRF/会话机制，Blob 响应及取消操作正确。
**验证：** V、C；原始文件不被 JSON 化，上传返回正确元数据。

## T28：设置页骨架、读取和草稿

**文件：** 新建 `pages/admin/views/site-settings.vue`、`site-settings.test.ts`。
**依赖：** T26、T27。
**步骤：**
1. 构建四组设置字段、加载/失败/重试状态，深拷贝已读配置建立草稿。
2. 使用服务端 limits 展示限制，接入 useFormDraft、工作区及 beforeunload。
3. 管理读取失败时禁止保存；登出或账户切换清空草稿与迟到响应。
**验证：** V、C；编辑不改变公开 store，切页/关页行为符合现有草稿机制。

## T29：图片字段与 OIDC 继承控制

**文件：** 新建 `site-asset-field.vue`，修改 `site-settings.vue`、`site-settings.test.ts`。
**依赖：** T27、T28。
**步骤：**
1. 实现上传、更换、移除、限制提示和错误保留原图。
2. 每项覆盖明确继承/自定义，清除封面后显示全站封面；文字 null 与空串正确保留。
3. 上传、换图、销毁和会话重置时管理 Blob URL；标明未引用素材 24 小时期限。
**验证：** V、C；上传失败不丢原配置，预览对象地址释放，未经保存不公开。

## T30：预览展示与主题尺寸

**文件：** 新建 `site-branding-preview.vue`，修改 `site-settings.vue`、`style.css`、`site-settings.test.ts`。
**依赖：** T23、T24、T29。
**步骤：**
1. 用草稿解析结果渲染共用视觉、Logo、页脚，切换普通/OIDC 预览。
2. 使用只展示的表单样式，不触发登录/授权接口或改变全局品牌。
3. 补齐浅深色与窄屏展示，长文案不遮挡设置和保存按钮。
**验证：** V、C；预览切换只改变草稿视图，没有认证请求。

## T31：保存、冲突与恢复默认交互

**文件：** `site-settings.vue`、`site-settings.test.ts`。
**依赖：** T28、T29、T30。
**步骤：**
1. 保存携带读取时 revision，成功后重建基线并刷新公开状态。
2. 409 保留草稿，确认后才重新加载；失败及不确定结果不提示保存成功。
3. 恢复默认先确认，取消不请求；成功同步表单、覆盖状态、预览及公开状态。
**验证：** V、C；测试保存失败保留输入、冲突不覆盖、恢复取消与成功场景。

## T32：审计展示与导出

**文件：** `web/src/lib/identity-labels.ts`、`web/tests/unit/identity-labels.test.ts`、`site-settings.test.ts`。
**依赖：** T14、T15、T31。
**步骤：**
1. 添加保存、恢复和站点设置对象的中文名称，展示 changed_fields。
2. 验证现有身份审计列表、展开摘要、筛选和导出可容纳新事件。
3. 不增加文案全文、链接全文或图片内容到审计。
**验证：** 前端相关单测与 I；记录有操作者、时间、变更项，失败无成功记录。

## T33：真实设置到登录生效的 E2E

**文件：** 新建 `enterprise-sso-site-branding.spec.ts`，扩展 `enterprise-sso-fixture.ts`。
**依赖：** T20、T24、T25、T31、T32。
**步骤：**
1. 用隔离超级管理员经设置页上传并保存 Logo/封面/网站名/页脚。
2. 用另一浏览器上下文打开普通登录页验证已发布配置；验证未保存修改不可见。
3. 添加独立普通企业管理员/成员账户，验证无入口且直连管理 API 被拒绝；测试结束恢复默认，避免污染后续用例。
**验证：** E；使用真实后端和隔离数据库，界面与接口权限同时通过。

## T34：OIDC、容错和多视口 E2E

**文件：** `enterprise-sso-site-branding.spec.ts`、`enterprise-sso-webkit.mjs`。
**依赖：** T33。
**步骤：**
1. 从真实接入示例发起 OIDC，验证覆盖、继承、清除及实际授权完成。
2. 模拟公开配置/图片读取失败，确认默认外观及登录仍可用；检查空项、长文案、浅深色与移动视口。
3. 为旧 macOS 外部 WebKit 流程补充相同关键品牌场景；三引擎结果分别记录，不因环境缺失宣称通过。
**验证：** E；再执行 `pnpm --filter @yufu/web test:e2e`，原登录/退出及新品牌流程通过。

## T35：重启、升级与备份恢复验证

**文件：** `tests/yufu/routes/site_branding.rs`、`docs/operations/site-branding.md`。
**依赖：** T20、T33。
**步骤：**
1. 保存配置后重启隔离服务，重新运行迁移，验证内容、图片和版本保持。
2. 对隔离数据库执行一致性备份并恢复到另一隔离实例，验证配置及所有引用图片可读。
3. 写明数据库卷持久化、备份命令、图片体积与解码容量、24 小时期限及恢复默认语义。
**验证：** I 和隔离备份恢复演练；记录实际命令与恢复前后摘要，禁止使用业务数据库。

## T36：质量门禁与验收证据

**文件：** `docs/specs/site-branding/verification.md`、审批后的 `checklist.md`。
**依赖：** T1–T35。
**步骤：**
1. 按下方门禁执行一次必要的完整回归，失败先修复再重跑受影响检查。
2. 按 checklist 执行行为验收，记录真实结果、命令、截图/追踪和环境限制。
3. 完成项标记为已验证，缺环境或未执行项保留未通过状态；按逻辑组提交，提交前仅暂存本功能文件。
**验证：** 门禁和 checklist 的全部必需项通过；不能以文档或编译代替运行期行为证据。

## 执行顺序

实际按编号顺序执行，不启用子代理。以下依赖表示可独立开展的工作关系：

```text
T1 → T2、T3、T5、T10
T4 + T5 → T6
T1 + T5 → T7 → T8 → T9（另依赖 T3）
T10 → T11 → T12
T4 + T6 + T7 → T13
T2 + T3 + T9 + T13 → T14 → T15
T8 + T12 + T13 → T16
T13 + T16 → T17
T14 + T15 → T18 → T19（另依赖 T16）
T9 + T16 → T20
T1 + T3 → T21 → T22（另依赖 T17）、T23
T22 + T23 → T24、T25
T4 + T21 → T26
T18 + T19 + T21 → T27
T26 + T27 → T28 → T29 → T30（另依赖 T23、T24）
T28 + T29 + T30 → T31
T14 + T15 + T31 → T32
T20 + T24 + T25 + T31 + T32 → T33 → T34
T20 + T33 → T35
T1–T35 → T36
```

## 完整质量门禁

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features -- --test-threads=1
cargo test --workspace --doc --all-features
cargo machete
cargo audit
pnpm --filter @yufu/web test
pnpm --filter @yufu/web check
pnpm --filter @yufu/web lint
pnpm build:web
cargo build --bin yufu --features webui
pnpm --filter @yufu/web test:e2e
```

前端先构建再验证嵌入二进制。数据库测试串行运行。完整门禁只在功能整合后执行；中间任务使用聚焦验证，避免重复全量运行。若发现必须改变已批准设计的实质问题，先记录问题和具体调整建议，再按 mew-spec 提交变更确认。
