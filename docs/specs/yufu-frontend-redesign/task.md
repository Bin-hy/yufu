# yufu 工作台改版任务单

状态：需求、视觉方向、技术设计、任务单和验收清单均已获用户批准，2026-11-08；前端实现与工程自动化验证已完成，真实环境及未覆盖项见 [验收报告](acceptance-report.md)。

依据：[spec.md](spec.md)、[plan.md](plan.md)、[audit.md](audit.md) 和 [视觉方向](design-direction.svg)。四份文档均已批准，本单实现与工程检查已执行，详见进度和验收报告。

## 实施规则与验证命令

每项围绕一个状态契约、组件或失败路径展开。编辑以2–5分钟可检查的小步骤推进；发现步骤仍过大时在原任务下细分并保留依赖。全量构建、浏览器运行及真实联调耗时按环境记录，不把任务数量换算为承诺工期。

文件清单使用仓库根目录相对路径；各任务的文件使用 `web/` 相对路径。任务中的 `../../../` 指向仓库根，例如 `../../../pnpm-lock.yaml`。原清单为批准时的规划；实际文件聚合和接口命名差异见本文实施记录。

从仓库根执行以下命令。缩写只用于本文验证栏：

- `check`：`pnpm --filter @yufu/web check`（已实现，对应vue-tsc -b）。
- `lint`：`pnpm --filter @yufu/web lint`（已实现）。
- `U name`：`pnpm --filter @yufu/web exec vitest run tests/unit/name.test.ts`。
- `E name`：`pnpm --filter @yufu/web exec playwright test tests/e2e/name.spec.ts --project=chromium`；最终再跑三个引擎。
- `test`：`pnpm --filter @yufu/web test`。
- `B`：`pnpm --filter @yufu/web exec vite build --outDir /tmp/yufu-redesign-dist`。
- `test:e2e`：`pnpm --filter @yufu/web test:e2e`。

命令、依赖与测试已实现；42项单测和71场景×3引擎全部通过。原U/E name为逻辑验证域，实际聚合文件见 [逐例索引](evidence/after/test-index.md)。每项实施后按验证栏检查，证据通过才登记完成；基础设施任务只证明配置可加载，不能用空测试集宣称业务通过。

公共Ui契约迁移会暂时影响旧调用方，所有调用方完成适配并通过回归后才形成可交付版本。业务读取模拟只验证前端；真实认证与设备验证单独记录。共享包本轮不预先改动，若实际修改须检索调用方并验证其应用。

## 执行分组

| 分组 | 任务 | 主要结果 |
|---|---|---|
| 工程基础 | T001–T004 | 依赖锁定、单测/浏览器环境、有效lint |
| 路由、标签与页面缓存 | T005–T018 | 叶子路由、唯一首页、独立上下文、公开KeepAlive验证 |
| 会话、HTTP与查询基础 | T019–T033 | owner/epoch、取消/刷新队列、URL查询、分页选项 |
| 草稿、浮层与导航事务 | T034–T054 | 受控确认、草稿保护、原子关闭、启动恢复与退出 |
| 视觉系统与响应式壳 | T055–T065 | 语义token、主题、组件、侧栏/顶栏/标签/首页 |
| 用户与角色样板 | T066–T074 | 分页状态、创建/修改/危险操作及失败回归 |
| 两类权限与分配 | T075–T087 | 完整授权基线、差异提交、候选继续加载 |
| 应用与授权 | T088–T099 | 客户端/应用多步状态、一次性密钥、撤销 |
| 审计、个人中心与用户覆写 | T100–T108 | 独立首屏、双表单、两类用户覆写 |
| 认证入口与迁移清理 | T109–T118 | 普通/OIDC流程、错误页、旧调用及无引用包装 |
| 回归与真实验收 | T119–T133 | 五视口、多引擎、真实接口/设备及逐项报告 |

任务编号供追踪，实际顺序以依赖图为准，存在少量跨分组前置依赖：草稿确认依赖已实现的确认宿主，完整授权测试依赖两类读取器。浏览器焦点用例依赖已接入busy状态的用户操作页面。禁止为按编号顺序执行而绕过依赖。

## 关键交付节点

| 节点 | 必须先满足 | 可审阅的结果 |
|---|---|---|
| 导航/缓存基础 | T006、T008、T011、T014、T018 | 标签身份正确、路由兼容、独立缓存可保留且可销毁 |
| 请求/会话/关闭基础 | T022、T024、T028、T031、T033、T036、T047、T051、T054 | 乱序与重复请求被拦截，关闭取消不丢页，会话状态隔离 |
| 用户/角色样板 | T072、T074 | 可运行样板及慢请求/失败/草稿回归 |
| 业务全覆盖 | T085、T086、T087、T097、T099、T101、T104、T108、T114 | 两类权限、应用、授权、记录、个人中心、覆写与入口全部迁移 |
| 工程交付 | T123、T124、T125、T126、T127、T128、T129 | 实现截图、类型/lint/单测/构建、多引擎回归证据 |
| 完整验收 | T130、T131、T132、T133 | 真实认证/设备检查及AC逐项结果，缺失环境明确未验证 |

## 文件清单

