# cobotusr 前端排查与改版建议

审查日期：2026-11-08。基线提交：`fa571d8`。范围：`cobotweb/apps/cobotusr`，涉及公共组件时检查 `cobotweb/packages/shared`。

## 结论

现状的问题同时出在导航状态、异步数据和界面结构。建议采用“保留 Vue 与现有业务接口，重做前端工作台与交互基础”的方案。仅更换配色无法解决分页丢失、响应乱序和弹窗提前关闭；直接迁移另一套前端框架也没有证据表明能降低改版成本。

本次已完成源码检查、代码图谱调用范围核对、类型检查、生产构建、Chrome 浏览器复现和改版规划。用户已于 2026-11-08 批准 [spec.md](spec.md) 的需求与视觉方向、[plan.md](plan.md) 技术设计及 [task.md](task.md) 任务单；[checklist.md](checklist.md) 验收清单已生成，待审阅。尚未修改产品实现。

## 证据与限制

- 类型检查：`pnpm --filter @cobotweb/cobotusr exec vue-tsc -b --pretty false`，通过。
- 生产构建：`pnpm --filter @cobotweb/cobotusr exec vite build --outDir /tmp/cobotusr-baseline-dist`，通过，850 个模块，构建耗时 1.57 秒。该耗时是本机一次测量，不作为性能 SLA。
- 浏览器：Playwright 驱动本机 Google Chrome，1440×960 与 390×844 两个视口。
- API 请求全部由浏览器拦截并返回模拟数据；认证使用临时浏览器上下文中的虚构 token。没有调用真实后端，没有登录真实账号，没有执行真实撤销或删除。
- 浏览器记录没有捕获到未处理的 `pageerror`；交互缺陷仍能稳定出现。
- 图谱确认 `useTabsStore` 的直接调用方是 TagsView，实际布局没有使用现有 AppMain；标签生命周期应由实际布局接入。
- 当前 4000 端口由工作区外的进程占用，不以其作为 cobotusr 后端。真实认证、刷新 token、OIDC 及权限接口仍需后续环境联调。
- `/ground-first` 在当前技能目录、命令目录和项目中未找到。同名搜索返回 `grounding-before-coding` 等候选，未核实为同一技能，因此本轮按“先证据，后方案”开展，而没有声称执行了该技能。

浏览器原始结果：[browser-baseline.json](evidence/browser-baseline.json)。复现脚本：[browser-audit.cjs](evidence/browser-audit.cjs)。

当前截图：[桌面用户列表](evidence/before-users.png)、[窄屏用户列表](evidence/before-mobile.png)、[当前首页](evidence/before-dashboard.png)。截图内用户名、应用名及数量均为模拟数据。

## 已由浏览器验证的问题

P1 表示核心操作受影响，P2 表示持续影响体验或维护。优先级是本次改版的工作排序。

| ID | 优先级 | 实际行为与复现 | 源码位置 | 修复方向 |
|---|---|---|---|---|
| B01 | P1 | 从 `/users?page_number=3&page_size=10` 切到角色列表，再点击用户 tab，URL 变为第 1 页 | `src/pinia/stores/tabs.ts:26`、`src/layouts/components/TagsView/index.vue:43` | 区分标签身份与完整跳转地址；列表查询状态与 URL 双向同步 |
| B02 | P1 | 第 2 页响应延迟 650ms、第 3 页响应延迟 30ms，最终 URL 是第 3 页，表格却显示 `audit-page-2` | `src/pages/users/views/user-list.vue:91`、`:507` | 每个查询采用取消或递增请求标识；只允许当前请求更新结果和 loading |
| B03 | P1 | 折叠侧栏后，带子菜单的父入口只改变 openKeys；子菜单始终隐藏，用户、角色、应用等页面无法由这些入口访问 | `src/layouts/components/Sidebar/index.vue:122` | 折叠态提供直接导航与可键盘操作的弹出菜单；图标保留名称提示 |
| B04 | P1 | 点击撤销确认后 250ms 弹窗已关闭；模拟请求 1000ms 后失败，应用仍在，但弹窗与重试上下文消失 | `src/components/ui/dialog/dialog.vue:23`、`:30`、`src/pages/authorized-apps/views/authorized-apps-list.vue:31` | 异步确认由业务控制关闭；请求中显示 busy，失败留在当前弹窗并允许重试 |
| B05 | P1 | 390px 视口中侧栏占 256px，正文仅 134px；tab 标题纵向换行，表格与按钮被挤压 | `src/layouts/mode/left-top-mode.vue:22`、`src/layouts/components/TagsView/index.vue:2` | 小屏侧栏改抽屉；标签不换行并可横向滚动；表格在自身容器滚动 |
| B06 | P2 | 访问 `/dashboard` 后同时出现“仪表盘首页”和“系统介绍”，两项都固定，无法关闭 | `src/pinia/stores/tabs.ts:16`、`src/router/routes.ts:78` | 首页只有一个规范入口和一个固定标签；兼容原地址的跳转 |
| B07 | P2 | 首次进入用户列表产生两次参数完全相同的 GET `/users` | `src/pages/users/views/user-list.vue:346`、`:357`、`src/pages/users/index.vue:15` | 统一首次加载与缓存激活策略，避免 mounted / activated 重复拉取 |
| B08 | P2 | 清除当前用户并重新进入工作台后，上一会话的用户、角色、应用 tab 仍在 | `src/pinia/stores/user.ts:32`、`src/pinia/stores/tabs.ts:10` | 退出与会话失效时清理标签及页面缓存；持久化数据按用户隔离 |
| B09 | P2 | 撤销确认弹窗可见时，浏览器中 `role=dialog` 数量为 0 | `src/components/ui/dialog/dialog.vue:61` | 补全弹窗语义、焦点管理、Esc 与焦点返回，统一到可靠的交互基础组件 |

