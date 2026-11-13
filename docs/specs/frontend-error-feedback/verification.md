# 前端业务异常与反馈交互验收记录

日期：2026-11-13。范围：当前工作区的前端错误反馈；基于原有未提交改动继续实施。未修改 Rust、权限规则、数据库或标准 OIDC/OAuth 错误契约。

## 实际结果

| 检查 | 实际命令（在 web/ 执行） | 结果 |
| --- | --- | --- |
| 单元与组件回归 | `./node_modules/.bin/vitest run` | 退出 0；16 个文件、180 项测试通过，无未处理异常 |
| 类型检查 | `./node_modules/.bin/vue-tsc -b --pretty false` | 退出 0 |
| Lint | `./node_modules/.bin/eslint src tests *.config.*` | 退出 0 |
| 生产构建 | `./node_modules/.bin/vue-tsc -b --pretty false && ./node_modules/.bin/vite build` | 退出 0；构建 2903 个模块 |
| 浏览器交互 | `node tests/e2e/frontend-error-feedback.mjs` | Chromium 无头浏览器通过，写请求 1 次，pageerror 为 0 |
| 改动空白检查 | 仓库根目录执行 `git diff --check` | 退出 0 |

`pnpm` 运行入口曾尝试联网补齐依赖，受网络环境限制；最终使用现有 node_modules 中与 package.json 脚本相同的本地二进制运行。未安装新依赖、未修改依赖声明或锁文件。

构建有既有 Browserslist 数据过期提醒，不影响退出状态；本次未为更新该数据扩大依赖改动。

## 行为检查证据

以下条目均已通过。测试路径相对 `web/tests/unit/`，同一证据可以覆盖多个明确列出的条目。

| 条目 | 验证及实际结果 |
| --- | --- |
| C01、C02、C03 | `business-errors.test.ts` 逐类验证全部现有身份业务分类、9 类 CSRF 与旧安全响应；`admin-feedback.test.ts` 实际账户页面显示最后管理员原因；`approval-snapshot.test.ts` 验证配置冲突反馈 |
| C04 | `business-errors.test.ts` 验证标识只展示一次、非法标识回退响应头、超长标识被丢弃；`confirm-and-options.test.ts` 验证未知创建结果保留原请求标识 |
| C05、C06 | `business-errors.test.ts` 和 `http.test.ts` 验证 HTML、网络、超时、服务故障、限流与未知响应的安全降级；未知服务器对象及消息不直接显示 |
| C07 | 既有 `http.test.ts`、`workspace.test.ts` 等回归，以及 `feedback.test.ts` 的取消用例验证无可见错误、忙碌结束、旧会话请求隔离 |
| C08、C09 | `identity-ui.test.ts`、`approval-snapshot.test.ts`、`developer-validation.test.ts` 验证登录重试、注册已知失败重试、配置失败保留输入与字段级校验；秘密输入仍遵守原有清理规则 |
| C10、C11、C12 | `admin-feedback.test.ts` 的完整目录恢复流程与 `feedback.test.ts` 的查询/后续处理失败用例，验证失败不当作空数据、旧数据保留、已完成写入不误报失败 |
| C13、C14 | `http.test.ts` 验证保留 Axios 响应、local 不 Toast、并发不同异常实例同原因去重；实际账户页面组件与浏览器检查无额外 Toast |
| C15、C16 | `feedback.test.ts` 使用虚拟时钟验证 6 秒错误/警告、3 秒普通成功、显式 0、最多 4 条、独立重试、关闭与会话清理计时器 |
| C17 | 表单和列表流程使用原位错误；`feedback.test.ts`、`admin-feedback.test.ts` 与浏览器检查原确认框显示失败，无额外错误框；关键错误保留在页面，未知结果的全局提醒不自动关闭 |
| C18、C19 | `approval-snapshot.test.ts` 的实际应用编辑冲突用例验证读取最新数据保留草稿，写请求始终只有 1 次 |
| C20、C21 | `identity-ui.test.ts` 的未知创建结果流程与 `confirm-and-options.test.ts` 验证 commit_uncertain/网络失败之后只读核对，多次核对不再创建；同标识匹配明确要求核对，不伪造操作回执或密钥 |
| C22、C23 | 确认组件回归验证延迟执行防重复、失败保持打开、可安全重试成功后结束；浏览器验证对象与影响、忙碌禁用按钮、Escape 不关闭，最后管理员失败保持原确认框 |
| C24 | `feedback.test.ts` 验证未知提交禁用原写入、仍能取消；`confirm-and-options.test.ts` 验证卸载、会话重置及队列结算 |
| C25 | `business-errors.test.ts` 验证不同 401 分类在登录上下文统一为凭据失败；`identity-ui.test.ts` 验证实际登录表单失败到成功 |
| C26、C27、C28 | `identity-session.test.ts` 验证并发 401 单次过期处理和实际内存路由跳转、旧响应隔离、匿名/主动退出无异常提醒；现有 `identity-ui.test.ts` 验证秘密清理与非持久化 |
| C29 | `http.test.ts`、`identity-session.test.ts` 全部通过；既有 CSRF、刷新、同源限制和会话隔离回归保留 |
| C30、C31 | `feedback.test.ts` 验证可访问名称/角色、无回调不展示按钮、恢复忙碌与恢复失败反馈；Chromium 实际页面验证 Tab、Enter、焦点初始位置和关闭后恢复。已检查截图并修复错误区域横向挤压问题 |

