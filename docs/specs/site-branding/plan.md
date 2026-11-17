# 站点品牌与认证页面配置 Plan

> 依据：已批准的 spec.md。用户在完整设计提交后指示「开发」，按认可本设计并继续推进处理。进入任务拆解；task.md 与 checklist.md 仍须按 mew-spec 流程审批后才能实现。

## 架构概览

沿用当前 Rust/Axum、SeaORM/PostgreSQL、Vue 3/Pinia 架构，在现有 yufu crate 内扩展，不新增 crate 或外部服务。

1. **领域规则**：纯函数负责配置校验、默认值、OIDC 逐项继承和变更字段计算，不访问数据库或 HTTP。
2. **设置服务**：编排当前会话认证、超级管理员与操作权限检查、图片校验、保存、恢复默认和成功审计。
3. **数据库适配**：扩展现有 IdentityTransaction，配置、图片内容与引用关系保存在同一 PostgreSQL 数据库；设置与成功审计共同提交。
4. **HTTP 接口**：公开展示接口与超级管理员管理接口分离；图片上传独立配置请求体限制，不放宽现有认证请求限制。
5. **前端**：公开品牌状态用于真实页面；设置页使用独立草稿，预览复用真实展示组件。配置请求失败不阻止页面挂载、登录或授权。

图片存储采用 PostgreSQL BYTEA。相对本地文件存储，避免新增目录权限、容器卷及跨存储备份一致性问题；相对对象存储，避免增加部署依赖。代价是数据库和备份体积增长，需要清理未使用素材。

## 核心数据结构

### 配置类型

Rust 类型使用 serde 严格拒绝未知请求字段；前端定义对应 TypeScript 类型。标识使用 UUID，版本使用正整数，时间对外使用 Unix 秒。版本持久化为 BIGINT，限制在 JavaScript 安全整数范围内，溢出时拒绝继续更新。

```rust
pub struct SiteBranding {
    pub site_name: String,
    pub site_description: String,
    pub logo_asset_id: Option<Uuid>,
    pub favicon_asset_id: Option<Uuid>,
    pub cover_asset_id: Option<Uuid>,
    pub welcome_title: String,
    pub welcome_description: String,
    pub footer: FooterConfig,
}
pub struct OidcBrandingOverride {
    pub cover_asset_id: Option<Uuid>,
    pub welcome_title: Option<String>,
    pub welcome_description: Option<String>,
}
pub struct FooterConfig {
    pub copyright_text: String,
    pub filing_number: String,
    pub filing_url: Option<String>,
    pub links: Vec<FooterLink>,
}
pub struct FooterLink { pub label: String, pub url: String }
pub struct SiteSettingsDocument {
    pub branding: SiteBranding,
    pub oidc: OidcBrandingOverride,
}
pub struct SiteSettings {
    pub document: SiteSettingsDocument,
    pub revision: u64,
    pub updated_by: Option<i32>,
    pub updated_at: Option<u64>,
}
pub struct SaveSiteSettings {
    pub expected_revision: u64,
    pub document: SiteSettingsDocument,
}
pub struct ResetSiteSettings { pub expected_revision: u64 }
```

字符串上限按 Unicode 字符数计算：网站名 80、简介 300、欢迎标题 120、说明文案 1000、版权文字 300、备案号 100；链接最多 10 个，名称 80、地址 2048。去除首尾空白后网站名不能为空；禁止 NUL 等无效控制字符，说明文案允许换行。链接必须是具有主机名且无用户名密码的绝对 HTTP/HTTPS 地址；备案链接为空时，备案号可仅作文字展示。

OIDC 每个 null 字段独立继承全站值；文字空字符串表示明确隐藏该项，区别于继承。全站图片 null 使用内置默认外观，OIDC 图片 null 继承全站封面。本期不增加“单独隐藏 OIDC 封面”的第三种状态。

### 图片与公开展示类型

