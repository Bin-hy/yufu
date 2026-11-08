# cobotusr 改版实施进度

四份文档均已批准，2026-11-08。实现按 task.md 依赖推进；前端实现与工程自动化完成；T124为部分验证，T130–T132真实环境未验证。任务完成指实现及对应主要回归完成，细分场景限制以验收报告为准。

| 任务 | 内容 | 状态 | 证据 |
|---|---|---|---|
| T001 | 固定新增依赖与安装版本 | 完成 | 依赖/验证日志、单测与浏览器结果 |
| T002 | 建立单元测试入口 | 完成 | 依赖/验证日志、单测与浏览器结果 |
| T003 | 建立浏览器测试与隔离数据 | 完成 | 依赖/验证日志、单测与浏览器结果 |
| T004 | 启用有效 Vue/TS lint 配置 | 完成 | 依赖/验证日志、单测与浏览器结果 |
| T005 | 定义工作台契约与标签身份 | 完成 | workspace / cache-and-close 单测；workspace E2E |
| T006 | 验证标签身份边界 | 完成 | workspace / cache-and-close 单测；workspace E2E |
| T007 | 规范工作台叶子路由 | 完成 | workspace / cache-and-close 单测；workspace E2E |
| T008 | 验证路由兼容与规范首页 | 完成 | workspace / cache-and-close 单测；workspace E2E |
| T009 | 建立导航分组配置 | 完成 | workspace / cache-and-close 单测；workspace E2E |
| T010 | 改造标签状态所有者 | 完成 | workspace / cache-and-close 单测；workspace E2E |
| T011 | 验证标签更新与重置 | 完成 | workspace / cache-and-close 单测；workspace E2E |
| T012 | 实现快照编码与地址白名单 | 完成 | workspace / cache-and-close 单测；workspace E2E |
| T013 | 接入快照存取及故障回退 | 完成 | workspace / cache-and-close 单测；workspace E2E |
| T014 | 验证持久化范围 | 完成 | workspace / cache-and-close 单测；workspace E2E |
| T015 | 实现每页独立上下文 | 完成 | workspace / cache-and-close 单测；workspace E2E |
| T016 | 实现稳定缓存包装注册 | 完成 | workspace / cache-and-close 单测；workspace E2E |
| T017 | 接入唯一 AppMain 缓存及滚动 | 完成 | workspace / cache-and-close 单测；workspace E2E |
| T018 | 验证公开缓存机制及失活页面 | 完成 | workspace / cache-and-close 单测；workspace E2E |
| T019 | 建立会话 owner 与 epoch | 完成 | http / workspace / confirm-and-options 单测；乱序、401、8105项 E2E |
| T020 | 统一 HTTP 取消与错误归属 | 完成 | http / workspace / confirm-and-options 单测；乱序、401、8105项 E2E |
| T021 | 修复刷新等待队列 | 完成 | http / workspace / confirm-and-options 单测；乱序、401、8105项 E2E |
| T022 | 验证 HTTP 刷新与取消 | 完成 | http / workspace / confirm-and-options 单测；乱序、401、8105项 E2E |
| T023 | 实现写操作生命周期 | 完成 | http / workspace / confirm-and-options 单测；乱序、401、8105项 E2E |
| T024 | 验证重复提交与迟到写响应 | 完成 | http / workspace / confirm-and-options 单测；乱序、401、8105项 E2E |
| T025 | 建立读取请求的激活调度 | 完成 | http / workspace / confirm-and-options 单测；乱序、401、8105项 E2E |
| T026 | 阻止读取乱序及旧 finally 回写 | 完成 | http / workspace / confirm-and-options 单测；乱序、401、8105项 E2E |
| T027 | 统一读取缓存、失效与失败状态 | 完成 | http / workspace / confirm-and-options 单测；乱序、401、8105项 E2E |
| T028 | 验证读取乱序与请求次数 | 完成 | http / workspace / confirm-and-options 单测；乱序、401、8105项 E2E |
| T029 | 解析和校验页面 URL 查询 | 完成 | http / workspace / confirm-and-options 单测；乱序、401、8105项 E2E |
| T030 | 实现分页筛选动作与历史同步 | 完成 | http / workspace / confirm-and-options 单测；乱序、401、8105项 E2E |
| T031 | 验证查询及历史同步 | 完成 | http / workspace / confirm-and-options 单测；乱序、401、8105项 E2E |
| T032 | 实现分页候选选项 | 完成 | http / workspace / confirm-and-options 单测；乱序、401、8105项 E2E |
| T033 | 验证选项完整性与失败恢复 | 完成 | http / workspace / confirm-and-options 单测；乱序、401、8105项 E2E |
| T034 | 实现表单 baseline 与草稿登记 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T035 | 实现草稿检查注册表 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T036 | 验证表单与草稿语义 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T037 | 统一按钮与输入契约 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T038 | 规范选择与复选框值 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T039 | 验证表单基础组件 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T040 | 改造受控编辑弹窗 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T041 | 实现异步危险操作确认 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T042 | 实现侧边导航抽屉 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T043 | 实现菜单与 Tooltip 适配 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T044 | 验证浮层受控与异步失败 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T045 | 验证浏览器浮层焦点 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T046 | 统一全局确认宿主与 modal Promise | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T047 | 验证确认结束路径 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T048 | 统一 toast 与异步播报 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T049 | 接入导航与刷新草稿保护 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T050 | 实现可取消的标签关闭事务 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T051 | 验证关闭事务的用户行为 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T052 | 调整启动与工作台恢复顺序 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T053 | 统一退出与账号切换清理 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T054 | 验证启动与会话隔离 | 完成 | forms / confirm / cache-and-close 单测；workspace-state / business E2E |
| T055 | 建立语义视觉 token | 完成 | theme单测；五视口/主题截图与visual E2E |
| T056 | 实现主题启动与偏好 | 完成 | theme单测；五视口/主题截图与visual E2E |
| T057 | 验证主题偏好及监听 | 完成 | theme单测；五视口/主题截图与visual E2E |
| T058 | 建立统一页面与状态组件 | 完成 | theme单测；五视口/主题截图与visual E2E |
| T059 | 规范分页事件与输入 | 完成 | theme单测；五视口/主题截图与visual E2E |
| T060 | 验证分页边界与禁用 | 完成 | theme单测；五视口/主题截图与visual E2E |
| T061 | 重建响应式工作台壳 | 完成 | theme单测；五视口/主题截图与visual E2E |
| T062 | 重做侧栏导航与折叠菜单 | 完成 | theme单测；五视口/主题截图与visual E2E |
| T063 | 重做顶栏、用户菜单与面包屑 | 完成 | theme单测；五视口/主题截图与visual E2E |
| T064 | 重做标签栏与关闭操作 | 完成 | theme单测；五视口/主题截图与visual E2E |
| T065 | 建设真实工作台首页 | 完成 | theme单测；五视口/主题截图与visual E2E |
| T066 | 接入用户与角色 API 读取选项 | 完成 | business / robustness 用户、角色 E2E |
| T067 | 迁移用户列表查询与视觉 | 完成 | business / robustness 用户、角色 E2E |
| T068 | 迁移用户角色修改 | 完成 | business / robustness 用户、角色 E2E |
| T069 | 迁移删除用户与踢下线 | 完成 | business / robustness 用户、角色 E2E |
| T070 | 迁移创建用户初始化与校验 | 完成 | business / robustness 用户、角色 E2E |
| T071 | 迁移创建用户提交与离开 | 完成 | business / robustness 用户、角色 E2E |
| T072 | 验证用户样板全部关键操作 | 完成 | business / robustness 用户、角色 E2E |
| T073 | 迁移角色列表与编辑 | 完成 | business / robustness 用户、角色 E2E |
| T074 | 验证角色列表与表单 | 完成 | business / robustness 用户、角色 E2E |
| T075 | 分页化 API 权限 adapter | 完成 | forms / options单测；两类授权、资源CRUD与8105项 E2E |
| T076 | 分页化资源权限 adapter | 完成 | forms / options单测；两类授权、资源CRUD与8105项 E2E |
| T077 | 迁移 API 权限列表与新增 | 完成 | forms / options单测；两类授权、资源CRUD与8105项 E2E |
| T078 | 迁移资源权限列表与 CRUD | 完成 | forms / options单测；两类授权、资源CRUD与8105项 E2E |
| T079 | 实现 API 授权完整基线读取 | 完成 | forms / options单测；两类授权、资源CRUD与8105项 E2E |
| T080 | 验证两类授权基线 | 完成 | forms / options单测；两类授权、资源CRUD与8105项 E2E |
| T081 | 实现 API 授权差异提交 | 完成 | forms / options单测；两类授权、资源CRUD与8105项 E2E |
| T082 | 实现资源授权完整基线读取 | 完成 | forms / options单测；两类授权、资源CRUD与8105项 E2E |
| T083 | 验证部分成功与差异重试 | 完成 | forms / options单测；两类授权、资源CRUD与8105项 E2E |
| T084 | 实现资源授权差异提交 | 完成 | forms / options单测；两类授权、资源CRUD与8105项 E2E |
| T085 | 迁移 API 权限分配页面 | 完成 | forms / options单测；两类授权、资源CRUD与8105项 E2E |
| T086 | 迁移资源权限分配页面 | 完成 | forms / options单测；两类授权、资源CRUD与8105项 E2E |
| T087 | 回归两类权限业务 | 完成 | forms / options单测；两类授权、资源CRUD与8105项 E2E |
| T088 | 接入应用与 OAuth API 读取配置 | 完成 | workflow / verified-create单测；应用多步、密钥、撤销 E2E |
| T089 | 迁移应用列表独立数据区域 | 完成 | workflow / verified-create单测；应用多步、密钥、撤销 E2E |
| T090 | 实现应用注册分步状态 | 完成 | workflow / verified-create单测；应用多步、密钥、撤销 E2E |
| T091 | 验证应用创建部分成功 | 完成 | workflow / verified-create单测；应用多步、密钥、撤销 E2E |
| T092 | 实现应用更新与删除剩余步骤 | 完成 | workflow / verified-create单测；应用多步、密钥、撤销 E2E |
| T093 | 验证应用更新删除部分失败 | 完成 | workflow / verified-create单测；应用多步、密钥、撤销 E2E |
| T094 | 拆出应用编辑表单 | 完成 | workflow / verified-create单测；应用多步、密钥、撤销 E2E |
| T095 | 提供一次性密钥结果组件 | 完成 | workflow / verified-create单测；应用多步、密钥、撤销 E2E |
| T096 | 接入应用表单、密钥与危险操作 | 完成 | workflow / verified-create单测；应用多步、密钥、撤销 E2E |
| T097 | 验证应用全流程与敏感结果 | 完成 | workflow / verified-create单测；应用多步、密钥、撤销 E2E |
| T098 | 迁移已授权应用与撤销 | 完成 | workflow / verified-create单测；应用多步、密钥、撤销 E2E |
| T099 | 验证异步撤销基线缺陷 | 完成 | workflow / verified-create单测；应用多步、密钥、撤销 E2E |
| T100 | 迁移记录 API 与并行首屏 | 完成 | business / robustness 记录、个人中心、两类覆写 E2E |
| T101 | 验证记录加载与筛选 | 完成 | business / robustness 记录、个人中心、两类覆写 E2E |
| T102 | 迁移个人中心读取与昵称表单 | 完成 | business / robustness 记录、个人中心、两类覆写 E2E |
| T103 | 迁移密码修改与成功退出 | 完成 | business / robustness 记录、个人中心、两类覆写 E2E |
| T104 | 验证个人中心独立表单 | 完成 | business / robustness 记录、个人中心、两类覆写 E2E |
| T105 | 迁移用户覆写 API 与 API 权限面板 | 完成 | business / robustness 记录、个人中心、两类覆写 E2E |
| T106 | 迁移资源权限覆写面板 | 完成 | business / robustness 记录、个人中心、两类覆写 E2E |
| T107 | 接入用户覆写弹窗 | 完成 | business / robustness 记录、个人中心、两类覆写 E2E |
| T108 | 验证用户权限覆写 | 完成 | business / robustness 记录、个人中心、两类覆写 E2E |
| T109 | 建立统一认证视觉布局 | 完成 | auth E2E；路由兼容与引用核对 |
| T110 | 迁移普通登录和目标恢复 | 完成 | auth E2E；路由兼容与引用核对 |
| T111 | 迁移注册表单与结果反馈 | 完成 | auth E2E；路由兼容与引用核对 |
| T112 | 迁移 OIDC 登录保持协议参数 | 完成 | auth E2E；路由兼容与引用核对 |
| T113 | 迁移 OIDC 授权同意与拒绝 | 完成 | auth E2E；路由兼容与引用核对 |
| T114 | 验证普通与 OIDC 入口 | 完成 | auth E2E；路由兼容与引用核对 |
| T115 | 统一错误页与可用返回入口 | 完成 | auth E2E；路由兼容与引用核对 |
| T116 | 迁移旧 OAuth 组件的确认调用 | 完成 | auth E2E；路由兼容与引用核对 |
| T117 | 清理无引用包装与旧导航 | 完成 | auth E2E；路由兼容与引用核对 |
| T118 | 清理重复缓存及旧视觉补丁 | 完成 | auth E2E；路由兼容与引用核对 |
| T119 | 回归核心导航基线缺陷 | 完成 | workspace / workspace-state / robustness E2E |
| T120 | 回归标签操作及首访请求 | 完成 | workspace / workspace-state / robustness E2E |
| T121 | 回归草稿导航与缓存销毁 | 完成 | workspace / workspace-state / robustness E2E |
| T122 | 回归会话切换及存储故障 | 完成 | workspace / workspace-state / robustness E2E |
| T123 | 生成多视口与主题实现截图 | 完成 | evidence/after/*png；visual E2E |
| T124 | 核验对比度、焦点与触控 | 部分完成 | 4页浅/暗抽样与用户页几何/焦点通过；全状态、读屏未完成 |
| T125 | 完成类型和 lint 最终检查 | 完成 | verification.log / unit-results.json / e2e-results.json |
| T126 | 完成单测、构建及共享影响检查 | 完成 | verification.log / unit-results.json / e2e-results.json |
| T127 | 完成 Chromium 浏览器回归 | 完成 | verification.log / unit-results.json / e2e-results.json |
| T128 | 完成 Firefox 浏览器回归 | 完成 | verification.log / unit-results.json / e2e-results.json |
| T129 | 完成 WebKit 浏览器回归 | 完成 | verification.log / unit-results.json / e2e-results.json |
| T130 | 真实后端登录、刷新与权限联调 | 未验证 | 缺已授权真实测试环境或实际设备；参见C068–C070 |
| T131 | 真实 OIDC 同意与拒绝联调 | 未验证 | 缺已授权真实测试环境或实际设备；参见C068–C070 |
| T132 | 检查真实桌面与移动设备 | 未验证 | 缺已授权真实测试环境或实际设备；参见C068–C070 |
| T133 | 编写逐项验收报告 | 完成 | [逐项验收报告](acceptance-report.md) |

证据与实际聚合用例见 [索引](evidence/after/test-index.md)、[执行说明](evidence/README.md)。源码接口名和测试文件的实施差异记录在plan/task补充说明中。
