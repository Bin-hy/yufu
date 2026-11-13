# 前端业务异常与反馈交互 Plan

状态：已获用户批准，已完成实现及前端范围验收；实际证据与验证边界见 [verification.md](verification.md)。

## 架构概览

复用 Vue、Pinia、Axios 和现有 Dialog/AlertDialog，增加一个无界面依赖的错误归一化模块。请求层与异步组合函数共用解析结果，不再各自解释后端错误；页面决定本地错误的交互方式，全局反馈只处理未被页面承接的请求。

本期先消费已有后端公开错误码，不更改 Rust 或标准协议响应。覆盖现有身份、账户、开发者和管理员控制台，保留工作区其他未提交改动。

## 核心数据结构与接口

在 `web/src/http/errors.ts` 定义：

```ts
type ErrorRecovery = 'correct-input' | 'reload' | 'check-result'
    | 'sign-in' | 'contact-admin' | 'try-later' | 'none'

interface BusinessFailure {
    code: string
    title: string
    message: string
    requestId?: string
    status?: number
    recovery: ErrorRecovery
    cancelled: boolean
    outcomeUncertain: boolean
}

interface FailureContext {
    operation?: 'login' | 'read' | 'write'
}

function normalizeFailure(error: unknown, context?: FailureContext): BusinessFailure
function formatFailure(failure: BusinessFailure): string
function isCancelledFailure(error: unknown): boolean
```

`message` 包含公开原因和可执行建议，`formatFailure` 在末尾追加合法、限长的请求标识。解析返回值保持为普通数据，不包含路由、DOM、Toast 或闭包。

沿用 `RequestOptions.errorPresentation: 'local' | 'global'`。保留原 Axios 异常对象及响应，设置安全的 `error.message`，不破坏已有调用方通过状态码或错误码判断行为的方式。兼容原有 `errorMessage(error)` 导出，但内部委托统一模块。

`useAsyncMutation()` 保留 `pending/error/run` 契约，追加 `failure` 和显式 `clearError()`，供页面读取恢复语义。`useAsyncQuery()` 同样提供归一化错误信息，保留现有数据与刷新语义。

## 模块设计

### 1. 错误归一化（F1、F2、F7）

- 固定映射已有 IdentityError 与 CSRF 公开分类，统一标题、原因和建议。已知业务码优先于通用 message，防止同一错误被两个解析器解释成不同含义。
- 凭据失败区分登录上下文和受保护会话上下文；登录提示“账户或密码不正确”，不暴露账户存在性。权限、负责人、最后管理员、自审及过期审批分别给出明确文案。
- 响应请求标识优先读取合法响应体字段，再回退到响应头；限制字符与长度，不接受任意服务端内容。
- HTML、网络错误、超时、429 和 5xx 有固定安全文案。未知服务器字符串或对象不原样展示；已知旧版公开字符串通过显式映射兼容。
- 保留项目自行生成的可公开本地校验提示，限制内容和长度，排除 HTML、堆栈等异常内容。非 Error 值使用安全兜底。
- 取消及旧会话请求不展示错误。写请求发生网络错误或超时只说明结果可能未知，不假定失败，也不自动重放。

### 2. 请求层与异步状态（F1—F4、F7）

- `axios.ts` 移出重复文案解析，继续保留 CSRF、刷新、取消和会话隔离逻辑，按统一结果设置异常消息。
- `use-async-mutation.ts` 和 `use-async-query.ts` 使用同一解析结果，取消不产生可见错误；请求结算后正确解除忙碌状态。
- 所有已由页面、表单或确认框承接的请求使用 local，逐一核对实际 API 调用方；不全局静默没有错误宿主的调用。
- 成功写入后刷新读取失败，仅报告刷新失败并保留写入已完成的事实；不暗示用户需要重新提交。

### 3. Toast 与全局去重（F4）

- 复用现有 Toast store/host；错误与警告以类型和稳定文案建立去重键，同一键在短时间内更新或保留现有提示，不追加重复条目。请求标识不作为不同故障的唯一依据。
- 错误与警告延长默认展示时间；必须处理的重要错误由本地持久提示或对话框承接。
- 保留原有按异常对象去重，增加跨异常实例的短时间去重；设置可见数量上限，清理计时器并随会话重置。

### 4. 本地错误与恢复弹窗（F3、F5、F6）