- `SiteAssetKind`：`logo | favicon | cover`。OIDC 封面也使用 cover；保存时验证用途匹配，不能将封面素材用作图标。
- `SiteAsset`：`id: Uuid`、`kind: SiteAssetKind`、`mime: String`、`width/height: u32`、`byte_size: u64`、`sha256: String`、`bytes: Vec<u8>`、`uploaded_by: i32`、`created_at: u64`、`unreferenced_since: Option<u64>`。图片内容不可原地修改，更换图片生成新 ID。
- `SiteAssetMetadata`：上述字段中去掉 bytes、上传人及清理时间，增加管理预览地址，用于上传响应。
- `PublicSiteBranding`：`revision`、公开的站点品牌、普通认证页面展示字段、已解析继承的 OIDC 展示字段及页脚。图片 ID 转为服务端生成的同源公开地址；不包含修改人、上传人、原始覆盖状态和上传限制。
- `SiteSettingsAdminResponse`：`settings: SiteSettings`、`limits: SiteSettingsLimits`，包含原始覆盖状态及字段、格式、字节数和尺寸限制。

### 数据表

| 表 | 字段与约束 |
| --- | --- |
| `site_settings` | `id SMALLINT PRIMARY KEY CHECK(id=1)`、`schema_version`、`document JSONB`、`revision BIGINT`、`updated_by`、`updated_at`。迁移创建唯一默认行；升级不覆盖现有配置。 |
| `site_asset` | UUID 主键、用途、MIME、宽高、字节数、SHA-256、BYTEA 内容、上传人、创建时间、失去引用时间。约束用途及正尺寸，按用途约束文件字节数，字节数与内容长度一致。 |
| `site_settings_asset_ref` | `slot` 主键，仅允许 `logo/favicon/cover/oidc_cover`；`settings_id` 外键、`asset_id` 外键，图片外键使用删除限制。每次保存从配置重新生成全部引用。 |

引用表服务于公开可见性、外键完整性和清理。配置与引用只能在同一事务内更新；读取配置只取元数据，不加载图片内容。migration 固化初始配置快照；运行期恢复默认使用版本库中的默认配置文件。

### 核心函数与仓储接口

领域层提供以下接口，错误统一使用项目已有 IdentityError：

```rust
pub fn validate_settings(input: &SiteSettingsDocument) -> Result<(), IdentityError>;
pub fn resolve_branding(input: &SiteSettingsDocument) -> ResolvedSiteBranding;
pub fn changed_fields(before: &SiteSettingsDocument, after: &SiteSettingsDocument) -> Vec<String>;
pub fn referenced_assets(input: &SiteSettingsDocument) -> Vec<SiteAssetReference>;
```

`SiteAssetReference` 包含 slot、asset_id 和所需用途。`ResolvedSiteBranding` 包含全站品牌及两套已解析认证外观；公开地址由服务层生成。

在现有 IdentityTransaction 增加以下方法，其实现位于 PostgreSQL 适配模块：

```rust
async fn site_settings(&mut self, for_update: bool) -> Result<SiteSettings, IdentityError>;
async fn site_asset_metadata(&mut self, id: Uuid) -> Result<Option<SiteAssetMetadata>, IdentityError>;
async fn site_asset_content(&mut self, id: Uuid, published_only: bool) -> Result<Option<StoredSiteAsset>, IdentityError>;
async fn insert_site_asset(&mut self, asset: &NewSiteAsset) -> Result<(), IdentityError>;
async fn replace_site_settings(&mut self, input: &SaveSiteSettings, actor_id: i32) -> Result<SiteSettings, IdentityError>;
async fn cleanup_site_assets(&mut self, before: u64, limit: u32) -> Result<u64, IdentityError>;
```

`StoredSiteAsset` 包含返回图片所需 MIME、摘要、字节内容；`NewSiteAsset` 包含服务端生成 ID、验证后的用途/格式/尺寸/摘要/内容及上传人。写入时间由事务可信时钟赋值。保存方法仅在已认证、已授权且持有配置行锁的服务流程中调用，并再次用 revision 条件更新；不自行开启另一事务。