| 操作 | 仓库相对路径 | 对应任务 |
|---|---|---|
| 新建 | `web/eslint.config.js` | T004、T125、T126 |
| 修改 | `web/index.html` | T056 |
| 修改 | `web/package.json` | T001、T002、T003、T004、T125、T126 |
| 新建 | `web/playwright.config.ts` | T003、T127、T128、T129 |
| 修改 | `web/src/app.vue` | T046 |
| 修改 | `web/src/common/utils/auth.ts` | T053 |
| 新建 | `web/src/components/Feedback/confirm-host.vue` | T046 |
| 修改 | `web/src/components/Feedback/toast-host.vue` | T048 |
| 新建 | `web/src/components/ui/alert-dialog/alert-dialog.vue` | T041 |
| 修改 | `web/src/components/ui/button/button.vue` | T037 |
| 修改 | `web/src/components/ui/checkbox/checkbox.vue` | T038 |
| 修改 | `web/src/components/ui/dialog/dialog.vue` | T040、T118 |
| 新建 | `web/src/components/ui/drawer/drawer.vue` | T042 |
| 新建 | `web/src/components/ui/dropdown-menu/dropdown-menu.vue` | T043 |
| 修改 | `web/src/components/ui/input/input.vue` | T037 |
| 修改 | `web/src/components/ui/pagination/pagination.types.ts` | T059 |
| 修改 | `web/src/components/ui/pagination/pagination.vue` | T059 |
| 修改 | `web/src/components/ui/select/select.vue` | T038 |
| 修改 | `web/src/components/ui/theme-switcher/theme-switcher.types.ts` | T056 |
| 修改 | `web/src/components/ui/theme-switcher/theme-switcher.vue` | T056 |
| 新建 | `web/src/components/ui/tooltip/tooltip.vue` | T043 |
| 新建 | `web/src/components/workspace/data-state.vue` | T058 |
| 新建 | `web/src/components/workspace/page-header.vue` | T058 |
| 新建 | `web/src/components/workspace/query-toolbar.vue` | T058 |
| 新建 | `web/src/components/workspace/status-badge.vue` | T058 |
| 新建 | `web/src/composables/use-async-mutation.ts` | T023 |
| 新建 | `web/src/composables/use-async-query.ts` | T025、T026、T027 |
| 新建 | `web/src/composables/use-form-draft.ts` | T034、T049 |
| 修改 | `web/src/composables/use-menu.ts` | T062 |
| 新建 | `web/src/composables/use-page-query.ts` | T029、T030 |
| 新建 | `web/src/composables/use-paged-options.ts` | T032 |
| 新建 | `web/src/composables/use-theme.ts` | T056 |
| 修改 | `web/src/http/axios.ts` | T020、T021 |
| 新建 | `web/src/layouts/auth-layout.vue` | T109 |
| 修改 | `web/src/layouts/components/AppMain/index.vue` | T017 |
| 新建 | `web/src/layouts/components/Breadcrumb/index.vue` | T063 |
| 修改 | `web/src/layouts/components/Footer/index.vue` | T061 |
| 修改 | `web/src/layouts/components/Logo/index.vue` | T062 |
| 修改 | `web/src/layouts/components/NavigationBar/index.vue` | T053、T063 |
| 修改；无引用后才删除 | `web/src/layouts/components/NavigationBar/item.vue` | T117 |
| 修改；无引用后才删除 | `web/src/layouts/components/NavigationBar/link.vue` | T117 |
| 修改 | `web/src/layouts/components/Sidebar/index.vue` | T062 |
| 修改 | `web/src/layouts/components/TagsView/index.vue` | T064 |
| 修改 | `web/src/layouts/index.vue` | T061 |
| 修改 | `web/src/layouts/mode/left-top-mode.vue` | T017、T061、T118 |
| 修改 | `web/src/main.ts` | T052、T056 |
| 修改 | `web/src/pages/applications/api/index.ts` | T088 |
| 新建 | `web/src/pages/applications/components/application-form.vue` | T094 |
| 新建 | `web/src/pages/applications/components/secret-result.vue` | T095 |
| 新建 | `web/src/pages/applications/composables/use-application-registration.ts` | T090、T092 |
| 修改；无引用后才删除 | `web/src/pages/applications/index.vue` | T117 |
| 修改 | `web/src/pages/applications/views/application-list.vue` | T089、T096 |
| 修改 | `web/src/pages/authority/api/index.ts` | T075 |
| 修改 | `web/src/pages/authority/api/type.ts` | T075 |
| 新建 | `web/src/pages/authority/composables/use-authority-assignment.ts` | T079、T081 |
| 修改；无引用后才删除 | `web/src/pages/authority/index.vue` | T117 |
| 修改 | `web/src/pages/authority/views/authority-assign.vue` | T085 |
| 修改 | `web/src/pages/authority/views/authority-list.vue` | T077 |
| 修改 | `web/src/pages/authorized-apps/api/index.ts` | T098 |
| 修改；无引用后才删除 | `web/src/pages/authorized-apps/index.vue` | T117 |
| 修改 | `web/src/pages/authorized-apps/views/authorized-apps-list.vue` | T098 |
| 修改 | `web/src/pages/dashboard/api/index.ts` | T065 |
| 修改；无引用后才删除 | `web/src/pages/dashboard/components/dashboard-introduction.vue` | T117 |
| 修改；无引用后才删除 | `web/src/pages/dashboard/index.vue` | T117 |
| 修改 | `web/src/pages/dashboard/views/dashboard-home.vue` | T065 |
| 修改 | `web/src/pages/errors/403.vue` | T115 |
| 修改 | `web/src/pages/errors/404.vue` | T115 |
| 修改 | `web/src/pages/login/api/index.ts` | T066、T112 |
| 修改 | `web/src/pages/login/index.vue` | T110 |
| 修改 | `web/src/pages/login/register.vue` | T111 |
| 修改 | `web/src/pages/oauth/api/index.ts` | T088 |
| 修改；无引用后才删除 | `web/src/pages/oauth/index.vue` | T117 |
| 修改 | `web/src/pages/oauth/views/oauth-create.vue` | T116 |
| 修改 | `web/src/pages/oauth/views/oauth-detail.vue` | T116 |
| 修改 | `web/src/pages/oauth/views/oauth-list.vue` | T116 |
| 修改 | `web/src/pages/oidc-consent/api/index.ts` | T113 |
| 修改 | `web/src/pages/oidc-consent/index.vue` | T113 |
| 修改 | `web/src/pages/oidc-login/index.vue` | T112 |
| 修改 | `web/src/pages/permissions/api/index.ts` | T076 |
| 修改 | `web/src/pages/permissions/api/type.ts` | T076 |
| 新建 | `web/src/pages/permissions/composables/use-permission-assignment.ts` | T082、T084 |
| 修改；无引用后才删除 | `web/src/pages/permissions/index.vue` | T117 |
| 修改 | `web/src/pages/permissions/views/permission-assign.vue` | T086 |
| 修改 | `web/src/pages/permissions/views/permission-list.vue` | T078 |
| 修改 | `web/src/pages/profile/api/index.ts` | T019、T102 |
| 修改 | `web/src/pages/profile/index.vue` | T102、T103 |
| 修改 | `web/src/pages/recordings/api/index.ts` | T100 |
| 修改；无引用后才删除 | `web/src/pages/recordings/index.vue` | T117 |
| 修改 | `web/src/pages/recordings/views/recordings-list.vue` | T100 |
| 修改 | `web/src/pages/redirect/index.vue` | T115 |
| 修改 | `web/src/pages/roles/api/index.ts` | T066 |
| 修改；无引用后才删除 | `web/src/pages/roles/index.vue` | T117 |
| 修改 | `web/src/pages/roles/views/role-list.vue` | T073 |
| 修改 | `web/src/pages/users/api/index.ts` | T066、T105 |
| 新建 | `web/src/pages/users/components/authority-overrides.vue` | T105 |
| 新建 | `web/src/pages/users/components/permission-overrides.vue` | T106 |
| 新建 | `web/src/pages/users/components/user-overrides-dialog.vue` | T107 |
| 修改；无引用后才删除 | `web/src/pages/users/index.vue` | T117 |
| 修改 | `web/src/pages/users/views/user-create.vue` | T070、T071 |
| 修改 | `web/src/pages/users/views/user-list.vue` | T067、T068、T069、T107 |
| 修改 | `web/src/pinia/stores/layout.ts` | T062 |
| 修改 | `web/src/pinia/stores/tabs.ts` | T010 |
| 修改 | `web/src/pinia/stores/toast.ts` | T048 |
| 修改 | `web/src/pinia/stores/user.ts` | T019、T053 |
| 修改；无引用后才删除 | `web/src/router/config.ts` | T007、T117 |
| 修改 | `web/src/router/guard.ts` | T049、T052 |
| 修改；无引用后才删除 | `web/src/router/helper.ts` | T117 |
| 修改 | `web/src/router/index.ts` | T007、T052 |
| 新建 | `web/src/router/navigation.ts` | T009 |
| 修改 | `web/src/router/routes.ts` | T007 |
| 新建 | `web/src/storage/workspace.ts` | T012、T013 |
| 修改 | `web/src/style.css` | T055、T117、T118 |
| 修改 | `web/src/types/router.d.ts` | T005 |
| 新建 | `web/src/types/workspace.ts` | T005 |
| 修改 | `web/src/utils/modal.ts` | T046 |
| 修改 | `web/src/utils/toast.ts` | T048 |
| 新建 | `web/src/workspace/coordinator.ts` | T050 |
| 新建 | `web/src/workspace/draft-registry.ts` | T035、T049 |
| 新建 | `web/src/workspace/page-context.ts` | T015 |
| 新建 | `web/src/workspace/page-registry.ts` | T016 |
| 新建 | `web/src/workspace/session.ts` | T019、T052、T053 |
| 新建 | `web/src/workspace/tab-identity.ts` | T005 |
| 修改 | `web/tailwind.config.js` | T055 |
| 新建 | `web/tests/e2e/applications.spec.ts` | T097 |
| 新建 | `web/tests/e2e/auth.spec.ts` | T114 |
| 新建 | `web/tests/e2e/authorized-apps.spec.ts` | T099 |
| 新建 | `web/tests/e2e/fixtures/workspace.ts` | T003、T123、T124 |
| 新建 | `web/tests/e2e/overlays.spec.ts` | T045 |
| 新建 | `web/tests/e2e/permissions.spec.ts` | T087 |
| 新建 | `web/tests/e2e/profile.spec.ts` | T104 |
| 新建 | `web/tests/e2e/recordings.spec.ts` | T101 |
| 新建 | `web/tests/e2e/roles.spec.ts` | T074 |
| 新建 | `web/tests/e2e/routes.spec.ts` | T008 |
| 新建 | `web/tests/e2e/session.spec.ts` | T054 |
| 新建 | `web/tests/e2e/user-overrides.spec.ts` | T108 |
| 新建 | `web/tests/e2e/users.spec.ts` | T072 |
| 新建 | `web/tests/e2e/visual.spec.ts` | T123、T124 |
| 新建 | `web/tests/e2e/workspace-state.spec.ts` | T121、T122 |
| 新建 | `web/tests/e2e/workspace.spec.ts` | T119、T120 |
| 新建 | `web/tests/unit/application-registration.test.ts` | T091、T093 |
| 新建 | `web/tests/unit/assignment-baseline.test.ts` | T080 |
| 新建 | `web/tests/unit/assignment-mutation.test.ts` | T083 |
| 新建 | `web/tests/unit/async-mutation.test.ts` | T024 |
| 新建 | `web/tests/unit/async-query.test.ts` | T028 |
| 新建 | `web/tests/unit/confirm-host.test.ts` | T047 |
| 新建 | `web/tests/unit/draft-registry.test.ts` | T036 |
| 新建 | `web/tests/unit/form-controls.test.ts` | T039 |
| 新建 | `web/tests/unit/form-draft.test.ts` | T036 |
| 新建 | `web/tests/unit/http.test.ts` | T022 |
| 新建 | `web/tests/unit/overlays.test.ts` | T044 |
| 新建 | `web/tests/unit/page-cache.test.ts` | T018 |
| 新建 | `web/tests/unit/page-query.test.ts` | T031 |
| 新建 | `web/tests/unit/paged-options.test.ts` | T033 |
| 新建 | `web/tests/unit/pagination.test.ts` | T060 |
| 新建 | `web/tests/unit/routes.test.ts` | T008 |
| 新建 | `web/tests/unit/session.test.ts` | T054 |
| 新建 | `web/tests/unit/setup.ts` | T002 |
| 新建 | `web/tests/unit/tab-identity.test.ts` | T006 |
| 新建 | `web/tests/unit/tabs.test.ts` | T011 |
| 新建 | `web/tests/unit/theme.test.ts` | T057 |
| 新建 | `web/tests/unit/workspace-close.test.ts` | T051 |
| 新建 | `web/tests/unit/workspace-storage.test.ts` | T014 |
| 修改 | `web/tsconfig.node.json` | T002 |
| 新建 | `web/vitest.config.ts` | T002 |
| 新建（验收阶段） | `docs/specs/yufu-frontend-redesign/acceptance-report.md` | T133 |
| 修改 | `docs/specs/yufu-frontend-redesign/evidence/README.md` | T123、T124、T125、T126、T127、T128、T129、T130、T131、T132 |
| 修改 | `pnpm-lock.yaml` | T001 |

## 有序任务

### T001：固定新增依赖与安装版本

**文件：** `package.json`、`../../../pnpm-lock.yaml`

**依赖：** 无

**步骤：**

1. 核对 Reka UI、@lucide/vue 与测试依赖的实际 peer/engine 声明，选择兼容版本。
2. 仅为 yufu 增加运行依赖及 Vitest、Vue Test Utils、Playwright Test、jsdom 开发依赖，更新根锁文件。

**验证：** 执行工作区安装和 vue-tsc -b；核对锁文件只包含预期依赖变化，记录实际解析版本。

### T002：建立单元测试入口

**文件：** `package.json`、`vitest.config.ts`、`tsconfig.node.json`、`tests/unit/setup.ts`

**依赖：** T001

**步骤：**

1. 配置 Vue 插件、@ 别名与 jsdom，补齐必要 DOM 测试环境适配。
2. 新增 check、test 命令，保持测试配置与生产源码类型检查边界明确。

**验证：** 运行check，并让Vitest加载配置，确认Vue插件、@别名及jsdom可初始化；尚无业务用例时只报告基础设施可加载，业务测试由T006起逐项执行，不以空测试集宣称测试通过。

### T003：建立浏览器测试与隔离数据

**文件：** `package.json`、`playwright.config.ts`、`tests/e2e/fixtures/workspace.ts`

**依赖：** T001

**步骤：**

1. 配置独立测试端口与 Chromium、Firefox、WebKit 项目，webServer 不复用未知已有服务。
2. 提供模拟当前用户、分页数据、401、延迟及失败响应的 fixture；每个测试用独立浏览器上下文。
3. 拦截业务读写请求，禁止模拟测试向真实后端提交数据。

**验证：** 安装并核对Chromium/Firefox/WebKit引擎及配置加载；确认fixture拒绝未匹配业务请求。路由行为由T008验证，此处只确认浏览器基础设施就绪。

### T004：启用有效 Vue/TS lint 配置

**文件：** `package.json`、`eslint.config.js`

**依赖：** T001

**步骤：**

1. 接入现有 Vue 与 TypeScript ESLint 依赖，配置源码和测试文件。
2. 忽略构建产物；记录既有诊断，使用有理由的局部修正处理迁移范围内问题。

**验证：** 运行 lint 并记录实际诊断；最终 T126 要求通过，不将全局关闭规则作为修复。

### T005：定义工作台契约与标签身份

**文件：** `src/types/workspace.ts`、`src/types/router.d.ts`、`src/workspace/tab-identity.ts`

**依赖：** T002

**步骤：**

1. 定义 plan 中的 WorkspaceTab、WorkspaceSnapshot、PageContext、WorkspaceActions、DraftEntry 及请求状态。
2. 扩展 workspace/tabIdentity/tabParamKeys/persistQueryKeys/breadcrumb meta；实现 resolveTabIdentity，query 不参与身份。

**验证：** 运行 check；确认列表身份由路由名决定，对象身份仅使用声明的规范 params。

### T006：验证标签身份边界

**文件：** `tests/unit/tab-identity.test.ts`

**依赖：** T005

**步骤：**

1. 测试同路由不同 query 保持身份、同组件不同 ID 具有不同身份。
2. 覆盖缺失/重复 params、相同标题及近似路径，拒绝用标题或前缀误合并。

**验证：** U tab-identity：翻页不新增身份、对象页不相互覆盖，非法输入处理与契约一致。

### T007：规范工作台叶子路由

**文件：** `src/router/routes.ts`、`src/router/index.ts`、`src/router/config.ts`

**依赖：** T005

**步骤：**

1. 将实际业务叶子直接挂入 Layout，保留既有叶子 path/name；dashboard 只留一个固定入口。
2. 保留 introduction、oauth 旧地址重定向；引用检索后处理仍被调用的父级 name。
3. 停止在新路由上使用 flatMultiLevelRoutes；暂留尚未完成引用清理的 helper 和包装页。