- 新增可复用的 `failure-feedback.vue`，显示归一化原因、请求标识及可选的恢复按钮；调用方必须提供实际恢复回调才渲染操作按钮。
- 普通输入错误保留在表单内，不另开弹窗；加载失败继续使用 DataState 和现有重试入口。
- 表单内冲突/结果未知使用持久错误区域并给出“重新加载并核对”等可用动作。无表单承载的阻断写操作可用现有 Dialog 展示结果说明，关闭后保留页面错误，避免关掉弹窗即丢失原因。
- 不在已打开的编辑或确认框之上叠加错误弹窗，不为每一种业务错误强制弹窗。
- 危险操作优先把真实写入放入 `confirmCallback`，直接抛出错误交由 confirm-host 保持弹窗并显示原因。不得在回调里调用吞掉异常的 mutation.run 后将失败当作确认成功。
- 不能迁入确认回调的流程，在原页面明确显示执行失败；已有业务成功后刷新失败独立反馈。
- `createVerifiedCreate` 识别服务端 commit_uncertain 及既有网络结果未知，沿用“只核对，不重复创建”的路径。普通写请求不自动重试；刷新也不声称已经确认写入失败。

### 5. 会话过期（F7）

- 继续通过现有 expireSession 和统一身份失效处理器清理会话、跳转登录；旧会话后续响应直接取消。
- 会话过期仅提示一次，登录页保留明确原因。先完成安全清理再提示，不让弹窗阻塞会话撤销，也不保留旧账户敏感数据。
- 初次未登录访问和主动退出不展示“会话异常”提示；不为恢复表单而持久化密码、密钥或一次性链接。

## 模块交互

```text
后端异常 → Axios（会话隔离、统一解析）→ 保留原异常并抛出
  ├─ local → Query / Mutation / ConfirmHost → 页面或原弹窗展示
  └─ global → 归一化反馈 → 去重 Toast / 有实际恢复动作的说明框

受保护请求 401 → 现有会话失效处理 → 一次性登录原因提示
写入结果未知 → 本地持久反馈 → 只读核对现状 → 用户决定后续操作
```

## 文件组织

| 文件或目录 | 调整 |
| --- | --- |
| `web/src/http/errors.ts` | 新建统一解析与格式化 |
| `web/src/http/axios.ts` | 接入解析，保留认证与传输契约 |
| `web/src/composables/use-async-mutation.ts` | 兼容现有导出，增加归一化状态 |
| `web/src/composables/use-async-query.ts` | 统一读取错误与取消行为 |
| `web/src/composables/use-verified-create.ts` | 补充服务端结果未知分类 |
| `web/src/components/Feedback/failure-feedback.vue` | 新建持久反馈及受控恢复动作 |
| `web/src/components/Feedback/confirm-host.vue` | 统一确认执行失败原因 |
| `web/src/pinia/stores/toast.ts`、`web/src/utils/toast.ts` | 去重、时长与数量控制 |
| `web/src/workspace/session.ts`、`web/src/common/utils/auth.ts`、`web/src/router/guard.ts` | 按现有职责接入一次性会话失效说明 |
| `web/src/pages/`、`web/src/components/workspace/login-form.vue` | 核对本地承接、危险操作与恢复入口 |
| `web/src/style.css` | 按需补充反馈样式，复用既有设计变量 |
| `web/tests/unit/` | 错误映射、请求集成、取消、去重、确认失败及关键页面行为回归 |

任务拆解阶段进一步收敛需修改的页面清单；未发现缺陷的页面不做格式或布局改动。

## 技术决策

| 决策 | 理由 |
| --- | --- |
| 先复用已有错误码，前端集中解释 | 已有后端分类足够覆盖主要业务问题，避免更改协议 |
| 保留原异常而非替换为新 Error | 避免丢失响应、状态和既有调用方判断依据 |
| 本地承接优先，弹窗按是否需要决策选择 | 原表单更适合纠错，阻断操作才需要明确下一步 |
| 去重跨异常实例，按短窗口释放 | 并发请求可能生成不同异常对象，也允许用户后续重试再次收到提醒 |
| 恢复操作由页面显式提供 | 请求层不具备页面刷新、路由和业务核对上下文 |
| 不自动重复未知结果的写入 | 现有服务端和网络异常均可能发生在写入完成之后 |

## 验证策略

对纯解析执行表驱动测试，覆盖全部已知分类、未知数据、请求标识、脱敏、登录上下文和取消。请求层使用现有 Axios adapter 测试框架验证解析、local/global 分流及 CSRF/会话回归。组件测试验证提交失败不关闭确认框、保留表单、恢复按钮和去重。复用已有真实业务页面测试，再执行前端 test、check、lint、build；浏览器验证根据现有可用环境如实记录，不将静态检查替代交互验证。

本文件为设计，不表示上述行为已经实现或验证通过。