`SiteSettingsService` 持有 `Arc<dyn IdentityRepository>` 和时钟，提供 `public_branding(budget)`、`admin_settings(secret, budget)`、`save(evidence, input, request_id, budget)`、`reset(evidence, expected_revision, request_id, budget)`、`upload(evidence, kind, bytes, request_id, budget)`、`asset(secret: Option<&str>, id, budget)`。写入凭据沿用现有 BrowserWriteEvidence；读写返回领域结果，HTTP 状态与响应头由路由层处理。公开图片读取时 secret 为 None，强制 published_only；管理预览需要当前超级管理员及预览操作权限。

## 模块设计

### 超级管理员与操作授权

- 在 AccountSnapshot 增加 `super_administrator: bool`，由当前用户关联的数据库角色是否为既有 Superadmin 角色判定，沿用现有角色识别语义；不按用户名、固定用户 ID 或前端参数判定。
- 登录和账户会话响应增加该字段；前端 CenterSession、路由 qualification、导航筛选增加 `super_administrator`，站点设置路径为 `/admin/site-settings`。
- 增加五个 IdentityOperation：`SiteSettingsRead`、`SiteSettingsSave`、`SiteSettingsReset`、`SiteAssetUpload`、`SiteAssetPreview`，注册到准确的方法和路径。新迁移只插入新操作，不从旧企业管理权限复制授权，不改写既有授权或拒绝。
- 管理接口同时要求超级管理员身份和对应操作授权。即使普通企业管理员被单独授予新操作，也不能越过超级管理员检查；显式拒绝对超级管理员仍生效。
- 写入使用现有 authenticate_current_actor，先获取 Policy、当前用户和中心会话锁，再重新读取身份与权限，随后获取设置行锁。沿用全局锁顺序，避免用路由层预检代替事务内授权。

### 图片验证与资源限制

- 上传 POST 使用原始文件请求体；kind 为必填枚举。先从请求头验证来源、CSRF、会话及权限，再按用途有界读取请求体。Logo/favicon 上限 2 MiB，cover 上限 10 MiB，不能只依赖 Content-Length。
- 上传路由独立设置请求体限制；设置保存 JSON 单独允许 128 KiB，以容纳已批准的最长文案和 10 个链接；其余现有认证接口保留 16 KiB 限制。管理图片 Blob 读取和上传复用现有 HTTP 客户端凭据与错误处理。
- 使用 image crate，仅启用 PNG、JPEG、WebP、ICO 相关 codec。按实际内容识别格式，完整解码验证，宽高分别不超过 8192；ICO 仅允许 favicon，并校验容器与内嵌图像。PNG 检查 APNG、WebP 检查动画标记，动态图片拒绝；SVG/GIF 等不受支持格式拒绝。
- 解码置于 blocking 任务，限制单进程同一时间一个解码任务；读取与等待有超时，解码前检查尺寸，设置 512 MiB 解码分配预算。分配预算属于解码器的尽力限制，不宣称是进程内存硬上限；8192 × 8192 的 16 位 RGBA 原始像素约占 512 MiB，部署容量需包含解码临时开销。
- 不根据文件扩展名或浏览器 MIME 决定可接受格式；存储原始验证通过的内容与实际 MIME，服务端生成随机 ID 和 SHA-256。客户端文件名不用于路径。
- 图片解码前做当前权限预检，解码完成后在最终插入事务重新验证权限，避免长时间占用认证锁或沿用已经过期的授权。

### 图片可见性与清理

- 上传后不创建公开引用；未发布素材仅通过管理预览接口读取，公开猜中 UUID 也返回 404。
- 当前引用表是公开访问的唯一依据；移除引用后公开请求返回 404，即使素材仍保留在数据库中。不存在和未发布素材对公开调用方使用相同响应。
- 新上传图片记录 unreferenced_since；保存引用后置空，失去最后一个引用时重新记时。定期删除失去引用超过 24 小时的素材，每批最多 100 张；到期未保存草稿需要重新上传，界面说明这一规则。
- 复用现有后台清理调度，素材清理使用独立短事务，先锁设置行，再检查引用并删除，保证不与保存竞争删除正在使用的图片。清理失败记录日志后下次重试，不影响认证请求。
- 预览期间获取 Blob 并使用对象地址；设置页销毁、登出或更换图片后释放。未提交素材不写浏览器持久缓存。

