# 自动化证据索引

2026-11-08；最终源码上的生产预览测试。E为71个场景，每个在三引擎执行；U为42个单元/组件集成用例。完整原始结果见 [浏览器结果](e2e-results.json)、[单测结果](unit-results.json)。

<a id="e001"></a>

### E001

普通登录失败保留用户名，成功恢复站内返回目标

`auth.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e002"></a>

### E002

登录忽略不适合返回的目标 https://outside.test/path

`auth.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e003"></a>

### E003

登录忽略不适合返回的目标 //outside.test

`auth.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e004"></a>

### E004

登录忽略不适合返回的目标 /oidc/consent?client_id=x

`auth.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e005"></a>

### E005

注册校验、服务失败保留输入，成功后可直接登录

`auth.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e006"></a>

### E006

OIDC 登录与同意保留授权参数且按响应回跳

`auth.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e007"></a>

### E007

OIDC 登录与拒绝保留授权参数且按响应回跳

`auth.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e008"></a>

### E008

OIDC 同意失败留在原页可重试，已有同意自动返回

`auth.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e009"></a>

### E009

OIDC 刷新认证失败回对应登录入口并保留原参数

`auth.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e010"></a>

### E010

旧地址和安全 redirect 兼容，403/404直达有可用返回入口

`auth.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e011"></a>

### E011

撤销授权异步失败保持打开，防重复提交并恢复焦点

`business.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e012"></a>

### E012

角色编辑校验及失败不关闭，Esc 恢复触发按钮焦点

`business.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e013"></a>

### E013

创建用户失败保留表单，成功单次提交并返回列表

`business.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e014"></a>

### E014

用户角色修改、踢下线及删除保留业务操作

`business.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e015"></a>

### E015

API 权限新增包含 method；资源权限保留完整 CRUD

`business.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e016"></a>

### E016

authority 授权部分成功后只重试剩余差异

`business.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e017"></a>

### E017

permission 授权部分成功后只重试剩余差异

`business.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e018"></a>

### E018

应用注册部分失败重试不重复客户端，密钥不进入持久化

`business.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e019"></a>

### E019

应用更新和删除重试已完成步骤，重新生成密钥只一次

`business.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e020"></a>

### E020

用户 API 与资源权限覆写可允许、拒绝及恢复继承

`business.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e021"></a>

### E021

记录先加载，用户筛选重置分页，清除回全部接口

`business.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e022"></a>

### E022

个人中心保存昵称不清除密码草稿，修改密码后退出

`business.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e023"></a>

### E023

资源授权完整多页基线与增量候选，部分失败不删除未显示项目

`edge-cases.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e024"></a>

### E024

API 授权 8105 项完整读取；中间页失败禁用保存，重试恢复

`edge-cases.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e025"></a>

### E025

部分保存后实际状态再次读取失败，禁止盲目重试，核对后继续

`edge-cases.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e026"></a>

### E026

客户端已创建但响应丢失：核对一次并完成关联，说明密钥未收到

`edge-cases.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e027"></a>

### E027

创建结果无法确认时重试只核对，应用关联响应丢失也不重发

`edge-cases.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e028"></a>

### E028

删除触发对象后焦点回正文；仅键盘可完成编辑与菜单

`edge-cases.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e029"></a>

### E029

权限与应用字段错误定位、关联可读，修正后保留后续服务错误

`robustness.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e030"></a>

### E030

用户、权限和应用写成功后刷新失败，已更新或删除的对象不倒退

`robustness.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e031"></a>

### E031

刷新恢复当前页滚动，等待异步列表高度就绪

`robustness.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e032"></a>

### E032

撤销成功后的读取失败保留成功事实，不再提供重复撤销

`robustness.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e033"></a>

### E033

个人中心校验定位首错误，后续服务失败不被旧校验遮住

`robustness.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e034"></a>

### E034

慢旧读取在账号B登录后返回，不恢复A草稿；随后完成OIDC同意

`robustness.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e035"></a>

### E035

多个实际页面标签在小屏单行横滚，链接与键盘焦点可达

`robustness.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e036"></a>

### E036

完整列表流程保留分页，乱序结果正确，修改角色后关闭到邻页

`robustness.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e037"></a>

### E037

完整应用流程分步重试、密钥确认及撤销失败恢复

`robustness.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e038"></a>

### E038

1440×960 light：布局、触控和实际截图

`visual.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e039"></a>

### E039

1440×960 dark：布局、触控和实际截图

`visual.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e040"></a>

### E040

1280×800 light：布局、触控和实际截图

`visual.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e041"></a>

### E041

1280×800 dark：布局、触控和实际截图

`visual.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e042"></a>

### E042

768×1024 light：布局、触控和实际截图

`visual.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e043"></a>

### E043

768×1024 dark：布局、触控和实际截图