**验证：** 运行 check；用 router.resolve 检查 spec 核心地址与旧 name，不产生第二个首页标签入口。

### T008：验证路由兼容与规范首页

**文件：** `tests/unit/routes.test.ts`、`tests/e2e/routes.spec.ts`

**依赖：** T003、T007

**步骤：**

1. 对核心叶子 path/name 与旧命名引用建立解析用例。
2. 浏览器直达 dashboard、introduction、oauth 旧子地址和未知页，断言最终页面与 URL。

**验证：** U routes 与 E routes：兼容跳转抵达规范地址，404 独立显示，最终只有一个首页入口（标签断言在 T120 完成）。

### T009：建立导航分组配置

**文件：** `src/router/navigation.ts`

**依赖：** T007

**步骤：**

1. 用具名路由定义工作台、身份、访问控制、应用与授权、审计入口。
2. 明确 API/资源列表与分配标题，补精确活动路由映射；导航 href 从 router.resolve 取得。

**验证：** 运行 check；枚举每个导航项都能解析到 workspace 叶子页，创建/分配页能映射正确分组。

### T010：改造标签状态所有者

**文件：** `src/pinia/stores/tabs.ts`

**依赖：** T005、T007

**步骤：**

1. 按 key 保存 routeName/fullPath/title/cacheName/scrollTop，固定首页唯一。
2. 提供成功导航同步、原子删除和 reset 操作；query 更新同一标签，禁止 store 自行先删后导航。

**验证：** 运行 check；通过 T011 验证同步、保留固定首页及批量删除结果。

### T011：验证标签更新与重置

**文件：** `tests/unit/tabs.test.ts`

**依赖：** T010

**步骤：**

1. 覆盖翻页更新 fullPath、重复成功导航、无 workspace 路由、固定首页。
2. 覆盖原子关闭集合与 reset；验证不会残留活动 key 或旧账号标题。

**验证：** U tabs：数据不重复，首页不被批量删除，reset 清除业务标签。

### T012：实现快照编码与地址白名单

**文件：** `src/storage/workspace.ts`

**依赖：** T005、T007

**步骤：**

1. 只序列化版本、ownerId、允许的 routeName/fullPath/scrollTop、activeKey。
2. 按 route meta 过滤 query 并验证 route/owner/version；剔除 OIDC、token、secret、密码、草稿和结果数据。

**验证：** 运行 check；通过 T014 的敏感字段与伪造地址用例。

### T013：接入快照存取及故障回退

**文件：** `src/storage/workspace.ts`

**依赖：** T012

**步骤：**

1. 实现 ownerId 分区 key、必要的节流写入和明确退出清除。
2. 处理 JSON 损坏、存储禁用与写入异常，回退内存；限制 storage 事件只处理本应用标识及偏好。

**验证：** 通过 T014 模拟读取/写入抛错，导航继续可用；不同 owner 快照不混用。

### T014：验证持久化范围

**文件：** `tests/unit/workspace-storage.test.ts`

**依赖：** T013

**步骤：**

1. 覆盖所有允许路由、未知 query、旧版本、损坏 JSON、owner 不匹配和未知路由。
2. 模拟 localStorage 抛错，检查完整存储文本不含密码、secret、token 或表单内容。

**验证：** U workspace-storage：可恢复合法分页/顺序/滚动，无敏感内容，存储故障不抛出阻断页面的异常。

### T015：实现每页独立上下文

**文件：** `src/workspace/page-context.ts`

**依赖：** T005

**步骤：**

1. 建立类型化 provide/inject 的 PageContext。
2. 所属标签成功导航才更新 shallow route snapshot；active 与 sessionEpoch 各自可读，避免失活页跟随全局 query。

**验证：** 运行 check；用 T018 的两个页面验证独立快照和激活状态。

### T016：实现稳定缓存包装注册

**文件：** `src/workspace/page-registry.ts`

**依赖：** T010、T015

**步骤：**

1. 每个 tabKey 创建稳定 defineComponent wrapper 和单调 cacheName，叶子组件与 props 固定绑定。
2. 组件类型不进入深代理；提供条目释放和整会话 reset，不访问 Vue 私有缓存。

**验证：** 运行 check；同 tab 多次解析返回同 wrapper，不同对象得到不同 cacheName。

### T017：接入唯一 AppMain 缓存及滚动

**文件：** `src/layouts/components/AppMain/index.vue`、`src/layouts/mode/left-top-mode.vue`

**依赖：** T016

**步骤：**

1. 在真实布局接入 RouterView slot 与单一 KeepAlive，以打开标签 cacheName 控制 include。
2. 内容容器负责滚动保存/恢复；移除已迁移页的重复 min-h-screen/cache 容器。
3. 会话 epoch 改变销毁所有页面；include 删除后下一轮更新释放注册条目。

**验证：** 运行 check；用 T018 验证失活保留、关闭销毁、重开为新实例及滚动恢复。

### T018：验证公开缓存机制及失活页面

**文件：** `tests/unit/page-cache.test.ts`

**依赖：** T017

**步骤：**

1. 用真实 Vue Router/KeepAlive 测试两个页面、同组件不同 params、关闭后重开。
2. 记录挂载/激活/卸载和请求次数，确认 route snapshot 不串页，关闭释放监听。
3. 公开 include 机制若不能满足契约，先修订 plan 的具体方案，再继续业务迁移。

**验证：** U page-cache：独立实例、正确清理及草稿保留均通过；此项是全模块迁移的前置条件。

### T019：建立会话 owner 与 epoch

**文件：** `src/workspace/session.ts`、`src/pinia/stores/user.ts`、`src/pages/profile/api/index.ts`

**依赖：** T010、T013、T016

**步骤：**

1. 复用 GET user 返回的稳定 ID 建立 owner；未知 ID 时使用内存模式。
2. 提供读取请求注册/取消、epoch 递增与 reset 接口；不解析 token 推断账号。
3. 用户信息存取处理存储异常，避免启动因损坏 localStorage 阻断。
4. reset回调由启动入口注册，session不反向依赖router或页面组件；冷启动避免新增循环初始化。

**验证：** 运行 check；用 T054 验证同昵称不同 ID、未知 owner、账号切换与强制失效。

### T020：统一 HTTP 取消与错误归属

**文件：** `src/http/axios.ts`

**依赖：** T019

**步骤：**

1. 扩展本地 errorPresentation 选项并原样传递 signal，保留 baseURL、cookie 和 bearer 约定。
2. 取消错误直接 reject，不 toast、不 refresh、不退出；保留可读的服务端错误。
3. 避免深合并把 AbortSignal 变成不可用对象。

**验证：** 通过 T022 验证取消静默、local 不重复全局提示、403 不退出及普通旧调用反馈。

### T021：修复刷新等待队列

**文件：** `src/http/axios.ts`

**依赖：** T020

**步骤：**

1. 同批 401 共用刷新操作，成功只重发未取消的等待请求。
2. 失败、重复 401 和会话 reset 释放所有等待者；旧 epoch 不写入新 token。
3. 登录失败不刷新，OIDC 参数沿原流程保留。

**验证：** 通过 T022：并发 401 仅一次 refresh，失败后没有永久 pending；取消请求不会被刷新复活。

### T022：验证 HTTP 刷新与取消

**文件：** `tests/unit/http.test.ts`

**依赖：** T021

**步骤：**

1. 使用模拟 Axios adapter 覆盖普通错误、401 并发、刷新失败、取消及 epoch 改变。
2. 断言真实请求次数、Promise settle、token 更新归属和 toast 次数。

**验证：** U http：401 队列全部 settle，旧会话不能刷新新账号状态，403 只产生局部拒绝访问。

### T023：实现写操作生命周期

**文件：** `src/composables/use-async-mutation.ts`

**依赖：** T019、T020

**步骤：**

1. 实现 pending/error 和重复提交锁；同列表行按业务对象分别持有状态。
2. 不自动重试非幂等写操作、不把关闭界面解释为取消后端；epoch 变化阻止结果回写。

**验证：** 运行 check；通过 T024 模拟慢写与换账号后的迟到成功。

### T024：验证重复提交与迟到写响应

**文件：** `tests/unit/async-mutation.test.ts`

**依赖：** T023

**步骤：**

1. 1 秒 pending 期间重复调用只发一次；失败保留错误并允许明确重试。
2. 会话切换后旧成功/失败不能调用新页面的关闭、刷新或错误更新。

**验证：** U async-mutation：重复写次数为 1，失败与成功状态准确，旧 epoch 不回写。

### T025：建立读取请求的激活调度

**文件：** `src/composables/use-async-query.ts`

**依赖：** T015、T019、T020

**步骤：**

1. 实现 QueryState 与单一 active/key watcher，首次只调度一个读取请求。
2. 每次 loader 接收新的 signal，失活/卸载取消当前读取。

**验证：** U async-query 首访/失活用例：首次请求为1，失活后不继续请求；完整回归在后续读取测试任务运行。

### T026：阻止读取乱序及旧 finally 回写

**文件：** `src/composables/use-async-query.ts`

**依赖：** T025

**步骤：**

1. 用请求序号与 sessionEpoch 同时校验 data/error/status/finally 的所有更新。
2. 新查询使旧序号失效，即使取消未被服务端/loader尊重也不能覆盖当前页。

**验证：** U async-query 延迟用例：page2慢/page3快只显示page3，旧finally不提前结束新loading。

### T027：统一读取缓存、失效与失败状态

**文件：** `src/composables/use-async-query.ts`

**依赖：** T026

**步骤：**

1. 未失效页面激活复用缓存，stale激活及显式刷新发起必要读取。
2. 首次失败与刷新失败区分；不同query不冒充旧结果，相同query刷新失败保留旧数据和重试说明。

**验证：** U async-query 缓存/刷新用例：请求次数正确，错误/空/刷新状态符合plan。

### T028：验证读取乱序与请求次数

**文件：** `tests/unit/async-query.test.ts`

**依赖：** T027

**步骤：**

1. 用可控 Promise 模拟 page2 慢、page3 快且取消无效的响应，验证最后查询生效。
2. 覆盖 mount/activate 单请求、失活不请求、重新激活缓存、失效刷新和旧 finally。

**验证：** U async-query：page3 行与状态始终正确；page2 不提前结束 page3 loading，首访请求为 1。

### T029：解析和校验页面 URL 查询

**文件：** `src/composables/use-page-query.ts`

**依赖：** T015、T007

**步骤：**

1. 按schema从当前PageContext解析页码、页大小、q及已有筛选。
2. 拒绝重复数组、NaN、小数等输入，非法首次地址仅replace规范一次。

**验证：** U page-query 参数用例：非法参数归一为合法值，失活页不会修改其他页面地址。

### T030：实现分页筛选动作与历史同步

**文件：** `src/composables/use-page-query.ts`

**依赖：** T029

**步骤：**

1. 实现setPage/setSize/setFilters，筛选和大小变化回第1页，push/replace各按plan。
2. 总页数超界只修正重查一次，零结果停第1页；历史导航读取独立route snapshot。

**验证：** U page-query 历史/边界用例：前进后退同步，无replace或重查循环。