### 前端配置、预览与回退

- 默认配置放在 `brand/site-branding.defaults.json`，Rust 和 Vue 从同一份文件加载，前后端默认值保持一致。保留现有 YufuMark 和默认浏览器图标，默认封面为空时显示当前内置视觉背景。
- 公开品牌状态使用独立 Pinia store，页面启动时立即可用默认配置，再异步加载公开接口；设置有限请求超时，不阻塞 app.mount，也不调用登录状态失效处理。
- 首次打开页面、整页刷新，以及普通/OIDC 认证入口切换时重新获取公开配置；不持久缓存管理草稿。公开响应进行运行时结构校验，失败时整体回退默认配置。
- 统一站点 Logo、页脚、认证视觉组件以 props 接收展示配置，真实页面使用公开 store，设置预览传入草稿解析结果。预览只渲染外观，不复用可提交的登录/授权表单，不启动真实授权流程。
- 设置页分全站品牌、普通认证外观、OIDC 覆盖、页脚四组；OIDC 每项明确显示继承或自定义，清除覆盖后立即在预览中显示继承结果。
- 保存/恢复默认成功后，更新公开 store 并建立新草稿基线；其他浏览器下次打开或刷新生效。409 时保留草稿，不自动覆盖或合并，提供确认后重新加载当前配置的操作。
- 接入现有 useFormDraft、工作区关闭确认和 beforeunload。草稿、预览 Blob 及迟到的管理请求结果按当前会话生命周期清理，不在用户切换后继续展示。
- 路由标题与品牌名称共同计算 document.title；品牌异步加载后重新计算，避免标题固定为 yufu。favicon 更新失败恢复内置图标；组件图片 error 时回退默认标志/背景，并防止无限重试。
- Logo 使用 contain，封面使用 cover；保留明暗主题遮罩与对比度；长文本可换行/截断，认证表单不被挤出可用区域，窄屏沿用现有认证布局适配。

## HTTP 接口

下列路径均位于 `/api/v1` 下。

| 方法与路径 | 请求 | 响应/权限 |
| --- | --- | --- |
| GET `/site/branding` | 无 | PublicSiteBranding，公开 |
| GET `/site/assets/{id}` | UUID | 当前已引用图片，公开，否则 404 |
| GET `/admin/site-settings` | 当前中心会话 | SiteSettingsAdminResponse，超级管理员 + SiteSettingsRead |
| PUT `/admin/site-settings` | SaveSiteSettings | 更新后的 SiteSettingsAdminResponse，超级管理员 + SiteSettingsSave |
| POST `/admin/site-settings/reset` | ResetSiteSettings | 默认配置的新版本，超级管理员 + SiteSettingsReset |
| POST `/admin/site-assets?kind=logo\|favicon\|cover` | 原始文件内容 | 201 + SiteAssetMetadata，超级管理员 + SiteAssetUpload |
| GET `/admin/site-assets/{id}` | UUID、当前中心会话 | 图片内容，超级管理员 + SiteAssetPreview |

管理读接口采用现有中心 Cookie；写接口使用现有 CSRF、Origin 校验及 BrowserWriteEvidence。管理响应和预览图片为 `Cache-Control: no-store`。公开配置使用 no-store，公开图片使用 `no-cache, must-revalidate` 并支持基于摘要的 ETag；返回 304 前也必须验证当前公开引用，避免已移除图片继续被重新验证为有效。图片设置实际 Content-Type 和 nosniff，不允许跨源读取管理素材。