`visual.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e044"></a>

### E044

390×844 light：布局、触控和实际截图

`visual.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e045"></a>

### E045

390×844 dark：布局、触控和实际截图

`visual.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e046"></a>

### E046

360×800 light：布局、触控和实际截图

`visual.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e047"></a>

### E047

360×800 dark：布局、触控和实际截图

`visual.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e048"></a>

### E048

所有业务模块及入口使用统一布局，加载、空和失败有不同实际截图

`visual.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e049"></a>

### E049

应用失败、一次性密钥及移动认证的实际实现截图

`visual.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e050"></a>

### E050

主题跟随系统，手动选择保持；小屏可通过用户菜单切换

`visual.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e051"></a>

### E051

键盘跳转、焦点可见、减少动画及异步语义

`visual.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e052"></a>

### E052

light实际文本与背景对比度至少4.5

`visual.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e053"></a>

### E053

dark实际文本与背景对比度至少4.5

`visual.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e054"></a>

### E054

关闭右侧和其他标签保留首页；被关闭页重开建立新状态

`workspace-state.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e055"></a>

### E055

完整草稿流程：保留、后退取消、批量关闭、放弃后重开清空

`workspace-state.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e056"></a>

### E056

脏表单批量关闭取消保持所有标签；主动退出取消保留登录

`workspace-state.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e057"></a>

### E057

移动抽屉取消草稿导航仍打开，保留并切换后才关闭

`workspace-state.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e058"></a>

### E058

滚动、筛选和分页独立恢复；无效页码仅修正一次

`workspace-state.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e059"></a>

### E059

保存角色后失活用户列表下一次激活刷新，目录更新不丢权限草稿

`workspace-state.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e060"></a>

### E060

403保留登录，logout接口失败仍清空本地状态

`workspace-state.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e061"></a>

### E061

并发401单次refresh；失效清除脏表单，账号B不受旧响应影响

`workspace-state.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e062"></a>

### E062

脏表单刷新保护与提交pending导航保护

`workspace-state.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e063"></a>

### E063

Tab 地址与页状态保留，初访只请求一次

`workspace.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e064"></a>

### E064

慢旧请求无法覆盖快的新页，前进后退与地址一致

`workspace.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e065"></a>

### E065

首页别名只产生一个固定标签

`workspace.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e066"></a>

### E066

折叠侧栏可以直接进入创建用户

`workspace.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e067"></a>

### E067

关闭标签先完成导航，重新打开得到新实例

`workspace.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e068"></a>

### E068

新建用户默认角色不会产生脏表单，输入后可取消关闭

`workspace.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e069"></a>

### E069

390px 导航抽屉可用且页面不横向溢出

`workspace.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e070"></a>

### E070

用户菜单为有效菜单，退出清空标签与快照

`workspace.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="e071"></a>

### E071

刷新恢复同一 owner 标签，存储失败仍可使用

`workspace.spec.ts`；Chromium / Firefox / WebKit 均通过。

<a id="u001"></a>

### U001

同组件两个对象分别保留输入和query，关闭一个真实卸载且不影响另一个

`cobotweb/apps/cobotusr/tests/unit/cache-and-close.test.ts`；通过。

<a id="u002"></a>

### U002

活动标签回退导航被守卫拒绝时，标签、缓存和输入均保留

`cobotweb/apps/cobotusr/tests/unit/cache-and-close.test.ts`；通过。

<a id="u003"></a>

### U003

批量关闭包含脏表单时取消不删除任何页，确认后才销毁

`cobotweb/apps/cobotusr/tests/unit/cache-and-close.test.ts`；通过。

<a id="u004"></a>

### U004

慢关闭确认期间新导航或重复关闭，不允许旧事务删除新页面

`cobotweb/apps/cobotusr/tests/unit/cache-and-close.test.ts`；通过。

<a id="u005"></a>

### U005

右侧关闭保留左侧与固定首页，活动回退完成后再清理

`cobotweb/apps/cobotusr/tests/unit/cache-and-close.test.ts`；通过。

<a id="u006"></a>

### U006

取消回调抛错仍兑现为 false

`cobotweb/apps/cobotusr/tests/unit/confirm-and-options.test.ts`；通过。

<a id="u007"></a>

### U007

异步失败保持打开，重复确认只调用一次，重试成功兑现为 true

`cobotweb/apps/cobotusr/tests/unit/confirm-and-options.test.ts`；通过。

<a id="u008"></a>

### U008

卸载、会话重置及显式清理结束所有排队请求

`cobotweb/apps/cobotusr/tests/unit/confirm-and-options.test.ts`；通过。

<a id="u009"></a>

### U009

失活不启动，加载更多去重，失败重试同一页

`cobotweb/apps/cobotusr/tests/unit/confirm-and-options.test.ts`；通过。

<a id="u010"></a>

### U010