### T031：验证查询及历史同步

**文件：** `tests/unit/page-query.test.ts`

**依赖：** T030

**步骤：**

1. 覆盖 URL 恢复、重复 query、0/负数/超界页码、非法页大小、筛选重置及零结果。
2. 真实 memory router 前进/后退同步 query，失活页不修正其他页面地址。

**验证：** U page-query：URL 与页状态一致，没有 replace/reload 循环，筛选变化只产生一次有效查询动作。

### T032：实现分页候选选项

**文件：** `src/composables/use-paged-options.ts`

**依赖：** T020、T027

**步骤：**

1. 实现 items/loading/error/hasMore/loadNext/retry，按 ID 去重并保留已加载页。
2. 首批 100，按 total_pages 继续；筛选/所属对象切换后取消旧页，页面失败不当成已全部加载。

**验证：** 运行 check；通过 T033 的多页、失败、去重和切换用例。

### T033：验证选项完整性与失败恢复

**文件：** `tests/unit/paged-options.test.ts`

**依赖：** T032

**步骤：**

1. 模拟超过 100/1000 的候选，逐页加载、重复 ID、末页和中途失败。
2. 验证重试只请求失败页，对象改变后旧结果不混入新列表，已选项不会因未加载被删除。

**验证：** U paged-options：全部页可继续获取，无静默数量上限或重复记录。

### T034：实现表单 baseline 与草稿登记

**文件：** `src/composables/use-form-draft.ts`

**依赖：** T015

**步骤：**

1. 实现规范化 baseline/dirty、初始化完成后建立基线、成功更新与失败保留。
2. 提供 DraftEntry 生命周期接入：失活保留，卸载注销；敏感字段不持久化。

**验证：** 运行 check；通过 T036 的异步默认值与失活测试。

### T035：实现草稿检查注册表

**文件：** `src/workspace/draft-registry.ts`

**依赖：** T034、T046

**步骤：**

1. 实现 register/checkNavigation/checkClose/reset，区分保留草稿切换与放弃草稿关闭。
2. 汇总批量关闭的脏表单对象；pending 写请求阻止普通离开，强制会话失效不被拦截。
3. 一次动作只有一个确认 Promise；取消时不提前调用 discard。

**验证：** 通过 T036：取消无数据丢失，批量关闭全有或全无，reset 结束等待。

### T036：验证表单与草稿语义

**文件：** `tests/unit/form-draft.test.ts`、`tests/unit/draft-registry.test.ts`

**依赖：** T035

**步骤：**

1. 测试默认角色加载后不脏、编辑变脏、失败保留、成功新 baseline。
2. 覆盖切换保留、关闭放弃、批量取消、pending 阻止、失活保留及 reset。

**验证：** U form-draft 与 U draft-registry：输入与确认语义准确，没有初次进入误警告。

### T037：统一按钮与输入契约

**文件：** `src/components/ui/button/button.vue`、`src/components/ui/input/input.vue`

**依赖：** T001

**步骤：**

1. 按钮默认 type=button，表单提交显式 submit；非 button 不传无效 disabled。
2. 输入传递 id/name/autocomplete/inputmode/aria 属性，保留模型值与校验需要的基本类型。

**验证：** 运行 check；通过 T039 验证普通按钮不误提交、字段名称及键盘提交。

### T038：规范选择与复选框值

**文件：** `src/components/ui/select/select.vue`、`src/components/ui/checkbox/checkbox.vue`

**依赖：** T037

**步骤：**

1. 保留原生 select，空值明确为 null，数值选项按契约转换。
2. Checkbox checked 只来自 modelValue，标签点击与禁用状态准确；关联 input id。

**验证：** 通过 T039：null 不变 NaN，数字不残留字符串，点击 label 只切换一次。

### T039：验证表单基础组件

**文件：** `tests/unit/form-controls.test.ts`

**依赖：** T038、T002

**步骤：**

1. 覆盖 button 默认行为、显式 submit、input 属性转发。
2. 覆盖 select null/number/string、checkbox model/label/disabled。

**验证：** U form-controls：原生表单语义和模型类型均正确，禁用项无用户事件。

### T040：改造受控编辑弹窗

**文件：** `src/components/ui/dialog/dialog.vue`

**依赖：** T001、T037

**步骤：**

1. 按安装版本的 Reka Dialog 事件实现受控 open、title/description、slots 和焦点。
2. confirm 只发事件；busy 阻止确认、Esc 与遮罩关闭，error 保留内容；允许业务拦截脏表单关闭。
3. 兼容 hide-footer/size 等现有入口，不保留自动关闭再重开的补丁。

**验证：** 运行 check；通过 T045：慢提交失败后仍显示输入，焦点限制和返回触发点准确。

### T041：实现异步危险操作确认

**文件：** `src/components/ui/alert-dialog/alert-dialog.vue`

**依赖：** T040

**步骤：**

1. 包装 Reka AlertDialog，展示对象/影响并优先聚焦取消项。
2. 异步 action 不使用会自动关闭的默认行为，业务成功才关闭；busy 保留焦点与对象。

**验证：** 通过 T045：失败不关闭、重复操作只一次、成功后焦点返回合理位置。

### T042：实现侧边导航抽屉

**文件：** `src/components/ui/drawer/drawer.vue`

**依赖：** T040

**步骤：**

1. 复用受控Dialog的侧边位置与遮罩，保留title/description、Esc与焦点返回。
2. 暴露受控open和关闭请求，供移动导航在成功导航后关闭。

**验证：** 通过浮层浏览器用例：抽屉焦点受控，Esc返回触发按钮，取消导航不误关闭。

### T043：实现菜单与 Tooltip 适配

**文件：** `src/components/ui/dropdown-menu/dropdown-menu.vue`、`src/components/ui/tooltip/tooltip.vue`

**依赖：** T042

**步骤：**

1. 按实际安装版Reka契约包装DropdownMenu与Tooltip。
2. as-child trigger保留原生link/button和名称，键盘方向键及焦点行为正确，不嵌套按钮。

**验证：** 通过浮层键盘用例：方向键选择、Esc退出、href及按钮语义保留。

### T044：验证浮层受控与异步失败

**文件：** `tests/unit/overlays.test.ts`

**依赖：** T003、T041、T043

**步骤：**

1. 覆盖Dialog/AlertDialog pending、失败、关闭请求、repeat confirm与脏表单拦截。
2. 断言失败保留open/字段，busy阻止确认和外部关闭。

**验证：** U overlays：受控关闭契约及异步失败测试通过。

### T045：验证浏览器浮层焦点

**文件：** `tests/e2e/overlays.spec.ts`

**依赖：** T044、T068、T069

**步骤：**

1. 真实浏览器验证role/title/description、Tab限制、Esc及菜单方向键。
2. 覆盖关闭返回触发点和触发对象删除后的合理焦点。

**验证：** E overlays：仅键盘完成确认/取消，焦点不逃逸或丢失。

### T046：统一全局确认宿主与 modal Promise

**文件：** `src/components/Feedback/confirm-host.vue`、`src/utils/modal.ts`、`src/app.vue`

**依赖：** T041

**步骤：**

1. 用应用级宿主接入 showModal，保留现有回调兼容入口。
2. 确认、取消、Esc、卸载和 reset 都 settle Promise，显式处理异步回调失败。
3. 确认请求串行且归属于会话，普通成功通知不强制走确认。

**验证：** 运行 check；通过 T047 验证无悬挂 Promise；为 T035 提供稳定确认依赖。

### T047：验证确认结束路径

**文件：** `tests/unit/confirm-host.test.ts`

**依赖：** T046

**步骤：**

1. 覆盖确认、取消、外部关闭、Esc、失败回调、组件卸载及会话 reset。
2. 断言每个请求只 settle 一次，未处理请求不会泄漏到下一会话。

**验证：** U confirm-host：所有关闭路径结束调用方等待，失败保留当前操作上下文。

### T048：统一 toast 与异步播报

**文件：** `src/components/Feedback/toast-host.vue`、`src/pinia/stores/toast.ts`、`src/utils/toast.ts`

**依赖：** T037

**步骤：**

1. 提供合适 live region 与状态/错误语义、关闭按钮和可读消息。
2. 页面已展示局部错误时不重复 toast；会话 reset 清除旧提示。

**验证：** 用 T045 浏览器操作观察异步播报节点；HTTP T022 校验 local 错误不会再产生全局提示。

### T049：接入导航与刷新草稿保护

**文件：** `src/router/guard.ts`、`src/composables/use-form-draft.ts`、`src/workspace/draft-registry.ts`

**依赖：** T007、T035

**步骤：**

1. 在认证检查后接入所有导航来源的草稿检查，避免一次关闭事务二次弹确认。
2. 同标签 query 变化按表单语义决定检查，beforeunload 在 dirty/重要 pending 存在时注册。
3. 强制会话失效走明确 bypass，监听随注销清理。

**验证：** 通过 T051/T054 和 T122：侧栏/tab/历史导航取消保留输入，刷新触发浏览器原生保护。

### T050：实现可取消的标签关闭事务

**文件：** `src/workspace/coordinator.ts`

**依赖：** T010、T017、T035、T049

**步骤：**

1. 实现 WorkspaceActions.navigate/close/resetSession，成功导航后才能提交删除。
2. 当前/其他/右侧先收集全部目标并检查草稿，回退左邻/右邻/首页；固定首页保留。
3. 串行关闭，新导航/epoch 改变使旧事务失效；失败不删标签、不释放缓存、不写快照。

**验证：** 运行 check；通过 T051 验证导航失败、取消以及确认晚返回。

### T051：验证关闭事务的用户行为

**文件：** `tests/unit/workspace-close.test.ts`

**依赖：** T050

**步骤：**

1. 用真实 router 与草稿宿主覆盖当前/其他/右侧、失败导航和非当前脏表单。
2. 慢确认时切到另一页/切会话，确认返回不得删除新活动页或新会话标签。

**验证：** U workspace-close：取消保持原标签与实例；成功后才删除，批量操作没有部分误删。

### T052：调整启动与工作台恢复顺序

**文件：** `src/main.ts`、`src/router/index.ts`、`src/router/guard.ts`、`src/workspace/session.ts`

**依赖：** T019、T021、T050

**步骤：**

1. Pinia 初始化后完成当前身份恢复，再安装启动 router；读取失败提供合理恢复入口。
2. 只在 afterEach 成功 workspace 导航同步 tabs/PageContext，过滤失败与中间重定向。
3. 恢复 owner 对应合法快照，未知 owner 内存运行；恢复访问被拒绝时显示明确状态。

**验证：** 运行 check；通过 T054/T122：直接 URL、刷新、初始 401/403 和损坏快照正常显示。

### T053：统一退出与账号切换清理

**文件：** `src/common/utils/auth.ts`、`src/pinia/stores/user.ts`、`src/workspace/session.ts`、`src/layouts/components/NavigationBar/index.vue`

**依赖：** T046、T048、T052

**步骤：**

1. 明确退出先完成必要草稿确认，再统一清除 snapshot/tabs/cache/draft/confirm/toast/读取请求。
2. 后端 logout 失败仍保持本地退出；账号变化先 reset 再恢复新 owner。
3. 本应用跨标签会话变化触发对应清理，不监听其他应用任意 cookie 变化。