## 源码确认的问题与待运行验证项

以下与浏览器已复现项分开记录，避免将推断写成运行结论。

| ID | 定位与事实 | 影响 / 待验证 | 处理方向 |
|---|---|---|---|
| S01 | `src/layouts/components/Sidebar/index.vue:136` 仅在 mounted 初始化展开项 | 点击 tab、历史后退或程序跳转后，活动分组可能仍收起 | 监听已完成的当前路由，展开所属分组 |
| S02 | 实际布局 `left-top-mode.vue:26` 直接使用 router-view；各模块包装页各自 keep-alive；tab 删除不关联缓存 | 模块间状态被销毁，模块内关闭标签后状态又可能残留 | 明确页面缓存所有者、标签身份与缓存淘汰边界 |
| S03 | `TagsView/index.vue:53` 先删 tab，再执行 router.push，未检查导航失败 | 加入未保存表单守卫后，拒绝跳转仍会丢失标签 | 导航成功后再提交标签关闭；失败保留原状态 |
| S04 | `NavigationBar/index.vue:121` button 内含另一个 UiButton（默认 button） | 嵌套交互语义、键盘焦点及点击行为不可靠 | 使用单一触发按钮；下拉统一处理键盘和关闭 |
| S05 | `user-create.vue:127` 仅取消按钮检查字段；没有统一离开守卫，且默认 role_id 使取消判断可能恒为真 | 点击侧栏/tab、后退、刷新可能丢草稿；未编辑也被询问 | 比较初始快照产生 dirty，所有离开入口共用确认行为 |
| S06 | `user-list.vue:81` 对当前分页结果做本地搜索；界面提示未说明范围 | 用户可能将“本页无结果”理解为全平台不存在 | 有后端搜索能力则查询后端；否则明确“搜索本页”，不伪装全量搜索 |
| S07 | `recordings-list.vue:35` 聚合全部用户页后才请求记录；`:84` 监听筛选但不统一重置页码；`:66` 手动与 watch 同时触发 | 大量用户时首屏缓慢；第 5 页切换用户可能呈现假空状态；清空筛选重复请求 | 记录与选项独立加载；筛选重置第 1 页；单一加载入口 |
| S08 | `permission-assign.vue:21` 限制 1000 条权限，`:33` 限制 100 个角色；其他分配页有类似固定上限 | 数据增多时可选权限或角色不完整 | 在接口能力内分页加载/远程搜索；明确截断与加载更多 |
| S09 | 多个列表 catch 后保留空数组或旧数据；权限/记录页面有独立的原生弹层 | 请求失败可能被呈现为正常空状态，提示策略不一致 | 区分 loading / empty / error / ready；局部重试与提示去重 |
| S10 | `src/components/Feedback/toast-host.vue:10` 无 live region；tab 用 div click；弹窗缺少焦点锁定与返回 | 键盘与读屏用户无法可靠获知操作结果 | 导航使用链接，动作使用按钮；提示使用 live region |
| S11 | 模块包装层使用 min-h-screen；固定页脚占用视口；暗色样式依赖灰度文字全局覆盖 | 多余滚动和留白、主题细节不一致 | 应用壳统一分配高度与滚动；以语义 token 替换局部补丁 |

## 方案比较

| 方案 | 工作内容 | 收益 | 代价 | 建议 |
|---|---|---|---|---|
| A：工作台与交互基础重构 | 保留 Vue 3 / Pinia / Router 与业务接口；统一布局、路由状态、缓存与请求；选用 Vue 适配的可靠基础组件重做视觉 | 同时覆盖 bug、可访问性和视觉；各业务页面可逐步迁移 | 需要先明确组件边界并建立行为回归 | 推荐 |
| B：原组件补丁与换肤 | 保留当前结构，逐处修 tab、弹窗、CSS | 起步快，依赖变化少 | 缓存与请求策略仍分散，长期成本高 | 适合短期止血 |
| C：整体切换大型后台组件库或框架 | 全页面适配新组件体系/框架 | 可获得较多现成管理控件 | 全量迁移与业务回归成本最高；默认外观仍需设计 | 本轮证据不足以支持 |

组件库最终选择属于技术设计阶段。候选是 shadcn-vue / Reka UI 一类可定制的 Vue 交互基础；先验证 Dialog、Menu、Tooltip 的焦点与样式适配，再决定依赖，不把“手写所有交互”作为目标。