## 完整流程证据

| 条目 | 实际验证 | 结果 |
| --- | --- | --- |
| E01 | `identity-ui.test.ts`：登录失败显示安全原因、保留用户名、重新输入密码后进入工作台 | 通过；组件与内存路由，模拟接口 |
| E02 | `admin-feedback.test.ts` + `frontend-error-feedback.mjs`：停用管理员 → 确认 → 最后管理员保护 → 原框保留原因 → 取消 → 恢复触发按钮焦点 | 通过；实际 Chromium 页面，API 由 Playwright 拦截模拟 |
| E03 | `admin-feedback.test.ts`：目录首读失败 → 重读成功 → 已成功写入后的刷新失败保留旧数据 → 重试恢复 | 通过；实际页面组件，模拟接口 |
| E04 | `approval-snapshot.test.ts`：编辑配置 → 冲突 → 核对最新数据保留输入 → 无额外写入 | 通过；实际页面组件，模拟接口 |
| E05 | `identity-ui.test.ts`：创建结果未知 → 多次只读核对 → 找到相同标识对象 → 展示核对入口，无虚构回执 | 通过；实际页面组件，模拟接口 |
| E06 | `identity-session.test.ts`：并发中心 401 → 一次清理和实际内存路由跳转 → 过期原因状态保留，既有旧响应测试防止恢复旧身份 | 通过；路由/会话集成，模拟接口 |

浏览器脚本可复用：先执行 `./node_modules/.bin/vite --host 127.0.0.1 --port 5179 --strictPort`，再执行 `node tests/e2e/frontend-error-feedback.mjs`。可用 `FEEDBACK_BASE_URL` 指定预览地址；脚本拦截所有 `/api/v1/` 请求，不调用真实业务写接口。截图保存在系统临时目录的 `cobotrix-feedback-dialog.png`，不是版本化附件。

## 质量与边界记录

- Q01—Q04：上述测试、类型、lint、构建均通过。浏览器脚本最终加入忙碌状态检查后，单独复跑浏览器与 lint。
- Q05：审阅前端改动与启动前工作区记录；保留原有功能改动，不修改后端、依赖或锁文件。未创建提交，避免把用户已有未提交工作混入提交。
- Q06：本记录将清单逐项关联到实际证据。前端范围验收通过；模拟接口不是实际后端集成验证。

本次未运行真实服务端端到端、Firefox/WebKit、多设备或屏幕阅读器实机验证，也未重新运行 Rust 检查。上述范围没有被计为通过，当前交付结论仅适用于本次前端错误反馈及记录中的 Chromium 场景。