**验证：** 通过 T054/T122：退出再登录无旧标签/输入，迟到读取无回写，取消退出保留当前页。

### T054：验证启动与会话隔离

**文件：** `tests/unit/session.test.ts`、`tests/e2e/session.spec.ts`

**依赖：** T003、T053

**步骤：**

1. 覆盖相同昵称不同 ID、未知 owner、localStorage 失败、账号替换、logout 失败及401失效。
2. 工作台已打开草稿和 pending 请求后换账号，断言标签、缓存、确认和迟到结果全部隔离。

**验证：** U session 与 E session：新账号只出现其合法导航状态，强制失效不等待草稿确认。

### T055：建立语义视觉 token

**文件：** `src/style.css`、`tailwind.config.js`

**依赖：** T037

**步骤：**

1. 按方向稿设置浅暗主题的 surface/foreground/primary/border 等语义色、字体、间距和焦点。
2. 移除需要迁移的全局灰色覆盖，定义局部滚动、reduced-motion 及触控尺寸规则。

**验证：** 运行生产构建；用 T124 截图及对比度核验实际前景背景组合。

### T056：实现主题启动与偏好

**文件：** `src/composables/use-theme.ts`、`src/components/ui/theme-switcher/theme-switcher.vue`、`src/components/ui/theme-switcher/theme-switcher.types.ts`、`src/main.ts`、`index.html`

**依赖：** T055

**步骤：**

1. 实现 light/dark/system 与挂载前 html class/color-scheme/theme-color 初始化。
2. system 才监听媒体变化；异常存储回退，卸载清理监听；偏好 key 不含身份信息。

**验证：** 通过 T057 及 T124：刷新主题一致，系统切换只影响 system 偏好，无明显首屏闪烁。

### T057：验证主题偏好及监听

**文件：** `tests/unit/theme.test.ts`

**依赖：** T056

**步骤：**

1. 模拟 matchMedia、存储异常、light/dark/system 与媒体变更。
2. 断言 resolved 主题、html 状态和监听清理，避免重复注册。

**验证：** U theme：用户明确选择不被系统覆写，系统偏好正确跟随且卸载清理。

### T058：建立统一页面与状态组件

**文件：** `src/components/workspace/page-header.vue`、`src/components/workspace/query-toolbar.vue`、`src/components/workspace/data-state.vue`、`src/components/workspace/status-badge.vue`

**依赖：** T037、T055

**步骤：**

1. 定义一致 h1、说明、操作槽和响应式查询工具栏。
2. DataState 区分首次 loading/error/empty/ready 与刷新失败保留；StatusBadge 用文本和语义色说明。

**验证：** 运行 check；用户样板 T067 展示全部状态，360px 查询操作可触达，图标或颜色不是唯一说明。

### T059：规范分页事件与输入

**文件：** `src/components/ui/pagination/pagination.vue`、`src/components/ui/pagination/pagination.types.ts`

**依赖：** T038、T055

**步骤：**

1. 统一新调用 page-change/size-change，迁移结束前保留旧接口适配。
2. 页大小 10/20/50、整数跳页、总页数边界；disabled 同时约束全部入口。

**验证：** 通过 T060：一操作只发一个分页事件，空数据和超界输入保持合法值。

### T060：验证分页边界与禁用

**文件：** `tests/unit/pagination.test.ts`

**依赖：** T059

**步骤：**

1. 覆盖页数 0/1、多页、非法跳页、页大小变更及 disabled。
2. 同时核对旧兼容事件与新页面单一事件绑定，避免重复加载。

**验证：** U pagination：整数边界准确，禁用时不发操作，单动作单事件。

### T061：重建响应式工作台壳

**文件：** `src/layouts/index.vue`、`src/layouts/mode/left-top-mode.vue`、`src/layouts/components/Footer/index.vue`

**依赖：** T017、T043、T055

**步骤：**

1. 布局使用 100dvh/vh 回退、侧栏 224/64、顶栏 56、标签 40 和 min-width/min-height:0。
2. 小于1024启用抽屉，主内容独立滚动；帮助/版权移到低优先级区域。
3. 添加 skip link 和主内容目标，页面切换不用阻塞 out-in 动画。

**验证：** 用 T124 五种视口检查正文宽度、焦点遮挡和滚动；手机正文不再仅134px。

### T062：重做侧栏导航与折叠菜单

**文件：** `src/layouts/components/Sidebar/index.vue`、`src/layouts/components/Logo/index.vue`、`src/pinia/stores/layout.ts`、`src/composables/use-menu.ts`

**依赖：** T009、T043、T050、T061

**步骤：**

1. 展开/折叠/移动共用 navigation，折叠直接链接或菜单可进入所有子页面。
2. 成功导航才收起移动抽屉；活动路由变化更新分组，不只 mount 初始化。
3. lucide 图标具名导入、名称 Tooltip 与键盘焦点清晰。

**验证：** 通过 T120：折叠、移动、历史跳转均能访问核心页面，取消导航不关闭抽屉。

### T063：重做顶栏、用户菜单与面包屑

**文件：** `src/layouts/components/NavigationBar/index.vue`、`src/layouts/components/Breadcrumb/index.vue`

**依赖：** T009、T043、T053、T056、T061

**步骤：**

1. 顶栏展示移动导航开关、当前上下文、主题和用户菜单。
2. 菜单 trigger/actions 分离，消除嵌套 button；面包屑使用当前成功路由和真实链接。

**验证：** 运行 check；用 T120/T124 验证键盘打开个人中心/退出、窄屏布局和长名称。

### T064：重做标签栏与关闭操作

**文件：** `src/layouts/components/TagsView/index.vue`

**依赖：** T050、T061

**步骤：**

1. 标签用页面链接 aria-current，关闭按钮为兄弟节点；保留 fullPath href 和原生新标签打开。
2. 单行横滚、激活/焦点项滚入视口，菜单提供关闭当前/其他/右侧。
3. 全部关闭调用 coordinator，不在视图先删 tabs；固定首页不出现可关闭动作。

**验证：** 通过 T120/T122：分页切回保持地址，长标签不换行，取消关闭不丢页。

### T065：建设真实工作台首页

**文件：** `src/pages/dashboard/views/dashboard-home.vue`、`src/pages/dashboard/api/index.ts`

**依赖：** T058、T062、T063

**步骤：**

1. 展示真实当前身份、分组常用入口和能力说明，首页本身作为实际叶子页。
2. 没有聚合统计接口时不显示虚构数量或假趋势；独立读取失败不使常用导航失效。

**验证：** 通过 T120：首页唯一、入口能导航、慢/失败身份读取有反馈，无模拟数字冒充真实统计。

### T066：接入用户与角色 API 读取选项

**文件：** `src/pages/users/api/index.ts`、`src/pages/roles/api/index.ts`、`src/pages/login/api/index.ts`

**依赖：** T020、T032

**步骤：**

1. 读取 API 加可选 signal/errorPresentation，保留参数与返回类型。
2. 可修改角色按真实分页加载，kickout 保留原业务字段；写 API 支持局部错误呈现但不自动重试。

**验证：** 运行 check；T072/T074 请求断言核对 URL、分页和写 payload。

### T067：迁移用户列表查询与视觉

**文件：** `src/pages/users/views/user-list.vue`

**依赖：** T018、T027、T030、T058、T059、T066

**步骤：**

1. 列表使用 PageContext/usePageQuery/useAsyncQuery，删除 mounted+activated 双拉取。
2. 接入页头/工具栏/状态/分页和语义 table；本地过滤明确标注搜索本页。

**验证：** 通过 T072：第3页切回仍第3页，page2慢/page3快只显示page3，首次 GET/users 为1。

### T068：迁移用户角色修改

**文件：** `src/pages/users/views/user-list.vue`

**依赖：** T023、T034、T040、T067

**步骤：**

1. 可选角色独立分页/加载/失败，编辑角色维护自身 baseline 与 pending。
2. 确认成功才关闭并失效相关列表，失败保留目标与值，取消经过 dirty 检查。

**验证：** 用 T072 模拟角色加载失败与修改失败：用户列表仍显示，编辑上下文保留，重试只提交一次。

### T069：迁移删除用户与踢下线

**文件：** `src/pages/users/views/user-list.vue`

**依赖：** T041、T067

**步骤：**

1. 危险确认展示用户名/ID及真实影响，目标按对象保存。
2. 慢写阻止重复操作；成功才刷新或反馈，失败保留确认并可重试。

**验证：** 用 T072：每次确认写请求为1，失败用户仍显示，成功结果与业务反馈一致。

### T070：迁移创建用户初始化与校验

**文件：** `src/pages/users/views/user-create.vue`

**依赖：** T034、T038、T058、T066

**步骤：**

1. 使用分页角色选项与一致表单布局，默认值加载完成后建立 baseline。
2. 字段关联 label/error，设置合适 autocomplete，失败定位首个字段；角色加载失败可独立重试。

**验证：** 运行 check；T072 验证刚打开不误报 dirty，输入后取消导航保留用户名和角色。

### T071：迁移创建用户提交与离开

**文件：** `src/pages/users/views/user-create.vue`

**依赖：** T023、T049、T070

**步骤：**

1. pending 保护提交；失败保留输入，成功更新 baseline 并使用户列表 stale。
2. 成功按合法列表目标导航，普通成功提示用 toast；密码只在内存，成功/明确放弃时清理。

**验证：** 用 T072/T122：慢提交不双写，失败可继续编辑，切换保留草稿，关闭/刷新保护正确。

### T072：验证用户样板全部关键操作

**文件：** `tests/e2e/users.spec.ts`

**依赖：** T003、T064、T068、T069、T071

**步骤：**

1. 覆盖分页/历史/切回/乱序/首访、角色修改、删除、kickout 和创建。
2. 覆盖角色分页多于100、字段失败、写失败、pending 和草稿；断言接口次数及真实显示。

**验证：** E users：B01/B02/B07 与用户主要业务通过；所有模拟写仅在 fixture 中执行。

### T073：迁移角色列表与编辑

**文件：** `src/pages/roles/views/role-list.vue`

**依赖：** T027、T030、T034、T040、T058、T059、T066

**步骤：**

1. 列表统一查询/页头/状态/分页，创建/重命名分别保存 baseline 与 pending。
2. 失败不关闭，成功刷新实际结果；搜索标明范围，取消表单保护输入。

**验证：** 通过 T074：角色翻页恢复、创建/重命名失败保留值，成功仅发一次并反映结果。

### T074：验证角色列表与表单

**文件：** `tests/e2e/roles.spec.ts`

**依赖：** T003、T073

**步骤：**

1. 覆盖地址直达/历史、加载错误重试、创建/重命名慢请求、失败及取消 dirty。
2. 与用户页切换验证上下文不串页。

**验证：** E roles：分页及编辑反馈准确，用户/角色缓存互不污染。

### T075：分页化 API 权限 adapter

**文件：** `src/pages/authority/api/index.ts`、`src/pages/authority/api/type.ts`

**依赖：** T020

**步骤：**