## 改版方向

平台主要工作是用户、角色、API 权限、资源权限、应用注册及授权管理。将首页从泛化的机器人架构介绍调整为与这些任务对应的概览与入口；没有可用接口的数据不展示假统计。

建议以“授权与身份管理工作台”为方向：清晰的信息密度、克制的青绿色品牌色、中性色背景、稳定的表格和表单、可追踪的操作反馈。桌面采用分组侧栏、面包屑、单行 tab 和内容区域；小屏采用抽屉侧栏。单页尽量只设置一个醒目的主操作。删除、撤销、踢下线放在明确的次级操作区域或行菜单中，确认时展示对象与实际后果。

静态方向稿：[design-direction.svg](design-direction.svg)。这是一份带桌面和小屏布局的视觉草案，不是已实现的新界面。内容是设计示例。

建议导航分组：

- 概览：工作台。
- 身份管理：用户、角色。
- 访问控制：API 权限、资源权限；各自保留列表与分配入口，避免两处“权限列表 / 权限分配”无法辨认。
- 应用与授权：应用注册、已授权应用。
- 审计：操作记录。
- 用户菜单：个人中心、即时通讯、退出。

## 推荐执行阶段

| 阶段 | 范围 | 可审阅产物与完成条件 |
|---|---|---|
| 1：导航与状态 | 规范首页、标签完整地址、导航失败处理、分组侧栏、会话清理 | B01/B03/B06/B08 行为回归通过；草稿离开守卫生效 |
| 2：交互与布局基础 | 桌面/小屏壳、弹窗/菜单/提示、请求竞态与 loading | B02/B04/B05/B07/B09 回归通过；截图审阅 |
| 3：标杆页面 | 用户列表、创建用户、角色；抽取确有复用的页头、查询工具栏、表格状态和表单规范 | 页码恢复、搜索范围、保存失败重试、键盘操作均达标 |
| 4：全模块迁移 | 应用注册、授权、API/资源权限及分配、记录、个人中心 | 同类操作与主题一致；分页、筛选与分配数据完整性检查 |
| 5：入口与验收 | 登录/注册、OIDC 登录/授权、首页、错误页；真实环境回归 | 全量构建、必要 lint、行为测试、真实认证联调与多视口截图通过 |

阶段不是已批准的技术任务单，也不代表承诺的固定工期。先通过需求审阅，再形成文件级设计和任务依赖。底层交互变化会波及全部业务页，因此先用用户与角色页建立可运行的样板，再进行其余迁移。

## Skills 筛选结果

检索方式：先读取 skills.sh 排行，再执行 `npx --yes skills find vue`、`find frontend-design`、`find ground-first`。安装数与星数是 2026-11-08 的检索快照，不作为质量保证。GitHub API 达到限额后，通过仓库网页中的 stargazerCount 校验仓库热度。

| 技能 | 来源 | 检索安装数 | 源仓库 stars | 适合本次用途 | 阅读状态 |
|---|---|---:|---:|---|---|
| frontend-design | anthropics/skills | 1,091,224 | 189,107 | 产品视觉方向、排版、层级、避免模板化装饰 | 已读；用于方向草案 |
| web-design-guidelines | vercel-labs/agent-skills | 837,711 | 33,703 | 键盘、焦点、表单、导航状态、响应式检查 | 本地已存在；已获取最新规则 |
| vue-best-practices | vuejs-ai/skills | 44.8K | 3,272 | 状态所有者、组件职责、Composition API 与缓存边界 | 已读主文件；实现前还需核心参考 |
| vue-debug-guides | vuejs-ai/skills | 27.8K | 3,272 | 异步乱序、生命周期与路由调试 | 已读主文件；按具体缺陷读参考 |
| vue-router-best-practices | vuejs-ai/skills | 16K | 3,272 | 路由参数、守卫与页面生命周期 | 已读主文件 |
| webapp-testing | anthropics/skills | 200,381 | 189,107 | Playwright 行为验证与截图审阅 | 已读；后续可纳入正式测试流程 |

本轮检索与读取没有进行全局技能安装。Vue 专项来源是社区项目，不等同于 Vue 官方文档。实现时仍以 Vue / Vue Router 官方行为定义为准。React 专项技能与现有 Vue 项目不匹配，未作为迁移理由。

供后续项目级安装的命令：

```sh
npx skills add anthropics/skills@frontend-design
npx skills add vuejs-ai/skills@vue-best-practices
npx skills add vuejs-ai/skills@vue-debug-guides
npx skills add vuejs-ai/skills@vue-router-best-practices
```

来源：<https://skills.sh/>、<https://github.com/anthropics/skills>、<https://github.com/vercel-labs/agent-skills>、<https://github.com/vuejs-ai/skills>。规则原文：<https://raw.githubusercontent.com/vercel-labs/web-interface-guidelines/main/command.md>。

## 审阅与下一步

需求、技术设计与文件级任务已获用户批准，验收清单已写入 [checklist.md](checklist.md)。按本地 mew-spec 的明确阶段约束，验收清单批准后开始实现。审阅重点是可观察行为、全模块覆盖和真实联调证据。
