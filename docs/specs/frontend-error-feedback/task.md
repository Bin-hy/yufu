# 前端业务异常与反馈交互 Tasks

状态：已获用户批准，已完成实现及前端范围验收；实际证据与验证边界见 [verification.md](verification.md)。

## 文件清单

| 操作 | 文件 | 职责 |
| --- | --- | --- |
| 新建 | `web/src/http/errors.ts` | 错误分类、文案、恢复语义与安全格式化 |
| 修改 | `web/src/http/axios.ts` | 统一解析并维持传输及会话契约 |
| 修改 | `web/src/composables/use-async-mutation.ts` | 归一化错误状态与取消处理 |
| 修改 | `web/src/composables/use-async-query.ts` | 读取失败与刷新失败状态 |
| 修改 | `web/src/composables/use-verified-create.ts` | 服务端结果未知的只读核对 |
| 新建 | `web/src/components/Feedback/failure-feedback.vue` | 持久错误及显式恢复操作 |
| 修改 | `web/src/components/Feedback/confirm-host.vue` | 确认执行失败的统一提示 |
| 修改 | `web/src/pinia/stores/toast.ts`、`web/src/utils/toast.ts` | 去重、展示时长和数量控制 |
| 按需修改 | `web/src/components/Feedback/toast-host.vue`、`web/src/style.css` | 配合提示状态与复用设计样式 |
| 修改 | `web/src/pages/admin/views/accounts.vue`、`groups.vue` | 危险操作失败与恢复入口 |
| 修改 | `web/src/pages/account/views/consents.vue`、`sessions.vue` | 撤销授权与会话操作反馈 |
| 修改 | `web/src/pages/developer/views/app-detail.vue`、`apps.vue` | 冲突、审批约束及创建结果核对 |
| 修改 | `web/src/components/workspace/login-form.vue`、`web/src/pages/login/index.vue` | 凭据失败和会话过期说明 |
| 按需修改 | `web/src/common/utils/auth.ts`、`web/src/workspace/session.ts`、`web/src/router/guard.ts`、现有身份失效处理器所在文件 | 一次性过期原因与现有跳转衔接 |
| 按需修改 | `web/src/pages/account/api/index.ts`、`web/src/pages/admin/api/index.ts`、`web/src/pages/developer/api/index.ts` | 校正实际缺失的 local/global 承接关系 |
| 新建 | `web/tests/unit/business-errors.test.ts`、`feedback.test.ts` | 解析和反馈组件行为验证 |
| 修改 | `web/tests/unit/http.test.ts`、`confirm-and-options.test.ts`、`identity-session.test.ts`、`forms-and-workflows.test.ts` | 请求、确认、身份及业务回归 |

按需项仅在存在接入缺口时修改，具体位置以当前工作区为准。所有任务保留用户已有修改，不做无关格式化。测试在 `web/` 下使用项目现有包管理器执行，命令以下统一记为 `pnpm`。

## T1：实现业务分类与文案

**文件：** `web/src/http/errors.ts`、`web/tests/unit/business-errors.test.ts`。
**依赖：** 无。
**步骤：** 定义设计中的 BusinessFailure、FailureContext、ErrorRecovery；建立全部身份业务码及 CSRF 错误的中文映射；实现 normalizeFailure 和 formatFailure；为凭据错误按登录上下文提供不暴露账户存在性的文案。
**验证：** `pnpm exec vitest run tests/unit/business-errors.test.ts`，逐类断言原因与恢复语义不同，负责人、最后管理员、自审、审批过期均有明确建议。

## T2：完善传输异常与脱敏

**文件：** 同 T1。
**依赖：** T1。
**步骤：** 补充取消、HTML、网络、超时、限流和服务异常；增加未知数据兜底、合法限长请求标识与已知旧响应映射；未知服务端字符串不直接显示；写入无响应归为结果未知。保留受控本地提示，不泄漏响应正文、堆栈或敏感数据。
**验证：** 运行 business-errors 测试，覆盖 null、非 Error、恶意 HTML、异常对象、超长标识、响应头回退、读写超时差异和取消。

## T3：接入请求拦截器

**文件：** `web/src/http/axios.ts`、`web/tests/unit/http.test.ts`。
**依赖：** T2。
**步骤：** 用统一解析替换内联文案分支；保留原异常、状态码和响应；local 不全局提示；继续使用已有 CSRF、刷新和会话代际规则，不自动重放失败写入。
**验证：** `pnpm exec vitest run tests/unit/http.test.ts`，现有 CSRF、取消、刷新和会话隔离回归通过，新增结构化异常与 local/global 提示断言。

## T4：统一异步状态与创建核对

**文件：** 三个异步组合函数、`web/tests/unit/confirm-and-options.test.ts`、相关异步状态测试。
**依赖：** T2。
**步骤：** 保留 errorMessage 和已有组合函数接口；增加 failure/clearError；取消不显示错误，结束忙碌状态；查询刷新失败保留旧数据。createVerifiedCreate 将 commit_uncertain 纳入只读核对路径，普通 HTTP 失败继续维持原语义。
**验证：** 运行 confirm-and-options 与异步状态相关测试；模拟服务端已完成但响应为结果未知，多次核对仅调用一次写入；取消不显示错误或卡住按钮。