1. 给列表与按角色查询增加页码/页大小/signal/local 错误选项，移除内部 MAX_INT=8000 固定截断。
2. 保留真实新增与批量增删契约，不凭空增加不存在的编辑/删除接口。

**验证：** 运行 check；T080 验证多页 role_authority 请求与原批量 payload。

### T076：分页化资源权限 adapter

**文件：** `src/pages/permissions/api/index.ts`、`src/pages/permissions/api/type.ts`

**依赖：** T020

**步骤：**

1. 读取可带 signal/local；保留 application_id 过滤和 CRUD payload。
2. 按角色授权默认请求规模归一，完整基线由分页调度取得，不依赖1000条截断。

**验证：** 运行 check；T080/T087 核对 application_id、页参数、CRUD 与批量增删字段。

### T077：迁移 API 权限列表与新增

**文件：** `src/pages/authority/views/authority-list.vue`

**依赖：** T027、T030、T034、T040、T058、T059、T075

**步骤：**

1. 标题明确 API 权限，URI/action 信息与本页搜索清楚；读取、分页统一。
2. 新增由业务控制关闭，删除确认立即重开补丁；校验失败/提交失败保留内容。

**验证：** T087：读取空/错/重试正确，新增失败仍在弹窗，成功只刷新一次。

### T078：迁移资源权限列表与 CRUD

**文件：** `src/pages/permissions/views/permission-list.vue`

**依赖：** T027、T030、T032、T034、T041、T058、T059、T076

**步骤：**

1. application 筛选由 URL 驱动，变更回第一页，应用选项独立加载。
2. 创建/编辑/删除采用各自受控状态和对象说明，pending/dirty/失败遵循统一契约。

**验证：** T087：筛选 URL 与请求一致，编辑失败保留值，删除失败不清对象。

### T079：实现 API 授权完整基线读取

**文件：** `src/pages/authority/composables/use-authority-assignment.ts`

**依赖：** T032、T075

**步骤：**

1. 按角色分页读取全部已授权 API 权限，明确进度、读取失败及取消。
2. 完整前禁用提交，维护 serverIds/desiredIds，候选继续加载不覆盖已选集合。

**验证：** 通过 T080：8000以上授权仍逐页读全，中间页失败不把缺失项当作未授权。

### T080：验证两类授权基线

**文件：** `tests/unit/assignment-baseline.test.ts`

**依赖：** T079、T082

**步骤：**

1. 对 API 和资源授权使用多页、重复 ID、中间失败、切角色迟到响应。
2. 断言完成前不可提交、完整 serverIds、候选未加载不影响 desiredIds。

**验证：** U assignment-baseline：两种分配均无静默遗漏或错误删除基线。

### T081：实现 API 授权差异提交

**文件：** `src/pages/authority/composables/use-authority-assignment.ts`

**依赖：** T023、T079

**步骤：**

1. 新增/移除分别记录已完成结果，部分失败重新读取完整服务端基线。
2. 保留 desiredIds；重试只发剩余差异，再读取失败则禁用盲目提交并提示重新获取。

**验证：** 通过 T083：新增成功/删除失败不重发已完成新增，不自动回滚权限。

### T082：实现资源授权完整基线读取

**文件：** `src/pages/permissions/composables/use-permission-assignment.ts`

**依赖：** T032、T076

**步骤：**

1. 按 roleId 读取完整资源权限基线，和分页候选/应用筛选分开。
2. 维护 serverIds/desiredIds；失败保留状态，角色切换取消旧读取。

**验证：** 通过 T080：1000以上授权完整读取，应用筛选不隐式取消未显示的已选项。

### T083：验证部分成功与差异重试

**文件：** `tests/unit/assignment-mutation.test.ts`

**依赖：** T081、T084

**步骤：**

1. 模拟新增成功移除失败、响应超时、重新读取失败及重新读取成功。
2. 断言 desiredIds 保留、真实 serverIds 更新、重试只处理剩余差异和 pending 单次提交。

**验证：** U assignment-mutation：两类授权均不误报全成功，也不使用不完整基线重发整批写入。

### T084：实现资源授权差异提交

**文件：** `src/pages/permissions/composables/use-permission-assignment.ts`

**依赖：** T023、T082

**步骤：**

1. 按资源授权契约提交差异，独立记录新增/删除结果。
2. 部分失败重新读取完整基线；未知结果先核对，不按旧集合重复整批提交。

**验证：** 通过 T083：部分成功有准确状态，失败复读不会丢用户期望集合。

### T085：迁移 API 权限分配页面

**文件：** `src/pages/authority/views/authority-assign.vue`

**依赖：** T003、T034、T058、T077、T078、T080、T083

**步骤：**

1. roleId来自query，API候选/已选/新增移除计数有独立区块和加载范围说明。
2. 请求交给useAuthorityAssignment，选择注册草稿；完整基线前禁用提交，错误及部分结果明确。

**验证：** 权限浏览器用例API分配：角色与计数正确，无未显示项误删，继续加载可用。

### T086：迁移资源权限分配页面

**文件：** `src/pages/permissions/views/permission-assign.vue`

**依赖：** T085

**步骤：**

1. 资源候选与application筛选明确，roleId/query、已选及差异计数同步。
2. 请求交给usePermissionAssignment，筛选不隐式取消未显示已选；草稿/pending/失败遵循plan。

**验证：** 权限浏览器用例资源分配：应用筛选不破坏完整已选集合，失败可核对再重试。

### T087：回归两类权限业务

**文件：** `tests/e2e/permissions.spec.ts`

**依赖：** T086

**步骤：**

1. 覆盖API权限新增、资源权限CRUD、角色切换、超过上限候选和中间页失败。
2. 覆盖差异新增成功/删除失败后重试，断言真实payload、接口次数与可见状态。

**验证：** E permissions：两类权限业务与分页/部分失败通过，没有静默遗漏。

### T088：接入应用与 OAuth API 读取配置

**文件：** `src/pages/applications/api/index.ts`、`src/pages/oauth/api/index.ts`

**依赖：** T020

**步骤：**

1. 分页应用、客户端详情、scope 列表支持 signal/local，写契约保留。
2. getClient/listClients 用于已完成步骤及未知结果核对，未找到不当作已确认创建失败。

**验证：** 运行 check；T091/T097 断言 URL、原 payload 和查询/写请求次序。

### T089：迁移应用列表独立数据区域

**文件：** `src/pages/applications/views/application-list.vue`

**依赖：** T027、T030、T058、T059、T088

**步骤：**

1. 应用列表先加载，客户端信息/作用域独立读取，单个详情失败只影响关联区域。
2. 统一查询/分页/状态/table，显示 ID、类型、授权信息与可读行操作。

**验证：** T097：客户端详情慢/失败时应用列表仍可用，分页切回/错误重试正确。

### T090：实现应用注册分步状态

**文件：** `src/pages/applications/composables/use-application-registration.ts`

**依赖：** T023、T088

**步骤：**

1. 实现 idle/creating-client/linking-application/revealing-result/done 及错误归属。
2. 客户端成功保留 ID/一次性密钥，建立应用失败只重试关联；未知超时先核对或显示结果未知。

**验证：** 通过 T091：第二步失败后不会再次 POST 客户端，secret 只保存在内存。

### T091：验证应用创建部分成功

**文件：** `tests/unit/application-registration.test.ts`

**依赖：** T090

**步骤：**

1. 模拟第一步失败、第二步失败、慢重复确认、未知响应与显式重试。
2. 断言各步骤调用次数、已成功客户端不重复创建、无法确认时不自动非幂等重试。

**验证：** U application-registration：准确保留进度与一次性结果，没有误报全部成功或重复客户端。

### T092：实现应用更新与删除剩余步骤

**文件：** `src/pages/applications/composables/use-application-registration.ts`

**依赖：** T090

**步骤：**

1. 更新记录客户端/应用的已成功步骤，删除按原客户端→应用顺序记录。
2. 失败只允许核对后重试未完成步骤；404按具体对象判定，不统一吞掉错误。

**验证：** 通过 T093：客户端已删除而应用失败不再重复删客户端，更新不重发已成功步骤。

### T093：验证应用更新删除部分失败

**文件：** `tests/unit/application-registration.test.ts`

**依赖：** T092

**步骤：**

1. 补更新第二步失败、删除第二步失败、未知结果与404语义用例。
2. 确认只重试剩余步骤，并把服务端未确认状态显示为未知/待核对。

**验证：** U application-registration：更新/删除状态与调用次数符合分步事实。

### T094：拆出应用编辑表单

**文件：** `src/pages/applications/components/application-form.vue`

**依赖：** T034、T040、T041、T089、T093

**步骤：**

1. 表单负责字段、label/校验、baseline/dirty与必要选项。
2. 提交交给分步workflow，busy/error由props驱动，关闭请求允许脏表单检查。

**验证：** 运行check；应用浏览器用例字段/失败场景保留输入并定位首个错误。

### T095：提供一次性密钥结果组件

**文件：** `src/pages/applications/components/secret-result.vue`

**依赖：** T094

**步骤：**

1. 结果显示确认已保存，复制成功/失败有反馈，密钥只在当前内存。
2. 完成发单独事件，不触发创建；刷新前说明结果无法自动恢复。

**验证：** 应用浏览器用例：完成不会再创建，复制失败可见，URL/storage/toast无密钥。

### T096：接入应用表单、密钥与危险操作

**文件：** `src/pages/applications/views/application-list.vue`

**依赖：** T095

**步骤：**

1. 接入表单/workflow/结果，创建/更新/删除按已完成步骤展示状态。
2. 密钥重生成独立pending与一次性结果，失败可明确重试；关闭保护dirty，删除显示真实影响。

**验证：** 运行check；E applications核对全流程及部分失败重试次数。

### T097：验证应用全流程与敏感结果

**文件：** `tests/e2e/applications.spec.ts`

**依赖：** T003、T096

**步骤：**

1. 覆盖列表、创建、编辑、删除、scope、密钥重生成、一次性结果和部分失败。
2. 断言 storage/URL/toast 不含密钥，第二步重试只一条剩余写请求。

**验证：** E applications：所有现有可达能力保留，多步骤失败反馈真实，secret 不被持久化。

### T098：迁移已授权应用与撤销

**文件：** `src/pages/authorized-apps/api/index.ts`、`src/pages/authorized-apps/views/authorized-apps-list.vue`

**依赖：** T020、T023、T027、T041、T058

**步骤：**

1. 授权列表统一首次/刷新/错误/空状态，读取接受 signal/local。
2. 撤销显示目标与影响，pending 禁止重复/关闭，成功才清对象并刷新。

**验证：** T099：1秒失败后确认仍在且应用仍显示，成功后列表反映实际撤销。

### T099：验证异步撤销基线缺陷

**文件：** `tests/e2e/authorized-apps.spec.ts`

**依赖：** T003、T098

**步骤：**

1. 复现 B04 的1秒失败场景，断言期间对话框和busy、失败后的重试上下文。
2. 补成功、重复确认和列表刷新失败。

**验证：** E authorized-apps：没有250ms提前关闭，重复写为1，失败不误报撤销成功。

### T100：迁移记录 API 与并行首屏