失活取消请求，晚响应不覆盖新源；恢复激活可继续读取

`cobotweb/apps/cobotusr/tests/unit/confirm-and-options.test.ts`；通过。

<a id="u011"></a>

### U011

读取 8105 条跨越旧上限，任意中间页失败拒绝半份基线

`cobotweb/apps/cobotusr/tests/unit/confirm-and-options.test.ts`；通过。

<a id="u012"></a>

### U012

超时但服务端已完成，通过读取恢复结果，不重发创建

`cobotweb/apps/cobotusr/tests/unit/confirm-and-options.test.ts`；通过。

<a id="u013"></a>

### U013

未知且无法核对，只重试读取，禁止再次 POST

`cobotweb/apps/cobotusr/tests/unit/confirm-and-options.test.ts`；通过。

<a id="u014"></a>

### U014

已知 HTTP 失败可重试；取消与普通异常不冒充提交结果未知

`cobotweb/apps/cobotusr/tests/unit/confirm-and-options.test.ts`；通过。

<a id="u015"></a>

### U015

普通按钮默认为 button，显式 submit 保留

`cobotweb/apps/cobotusr/tests/unit/forms-and-workflows.test.ts`；通过。

<a id="u016"></a>

### U016

select 数值与 null 不泄漏为字符串

`cobotweb/apps/cobotusr/tests/unit/forms-and-workflows.test.ts`；通过。

<a id="u017"></a>

### U017

分页仅发出一个页码事件，全部控件受 disabled 约束

`cobotweb/apps/cobotusr/tests/unit/forms-and-workflows.test.ts`；通过。

<a id="u018"></a>

### U018

异步默认值只重建该字段基线，用户已输入内容仍为脏

`cobotweb/apps/cobotusr/tests/unit/forms-and-workflows.test.ts`；通过。

<a id="u019"></a>

### U019

权限新增成功、删除失败后核对实际状态，只重试剩余删除

`cobotweb/apps/cobotusr/tests/unit/forms-and-workflows.test.ts`；通过。

<a id="u020"></a>

### U020

授权读取失败时禁止差异写入，恢复读取后可以继续

`cobotweb/apps/cobotusr/tests/unit/forms-and-workflows.test.ts`；通过。

<a id="u021"></a>

### U021

应用分步创建/更新/删除第二步失败后不重复第一步

`cobotweb/apps/cobotusr/tests/unit/forms-and-workflows.test.ts`；通过。

<a id="u022"></a>

### U022

主题可启动、恢复、切换；监听可释放且不会重复

`cobotweb/apps/cobotusr/tests/unit/forms-and-workflows.test.ts`；通过。

<a id="u023"></a>

### U023

并发 401 共用一次刷新，取消的等待请求不重放

`cobotweb/apps/cobotusr/tests/unit/http.test.ts`；通过。

<a id="u024"></a>

### U024

刷新失败终结全部等待请求，不产生悬挂订阅

`cobotweb/apps/cobotusr/tests/unit/http.test.ts`；通过。

<a id="u025"></a>

### U025

旧会话的成功响应被取消

`cobotweb/apps/cobotusr/tests/unit/http.test.ts`；通过。

<a id="u026"></a>

### U026

query 不产生新标签，参数决定详情标签身份

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。

<a id="u027"></a>

### U027

更新地址保留 cacheName，首页唯一且不可被关闭

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。

<a id="u028"></a>

### U028

快照剔除密钥、认证路由和 hash，owner 隔离

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。

<a id="u029"></a>

### U029

损坏及无法写入的 storage 不妨碍导航

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。

<a id="u030"></a>

### U030

会话切换释放 owner 并清理确认队列

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。

<a id="u031"></a>

### U031

初访只请求一次，乱序返回不会覆盖新页

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。

<a id="u032"></a>

### U032

失活中止读取，重新激活取消中的页只重试一次

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。

<a id="u033"></a>

### U033

相同查询刷新失败保留上次结果并可恢复

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。

<a id="u034"></a>

### U034

重复写入被拦截，旧会话写响应不调用成功回调

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。

<a id="u035"></a>

### U035

完整基线读取超过 1000 条，任一页失败拒绝伪完整数据

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。

<a id="u036"></a>

### U036

无效页码 -1 归一化

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。

<a id="u037"></a>

### U037

无效页码 0 归一化

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。

<a id="u038"></a>

### U038

无效页码 1.2 归一化

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。

<a id="u039"></a>

### U039

无效页码 abc 归一化

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。

<a id="u040"></a>

### U040

无效页码 999999999999999999999 归一化

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。

<a id="u041"></a>

### U041

过滤数组、敏感值，保留合法筛选

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。

<a id="u042"></a>

### U042

确认和取消都兑现 Promise，清理未结束队列

`cobotweb/apps/cobotusr/tests/unit/workspace.test.ts`；通过。