## T5：实现持久错误组件

**文件：** `failure-feedback.vue`、`feedback.test.ts`，按需补充样式。
**依赖：** T2。
**步骤：** 显示标题、原因、请求标识；恢复按钮仅在有明确回调时显示；处理操作忙碌和键盘访问；未知结果不出现暗示再次写入的按钮；不将服务端字符串作为 HTML 渲染。
**验证：** `pnpm exec vitest run tests/unit/feedback.test.ts`，检查内容、按钮可用性、重复点击防护、可访问名称及文本安全渲染。

## T6：合并重复 Toast

**文件：** Toast store、emitToast、feedback 测试，按需修改 host。
**依赖：** T3。
**步骤：** 错误与警告按稳定分类/文案在短窗口去重；默认错误和警告显示 6 秒；可见提示上限 4 条；处理主动关闭、计时器、去重过期及会话清理。支持显式 durationMs，包括不自动关闭的 0。
**验证：** 使用虚拟时钟验证并发同类错误只出现一条、不同原因可区分、后续重试允许再次提示、最多 4 条、关闭和重置后无计时器残留。

## T7：完善确认框执行失败

**文件：** confirm-host、`confirm-and-options.test.ts`。
**依赖：** T2、T5。
**步骤：** 确认回调失败使用统一解析并保持原弹窗；取消不作为失败；执行期间禁用重复确认和关闭；会话重置结束队列；结果未知显示核对建议，禁用直接重复执行原写入的路径，由调用方提供只读核对动作或返回页面核对。
**验证：** 运行确认组件测试，分别模拟普通失败、业务约束、取消、commit_uncertain、重复点击和会话重置，断言弹窗状态及真实写入调用次数。

## T8：接入管理员危险操作

**文件：** accounts.vue、groups.vue、相关页面测试。
**依赖：** T3—T7。
**步骤：** 调整停用、撤销资格、撤销链接及用户组危险操作，能够在确认回调内执行的直接抛错交由宿主处理；保留对象与影响说明。最后管理员和负责人约束给出处理建议。提交成功与后续刷新失败分别表达，不误报写入失败。
**验证：** 组件测试模拟停用最后管理员、负责人阻断及刷新失败，确认失败不关闭、无重复写入、成功后读取失败不引导重新提交。

## T9：接入账户与开发者流程

**文件：** consents.vue、sessions.vue、app-detail.vue、apps.vue、forms-and-workflows 测试。
**依赖：** T3—T7。
**步骤：** 接入撤销操作与应用编辑的持久反馈；冲突和过期审批提供实际可用的重新加载/核对动作，保留尚未提交输入；创建结果未知使用只读核对；不叠加编辑框和错误框。核对其他控制台页面及 API，保留已正确承接错误的页面，修复确有缺口的调用。
**验证：** 模拟版本冲突、自审失败、已消费链接和未知创建结果，确认建议与上下文匹配、草稿保留、本地失败无额外 Toast、核对不增加写请求。

## T10：完善登录与过期恢复

**文件：** 登录表单/页面、现有身份失效处理器、session/auth/guard 按需修改、identity-session 测试。
**依赖：** T3、T6。
**步骤：** 找到并复用 setAuthExpiredHandler 现有注册点；过期会话先清理再跳转，在登录页保留一次性过期原因；并发 401 只触发一次流程；首次匿名访问及主动退出不显示异常。登录失败使用安全统一原因，不保存密码或一次性秘密。
**验证：** 运行 identity-session 与登录相关测试，断言并发 401、旧响应取消、主动退出、首次匿名访问以及凭据错误的提示和跳转次数。

## T11：交互回归与质量检查

**文件：** 本次变更文件与相关测试、后续 checklist.md 验证记录。
**依赖：** T8—T10。
**步骤：** 运行前端 test、check、lint、build；执行登录失败、表单失败、列表重试、确认失败和未知结果核对的浏览器或组件交互验证；检查键盘、焦点返回和请求标识脱敏。只修复本次引入的问题，既有失败和环境限制单列记录。
**验证：** `pnpm test`、`pnpm check`、`pnpm lint`、`pnpm build`；记录真实退出状态与交互证据，不将未执行项标为通过。

## 执行记录

T1—T11 已完成。统一分类、传输接入、异步状态、持久反馈、Toast、确认框、管理员/账户/开发者页面和会话提示均已接入；验证使用现有本地二进制，180 项测试、类型、lint、构建与 Chromium 场景通过，详见 verification.md。

补充实际文件：新增 `web/tests/unit/admin-feedback.test.ts` 与 `web/tests/e2e/frontend-error-feedback.mjs`；调整 `alert-dialog.vue` 支持未知结果禁用确认。无需修改的 API、本地路由入口和样式文件保持原状。

## 执行顺序

```text
T1 → T2 → T3 → T4 → T5 → T6 → T7 → T8 → T9 → T10 → T11
```

按顺序在当前会话执行，不委派子代理。每项先通过对应验证再记录完成；四份文档均获批准前不编写实现代码。