沿用项目统一错误体及 request_id：无会话 401、权限或请求来源校验失败 403、参数/解码无效 400、图片不存在/未发布 404、版本冲突 409、请求体超限 413、依赖或解码资源暂不可用 503。公开配置依赖失败返回错误，由前端回退默认；不能将数据库故障当成默认配置成功响应。管理读取失败不显示可保存的默认草稿，避免误覆盖真实配置。

## 模块交互

### 页面打开

`main.ts → 品牌 store 的内置默认值 → 页面挂载`；同时 `公开 API → 设置服务 → 当前配置及图片元数据 → 继承解析 → PublicSiteBranding → store → Logo/认证布局/页脚/标题/favicon`。这一请求不携带管理权限，不影响中心会话验证；失败则保留默认外观。

### 上传与预览

`设置页选择文件 → 客户端提示格式和大小 → 上传路由权限预检 → 有界读请求体 → 解码校验 → 新事务重新认证授权 → 存入 site_asset → 返回元数据 → 管理接口读取 Blob → 草稿预览`。公开配置与引用表此时不变。

### 保存

`设置页完整草稿 + expected_revision → 来源/CSRF 校验 → 字段校验 → execute_identity → 锁定并重新认证当前操作者 → 超级管理员及精确操作校验 → 锁 site_settings 单例行 → 比对 revision → 校验所有图片用途与存在性 → 更新配置/revision/引用/失去引用时间 → 写成功审计 → 提交 → 返回新基线 → 更新当前页面公开品牌状态`。

事务中的冲突重试始终携带原 expected_revision，不会重读后将旧草稿强行应用到新版本。失败回滚全部变更；不单独提交成功审计。提交结果不确定时沿用现有不确定结果反馈，不自动重放写操作；重新读取 revision 和配置确认实际结果。

### 恢复默认

`用户确认 → expected_revision → 同保存的认证、授权和行锁流程 → 写入默认配置 + 清除全部覆盖/引用 + revision 递增 → 写 reset 审计 → 提交`。保留之前使用的图片直到清理期限到达，公开接口立即按当前引用判断可见性。

### 审计与清理

保存事件 `site_settings.saved`，恢复事件 `site_settings.reset`，object_kind 为 `site_settings`、object_id 为 `1`。摘要包含 revision 和 changed_fields，不记录图片内容、整段文案或完整链接；changed_fields 使用稳定路径，如 `branding.site_name`、`oidc.cover_asset_id`、`branding.footer.links`。为现有摘要白名单追加 changed_fields，保留现有长度及纯值数组限制。复用身份审计查询页面及导出，新增动作和对象名称映射。

后台清理调用独立素材清理事务；不改变已有身份记录的保留策略。运行期间不提供历史配置回滚或素材库入口。

## 文件组织

以下为计划新增和修改的文件；文档之外的文件尚不在本阶段修改。