**文件：** `src/pages/recordings/api/index.ts`、`src/pages/recordings/views/recordings-list.vue`

**依赖：** T020、T027、T030、T032、T058、T059、T066

**步骤：**

1. 记录 query 立即发起，不等待全部用户；选项独立分页继续加载与失败重试。
2. user_id URL 筛选变化回第1页，清理 watch加手动双加载；搜索仅使用接口真实能力。

**验证：** T101：拖慢用户选项时记录首屏已显示，筛选仅一次记录请求，分页切回正确。

### T101：验证记录加载与筛选

**文件：** `tests/e2e/recordings.spec.ts`

**依赖：** T003、T100

**步骤：**

1. 覆盖慢用户选项/失败、全部记录与用户筛选、历史导航和更多用户加载。
2. 断言首屏记录开始时间与请求次数，用户选项失败不隐藏记录。

**验证：** E recordings：记录不串行等待全量用户，改变筛选从第一页加载一次。

### T102：迁移个人中心读取与昵称表单

**文件：** `src/pages/profile/api/index.ts`、`src/pages/profile/index.vue`

**依赖：** T020、T023、T027、T034、T058

**步骤：**

1. 当前身份使用真实请求与统一状态，昵称表单有独立 baseline/pending/error。
2. 保存成功更新当前用户显示，失败保留昵称；与密码表单不共享提交锁或dirty基线。

**验证：** T104：昵称失败不丢输入，成功顶栏同步，不影响尚未提交的密码字段。

### T103：迁移密码修改与成功退出

**文件：** `src/pages/profile/index.vue`

**依赖：** T053、T102

**步骤：**

1. 密码表单独立校验/pending，正确 autocomplete 和错误定位。
2. 成功清敏感字段并沿原业务约定统一退出；失败保留必要输入但不写持久化。

**验证：** T104：失败仍可编辑，成功无旧账号标签/密码，修改中普通导航被保护。

### T104：验证个人中心独立表单

**文件：** `tests/e2e/profile.spec.ts`

**依赖：** T003、T103

**步骤：**

1. 分别模拟昵称和密码的慢/失败/成功，检查两份 dirty/pending 互不覆盖。
2. 保存昵称同步用户展示，密码成功触发统一退出且存储无密码。

**验证：** E profile：独立反馈与草稿保护正确，敏感结果未残留。

### T105：迁移用户覆写 API 与 API 权限面板

**文件：** `src/pages/users/api/index.ts`、`src/pages/users/components/authority-overrides.vue`

**依赖：** T020、T023、T027、T032、T034、T075

**步骤：**

1. 覆写读取保留真实接口参数并明确显示所属用户，候选按页继续加载。
2. API 覆写面板管理自身读取、选择与增/改/删状态，切目标取消旧请求，pending阻止误切。

**验证：** T108：API覆写慢/失败/多页候选及用户切换无串数据，写payload保留原契约。

### T106：迁移资源权限覆写面板

**文件：** `src/pages/users/components/permission-overrides.vue`

**依赖：** T020、T023、T027、T032、T034、T076、T105

**步骤：**

1. 资源覆写与API覆写明确区分，application筛选/选项和覆写结果独立。
2. 新增/修改/删除反馈局部，失败保留选择，分页不静默截断，未显示项不误删。

**验证：** T108：资源覆写错误与结果只影响对应内容页，选择和用户对象准确。

### T107：接入用户覆写弹窗

**文件：** `src/pages/users/components/user-overrides-dialog.vue`、`src/pages/users/views/user-list.vue`

**依赖：** T040、T068、T105、T106

**步骤：**

1. 列表仅提供目标/打开操作，弹窗协调API与资源两个面板。
2. 标题含用户对象，busy/dirty汇总用于关闭保护；切内容页保留所属面板状态。

**验证：** 运行 check；T108验证切面板/取消/关闭/重开，新用户不会继承旧覆写草稿。

### T108：验证用户权限覆写

**文件：** `tests/e2e/user-overrides.spec.ts`

**依赖：** T003、T107

**步骤：**

1. 覆盖两类覆写增/改/删、候选分页、失败后重试及目标快速切换。
2. 断言取消保留草稿、关闭明确放弃、旧请求无跨用户回写。

**验证：** E user-overrides：所有已有覆写能力可用且用户归属清晰，无候选截断。

### T109：建立统一认证视觉布局

**文件：** `src/layouts/auth-layout.vue`

**依赖：** T034、T037、T038、T048、T052、T055

**步骤：**

1. 以已批准方向建立认证内容区、品牌/说明、表单与错误槽。
2. 满足窄屏、暗色、焦点和减少动画；认证入口保持独立路由。

**验证：** 运行check；入口截图在五种视口可用，字段区不被装饰挤压。

### T110：迁移普通登录和目标恢复

**文件：** `src/pages/login/index.vue`

**依赖：** T109

**步骤：**

1. 使用认证布局、标准字段标签/autocomplete与独立pending/error。
2. 登录成功确认owner后恢复经过验证的本地workspace目标，拒绝外部/无效返回地址；密码不持久化。

**验证：** E auth普通登录：合法目标恢复，失败不双提交，非法目标安全回首页。

### T111：迁移注册表单与结果反馈

**文件：** `src/pages/login/register.vue`

**依赖：** T110

**步骤：**

1. 注册字段校验和pending/失败反馈统一，密码只在内存。
2. 成功提供登录入口与toast，移除阻塞成功modal；表单失败保留可编辑状态。

**验证：** E auth注册：单次提交、错误可定位，成功进入登录且存储无密码。

### T112：迁移 OIDC 登录保持协议参数

**文件：** `src/pages/oidc-login/index.vue`、`src/pages/login/api/index.ts`

**依赖：** T111

**步骤：**

1. 使用认证布局，保留所有原授权参数及原协议返回路径，不纳入工作台快照。
2. 登录pending防双提交，失败定位与重试准确；会话失效仍返回原OIDC流。

**验证：** T114：client_id/state/redirect_uri/PKCE等原参数未丢失或被工作台query白名单过滤。

### T113：迁移 OIDC 授权同意与拒绝

**文件：** `src/pages/oidc-consent/index.vue`、`src/pages/oidc-consent/api/index.ts`

**依赖：** T020、T023、T027、T112

**步骤：**

1. 授权加载/错误明确显示应用、作用域与当前身份，保留同意/拒绝原协议。
2. pending阻止重复决定，失败保留请求上下文，跳转使用原约定并不进入标签存储。

**验证：** T114：同意/拒绝各只发一次，state/redirect_uri准确回传，失败不显示已授权。

### T114：验证普通与 OIDC 入口

**文件：** `tests/e2e/auth.spec.ts`

**依赖：** T003、T113

**步骤：**

1. 模拟登录注册、失效刷新、合法/非法返回目标、OIDC登录和授权同意/拒绝。
2. 核对原参数和跳转目标，断言凭据/授权query不进入workspace持久化。

**验证：** E auth：前端入口协议回归通过；真实服务端认证仍需T133独立联调。

### T115：统一错误页与可用返回入口

**文件：** `src/pages/errors/403.vue`、`src/pages/errors/404.vue`、`src/pages/redirect/index.vue`

**依赖：** T055、T111

**步骤：**

1. 错误页明确区分拒绝访问与不存在，统一标题/说明/可操作返回。
2. 无可用站内历史时回固定首页；redirect只按既有安全目标约定导航，避免循环。

**验证：** 通过 T008/T114：403不退出，404可返回，无历史直接打开也有可用下一步。

### T116：迁移旧 OAuth 组件的确认调用

**文件：** `src/pages/oauth/views/oauth-list.vue`、`src/pages/oauth/views/oauth-detail.vue`、`src/pages/oauth/views/oauth-create.vue`

**依赖：** T023、T034、T040、T041、T088

**步骤：**

1. 逐一核对保留源码的旧Dialog调用，全部改成业务控制关闭与busy/error。
2. 旧密钥/创建结果使用一致一次性展示；保留兼容地址重定向，不扩大旧页面可达范围。

**验证：** 运行 check；调用方清单无依赖自动关闭的残留，原兼容地址仍落在统一应用页。

### T117：清理无引用包装与旧导航

**文件：** `src/router/helper.ts`、`src/router/config.ts`、`src/pages/dashboard/index.vue`、`src/pages/dashboard/components/dashboard-introduction.vue`、`src/pages/users/index.vue`、`src/pages/roles/index.vue`、`src/pages/authority/index.vue`、`src/pages/permissions/index.vue`、`src/pages/applications/index.vue`、`src/pages/authorized-apps/index.vue`、`src/pages/recordings/index.vue`、`src/pages/oauth/index.vue`、`src/layouts/components/NavigationBar/item.vue`、`src/layouts/components/NavigationBar/link.vue`、`src/style.css`

**依赖：** T087、T097、T099、T101、T104、T108、T114、T115、T116

**步骤：**

1. rg引用检索后删除确实无调用的模块包装、router helper及旧NavigationBar子组件。
2. 保留实际仍被调用的文件并记录理由；旧path/name兼容重定向继续保留。

**验证：** 运行check与路由回归；无丢失导入，旧入口解析保持正确。

### T118：清理重复缓存及旧视觉补丁

**文件：** `src/style.css`、`src/layouts/mode/left-top-mode.vue`、`src/components/ui/dialog/dialog.vue`

**依赖：** T117

**步骤：**

1. 检查所有已迁移页，无重复KeepAlive/min-h-screen、mounted+activated双查询。
2. 移除灰色全局覆盖、Dialog自动关闭重开残留，保留语义token。

**验证：** rg核对残留及运行check/B；真实workbench使用单一缓存，主题截图无旧样式覆盖。

### T119：回归核心导航基线缺陷

**文件：** `tests/e2e/workspace.spec.ts`

**依赖：** T003、T064、T065、T072、T074

**步骤：**

1. 覆盖B03/B05/B06/B09：折叠子菜单、小屏正文、唯一首页及dialog语义。
2. 直达/历史改变正确活动分组，移动抽屉成功才关闭，长标题标签单行横滚。

**验证：** E workspace导航用例：核心入口可访问，正文不挤压，首页唯一。

### T120：回归标签操作及首访请求

**文件：** `tests/e2e/workspace.spec.ts`

**依赖：** T119

**步骤：**

1. 覆盖B01/B07：第3页切回保留地址、query不新建标签、首访单GET。
2. 覆盖关闭当前/其他/右侧、固定首页和导航被拒绝时的可见标签/内容。

**验证：** E workspace标签用例：成功后才关闭，分页/筛选恢复，初始请求为1。

### T121：回归草稿导航与缓存销毁

**文件：** `tests/e2e/workspace-state.spec.ts`

**依赖：** T051、T054、T072、T108、T120

**步骤：**

1. 覆盖脏表单tab/侧栏/后退/刷新、取消批量关闭、失活保留及关闭重开清理。
2. 验证beforeunload原生保护和慢确认期间新导航，取消不丢输入。

**验证：** E workspace-state草稿用例：所有离开入口语义一致，刷新不声称恢复敏感输入。

### T122：回归会话切换及存储故障

**文件：** `tests/e2e/workspace-state.spec.ts`

**依赖：** T121

**步骤：**