| 操作 | 文件 | 职责 |
| --- | --- | --- |
| 新建 | `brand/site-branding.defaults.json` | 前后端共用默认品牌配置 |
| 新建 | `yufu/src/domain/identity/site_branding.rs` | 类型、校验、继承、引用与变更计算，附领域单元测试 |
| 修改 | `yufu/src/domain/identity/mod.rs`、`account.rs`、`operation.rs` | 模块注册、超级管理员标识、新操作定义 |
| 新建 | `yufu/src/service/identity/site_settings.rs` | 设置服务、读写流程与事务编排 |
| 新建 | `yufu/src/service/identity/site_asset.rs` | 格式、动画、解码与大小校验，附图片验证测试 |
| 新建 | `yufu/src/service/identity/repository_site_settings.rs` | 配置/素材/引用存储、版本更新、清理 |
| 修改 | `yufu/src/service/identity/mod.rs`、`repository.rs`、`repository_pg.rs` | 模块及事务接口接入、账户快照查询新增标识 |
| 修改 | `yufu/src/service/identity/repository_outbox.rs`、`workers.rs` | 审计摘要白名单、素材清理调度 |
| 新建 | `yufu/src/route/site_branding.rs`、`site_settings.rs` | 公开展示、管理设置及上传/预览路由 |
| 修改 | `yufu/src/route/mod.rs`、`identity_browser.rs`、`account.rs` | 路由挂载、独立上传限制、会话响应 |
| 新建 | `yufu/src/migration/m20261117_000001_site_branding.rs` | 三张表、索引和固化默认行 |
| 新建 | `yufu/src/seed/m20261117_000002_site_branding_operations.rs` | 注册五个新操作，保留既有授权与拒绝 |
| 修改 | `yufu/src/migration/mod.rs`、`seed/mod.rs` | 追加版本化迁移顺序 |
| 修改 | `Cargo.toml`、`yufu/Cargo.toml`、`Cargo.lock` | image 依赖及必要 codec，仅实现阶段更新锁文件 |
| 新建 | `web/src/branding/types.ts`、`defaults.ts`、`resolve.ts`、`api.ts` | 品牌类型、默认值、草稿继承解析与公开 API |
| 新建 | `web/src/pinia/stores/branding.ts` | 公开品牌状态、加载与回退 |
| 新建 | `web/src/components/brand/site-logo.vue`、`site-footer.vue`、`auth-visual.vue` | 真实页面与预览共用展示组件 |
| 新建 | `web/src/pages/admin/api/site-settings.ts` | 设置读写、原始文件上传、Blob 预览 |
| 新建 | `web/src/pages/admin/views/site-settings.vue` | 设置页、草稿、冲突与恢复默认交互 |
| 新建 | `web/src/pages/admin/components/site-branding-preview.vue`、`site-asset-field.vue` | 普通/OIDC 外观预览、上传和继承控制 |
| 修改 | `web/src/main.ts`、`router/guard.ts`、`router/navigation.ts`、`router/routes.ts` | 非阻塞品牌加载、动态标题、超级管理员入口与守卫 |
| 修改 | `web/src/pages/account/api/type.ts`、`web/src/types/router.d.ts` | 会话及路由资格类型 |
| 修改 | `web/src/layouts/auth-layout.vue`、`layouts/components/Logo/index.vue`、`layouts/mode/left-top-mode.vue` | 认证页品牌、导航 Logo、工作台页脚 |
| 修改 | `web/src/components/workspace/login-form.vue`、`pages/oidc-consent/index.vue` | 明确普通/OIDC 外观上下文，保留认证行为 |
| 修改 | `web/src/style.css`、`lib/identity-labels.ts` | 图片/页脚/预览适配及审计名称 |
| 新建 | `tests/yufu/routes/site_branding.rs`、`tests/fixtures/site-branding/` | 隔离数据库、权限、并发、原子性和图片边界测试素材 |
| 修改 | `tests/yufu/routes/mod.rs`、相关测试中的 AccountSnapshot/CenterSession fixture | 新测试注册及新增会话字段兼容 |
| 新建 | `web/tests/unit/site-branding.test.ts`、`site-settings.test.ts` | 默认继承、标题/图标、草稿预览、冲突与回退 |
| 新建 | `web/tests/e2e/enterprise-sso-site-branding.spec.ts` | 设置到普通/OIDC 页面生效、权限隔离及恢复默认场景；名称匹配现有 E2E 配置 |
| 修改 | `web/tests/e2e/enterprise-sso-fixture.ts`、`enterprise-sso-webkit.mjs` | 扩展隔离测试辅助函数与旧 macOS 的 WebKit 验证；现有 runner 无须改动 |
| 新建 | `docs/operations/site-branding.md` | 配置说明、24 小时素材期限、备份恢复、容器与容量要求 |

数据库访问沿用现有 IdentityTransaction 的参数化查询方式，不为三张新表额外生成未使用的 ORM Entity。相关账户 fixture 的具体清单在 task.md 列出。

## 技术决策

| 决策点 | 选择 | 理由 |
| --- | --- | --- |
| 部署存储 | PostgreSQL JSONB + BYTEA + 引用表 | 统一事务、备份和容器持久化；配置读取不加载大图 |
| 并发控制 | 单例行锁 + expected_revision | 与现有事务框架兼容，拒绝陈旧草稿；恢复默认同样防覆盖 |
| 发布语义 | 上传暂存，保存引用后公开 | 上传和预览不影响其他用户，未发布素材不公开 |
| 超级管理员 | 当前数据库角色 + 当前操作权限 | 区分企业管理员，并保留显式拒绝优先规则 |
| 上传协议 | 单文件原始请求体 + kind | 避免 multipart 依赖，并可按用途精确限制读取量 |
| 图片处理 | 受限 codec、真实解码、动画拒绝 | 识别伪造文件并保持本期支持范围 |
| 缓存 | 公开配置 no-store，图片需重验证，管理 no-store | 刷新获得新配置，当前引用控制素材可见性 |
| 默认值 | 前后端共用 JSON，迁移冻结初始快照 | 避免语言间默认值漂移；历史迁移不随未来默认值改变 |
| 前端状态 | 已发布状态与管理草稿分离 | 未保存的预览不污染真实页面 |
| 图片回收 | 最后失去引用后 24 小时、分批清理 | 允许短期草稿和更换操作，同时限制无用素材积累 |
| 备份 | 数据库一致性备份包含全部三张表 | 品牌不依赖额外文件卷；仍需保留现有部署配置和密钥备份 |
| 测试 | 领域单测 → 隔离数据库/HTTP → Vue → 真实 OIDC E2E | 覆盖纯规则、权限、事务和用户完整操作 |

image 的 PNG 解码器提供 APNG 检测，WebP 解码器提供动画标记检测；尺寸与分配限制语义以官方文档为依据：[PngDecoder](https://docs.rs/image/0.25.10/image/codecs/png/struct.PngDecoder.html)、[WebPDecoder](https://docs.rs/image/0.25.10/image/codecs/webp/struct.WebPDecoder.html)、[Limits](https://docs.rs/image/0.25.10/image/struct.Limits.html)。实现使用经工具链兼容性与依赖审计检查的 0.25 系列版本，提交锁文件确定解析结果。

## 需求覆盖与验证安排

| 需求 | 负责模块 | 核心验证 |
| --- | --- | --- |
| F1 / N1 | 账户快照、精确操作、路由/导航 | 各角色、无会话、CSRF/来源、超级管理员显式拒绝与权限撤销 |
| F2 | 品牌 store、Logo、标题/favicon | 所有品牌位置、异步更新、坏图回退 |
| F3 / N6 | AuthLayout、AuthVisual | 桌面/移动端、浅深色主题、长文案、表单可用性 |
| F4 | 领域解析、OIDC 上下文、预览 | 逐项继承/覆盖/清除，真实登录及授权回归 |
| F5 / N2 | 校验、SiteFooter | 空项、非法链接、纯文本与两个布局 |
| F6 / N3 | SiteAsset、上传/公开/预览路由 | 文件字节/尺寸边界、伪造/损坏/动态/ICO 文件、未发布图片拒绝公开访问 |
| F7 / N4 | 事务保存、设置草稿 | 并发 409、失败回滚、未保存隔离、离开提示、会话切换清理 |
| F8 / N7 | 默认值、迁移、数据库存储 | 确认/取消恢复默认、重启、升级不覆盖、隔离备份恢复演练 |
| F9 | 现有身份审计 | 操作者/时间/字段列表、失败不产生成功记录、筛选及导出 |
| N5 | 公开加载与回退、响应头 | 新页面/刷新生效、配置失败仍能认证、图片移除后重新验证不返回 304 |

后续 task.md 按 Rust 技能的接口先行和测试先行顺序拆解；checklist.md 将上述覆盖落实到可运行命令和可观察场景。集成测试仅使用隔离 PostgreSQL，串行执行，不操作业务数据库。当前仅完成设计文档，不宣称运行期功能或测试已经通过。