1. 慢请求遇换账号、旧确认遇会话失效、损坏快照和403拒绝访问。
2. 记录可见结果/请求次数；旧结果不写入新页面，未知owner不恢复快照。

**验证：** E workspace-state会话用例：B08通过，强制失效不被旧草稿阻塞。

### T123：生成多视口与主题实现截图

**文件：** `tests/e2e/visual.spec.ts`、`tests/e2e/fixtures/workspace.ts`、`../../../docs/specs/yufu-frontend-redesign/evidence/README.md`

**依赖：** T045、T057、T087、T097、T099、T101、T104、T108、T114、T115、T120

**步骤：**

1. 建立五种spec视口与浅/暗/系统主题截图矩阵，包含长标题、加载/空/错状态。
2. 使用明确模拟fixture，记录浏览器版本/视口/数据性质和可重跑命令。

**验证：** E visual：正文可用，表格局部滚动，标签单行；保存实现截图而非仅方向稿。

### T124：核验对比度、焦点与触控

**文件：** `tests/e2e/visual.spec.ts`、`tests/e2e/fixtures/workspace.ts`、`../../../docs/specs/yufu-frontend-redesign/evidence/README.md`

**依赖：** T123

**步骤：**

1. 检查截图中实际前景/背景组合，普通文本对比度目标4.5:1。
2. 验证焦点可见/无遮挡、24px及主要触控44px、reduced-motion和主题系统切换。

**验证：** E visual可访问性用例及实际观察：记录对比度/几何/焦点证据，失败项修复后重跑。

### T125：完成类型和 lint 最终检查

**文件：** `package.json`、`eslint.config.js`、`../../../docs/specs/yufu-frontend-redesign/evidence/README.md`

**依赖：** T004、T118、T122、T124

**步骤：**

1. 运行check/lint，修复实际失败及残留诊断。
2. 保持有效规则，记录命令、退出码和环境；不以全局disable处理新问题。

**验证：** check与lint均退出0；结果为实际执行，未运行不得标通过。

### T126：完成单测、构建及共享影响检查

**文件：** `package.json`、`eslint.config.js`、`../../../docs/specs/yufu-frontend-redesign/evidence/README.md`

**依赖：** T125

**步骤：**

1. 运行全部单测和临时目录生产构建，修复失败后记录通过。
2. 测量实际bundle；核对共享包/其他应用是否受影响，若修改共享代码则运行真实调用方构建。

**验证：** test与B退出0，新增依赖与公共影响有记录；产物不覆盖现有assets。

### T127：完成 Chromium 浏览器回归

**文件：** `playwright.config.ts`、`../../../docs/specs/yufu-frontend-redesign/evidence/README.md`

**依赖：** T126

**步骤：**

1. 执行chromium项目全部行为及关键视觉用例。
2. 修复失败并记录引擎版本、测试数和重跑结果。

**验证：** pnpm --filter @yufu/web exec playwright test --project=chromium：全部通过，无跳过核心用例。

### T128：完成 Firefox 浏览器回归

**文件：** `playwright.config.ts`、`../../../docs/specs/yufu-frontend-redesign/evidence/README.md`

**依赖：** T127

**步骤：**

1. 执行firefox项目相同业务与关键视觉用例。
2. 修复引擎差异并记录实际版本/失败/重跑。

**验证：** pnpm --filter @yufu/web exec playwright test --project=firefox：全部通过。

### T129：完成 WebKit 浏览器回归

**文件：** `playwright.config.ts`、`../../../docs/specs/yufu-frontend-redesign/evidence/README.md`

**依赖：** T128

**步骤：**

1. 执行webkit项目相同业务与关键视觉用例。
2. 记录版本和差异，表述为WebKit引擎测试，真实Safari另验。

**验证：** pnpm --filter @yufu/web exec playwright test --project=webkit：全部通过。

### T130：真实后端登录、刷新与权限联调

**文件：** `../../../docs/specs/yufu-frontend-redesign/evidence/README.md`

**依赖：** T129

**步骤：**

1. 使用明确授权的测试后端及测试账号验证登录、刷新、退出/换账号和权限业务。
2. 记录真实请求与页面结果；缺后端/账号时记所需条件，继续其他可独立检查，不用模拟冒充。

**验证：** 真实接口的登录/刷新/权限修改结果与页面一致，操作仅针对授权测试数据，证据记录环境。

### T131：真实 OIDC 同意与拒绝联调

**文件：** `../../../docs/specs/yufu-frontend-redesign/evidence/README.md`

**依赖：** T130

**步骤：**

1. 在同一授权测试环境验证OIDC登录、同意/拒绝及原协议回跳。
2. 记录state/PKCE/redirect_uri等协议是否按原约定保存，证据脱敏且不记录密钥。

**验证：** 真实OIDC路径通过；无真实客户端环境则明确标未验证和缺失条件。

### T132：检查真实桌面与移动设备

**文件：** `../../../docs/specs/yufu-frontend-redesign/evidence/README.md`

**依赖：** T131

**步骤：**

1. 记录实际Chrome/Edge/Firefox/Safari和iOS Safari/Android Chrome可用设备版本。
2. 在真实设备检查核心导航/表单/焦点/滚动/主题；缺少设备的条目明确未验证。

**验证：** 设备与版本/结果有记录；WebKit自动化不代替真实Safari/手机结论。

### T133：编写逐项验收报告

**文件：** `../../../docs/specs/yufu-frontend-redesign/acceptance-report.md`

**依赖：** T132

**步骤：**

1. 按已批准checklist逐条记录实际结果，关联AC1–AC13、命令/截图/真实联调证据。
2. 未通过项列实际差异与所需条件，满足全部需求后才宣布改版完成。

**验证：** 报告所有条目有实际结果或明确未验证说明；没有用设计/模拟结论替代真实完成状态。

## 合法执行顺序

以下为按依赖计算的一条合法顺序；同一文件的修改串行执行。用户未授权启动子代理，本单中的独立任务不等同于子代理授权。

```text
T001 → T002 → T003 → T004 → T005 → T006 → T007 → T008 → T009 → T010 → T011 → T012
T013 → T014 → T015 → T016 → T017 → T018 → T019 → T020 → T021 → T022 → T023 → T024
T025 → T026 → T027 → T028 → T029 → T030 → T031 → T032 → T033 → T034 → T037 → T038
T039 → T040 → T041 → T042 → T043 → T044 → T046 → T035 → T036 → T047 → T048 → T049
T050 → T051 → T052 → T053 → T054 → T055 → T056 → T057 → T058 → T059 → T060 → T061
T062 → T063 → T064 → T065 → T066 → T067 → T068 → T069 → T045 → T070 → T071 → T072
T073 → T074 → T075 → T076 → T077 → T078 → T079 → T081 → T082 → T080 → T084 → T083
T085 → T086 → T087 → T088 → T089 → T090 → T091 → T092 → T093 → T094 → T095 → T096
T097 → T098 → T099 → T100 → T101 → T102 → T103 → T104 → T105 → T106 → T107 → T108
T109 → T110 → T111 → T112 → T113 → T114 → T115 → T116 → T117 → T118 → T119 → T120
T121 → T122 → T123 → T124 → T125 → T126 → T127 → T128 → T129 → T130 → T131 → T132
T133
```

任务自身依赖是最终依据；上述换行仅为排版。可以先推进独立的权限adapter与用户覆写迁移，再完成样板全能力回归，不因目录分组造成等待。真实后端或设备不可用时继续完成独立前端检查，在验收报告里如实列出尚未验证项。

## 需求与基线覆盖

| 需求 | 任务归属 |
|---|---|
| F1/F2 | T007、T008、T009、T010、T011、T016、T017、T018、T050、T051、T062、T063、T064、T065、T119、T120 |
| F3 | T012、T013、T014、T019、T034、T035、T036、T049、T052、T053、T054、T121、T122 |
| F4/F5 | T020、T021、T022、T025、T026、T027、T028、T029、T030、T031、T032、T033、T059、T060、T067、T072、T100、T101 |
| F6/F7 | T023、T024、T037、T038、T039、T040、T041、T042、T043、T044、T045、T046、T047、T048、T049、T050、T051 |
| F8 | T066、T067、T068、T069、T070、T071、T072、T073、T074、T105、T106、T107、T108 |
| F9 | T075、T076、T077、T078、T079、T080、T081、T082、T083、T084、T085、T086、T087 |
| F10 | T088、T089、T090、T091、T092、T093、T094、T095、T096、T097、T098、T099、T100、T101、T102、T103、T104 |
| F11 | T109、T110、T111、T112、T113、T114、T115 |
| F12 | T055、T056、T057、T058、T059、T060、T061、T062、T063、T064、T065、T123、T124 |
| N1–N6/AC13 | T001、T002、T003、T004、T018、T022、T028、T036、T044、T045、T051、T054、T117、T118、T119、T120、T121、T122、T123、T124、T125、T126、T127、T128、T129、T130、T131、T132、T133 |

| 已复现缺陷 | 实施/回归任务 |
|---|---|
| B01 分页地址丢失 | T005、T010、T029、T030、T064、T067、T072、T119、T120 |
| B02 迟到响应覆盖 | T020、T025、T026、T027、T028、T067、T072 |
| B03 折叠菜单不可达 | T009、T042、T043、T062、T119、T120 |
| B04 撤销确认过早关闭 | T040、T041、T098、T099 |
| B05 小屏正文被挤压 | T055、T061、T062、T064、T123、T124 |
| B06 两个固定首页 | T007、T008、T010、T065、T119、T120 |
| B07 首访重复请求 | T025、T026、T027、T028、T067、T072、T119、T120 |
| B08 会话残留标签 | T019、T052、T053、T054、T121、T122 |
| B09 对话框无语义 | T040、T041、T044、T045、T119、T120 |

本表仅说明任务覆盖，不表示缺陷已修复。T133 的验收报告按已批准checklist记录实际通过/未通过；后台接口未提供的全量搜索、事务及聚合统计不伪装成已实现。

## 文档自检与审阅

已核对任务依赖无环、每项有文件/步骤/验证，plan各组件与F1–F12均有归属。模板占位符不进入任务正文。执行过程中若发现KeepAlive公开机制或真实接口与设计不兼容，记录具体证据，修订对应设计与任务后再推进相关实现。

用户已确认任务范围、先后依赖及验证方式，验收清单已写入 [checklist.md](checklist.md)。当前实现与工程回归已完成；进度和验收报告保留真实环境及部分验证范围。

## 实施记录（2026-11-08）

实际单测聚合为workspace、http、forms-and-workflows、cache-and-close、confirm-and-options共5个文件；E2E聚合为auth、business、edge-cases、robustness、visual、workspace、workspace-state共7个文件。原任务中的专用测试名映射到 [逐例索引](evidence/after/test-index.md)，不保留空测试文件。

两类权限列表/分配复用permission-catalog和permission-assignment；应用流程采用createStepWorkflow + createVerifiedCreate组合，另有Editor/SecretResult组件。usePagedOptions候选每批100，用户可修改角色按完整分页读取；原生checkbox组件契约保持兼容，通过业务交互和其他控件单测验证。macOS14 WebKit使用匹配驱动与公开上下文API，具体见验收报告。
