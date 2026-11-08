# cobotusr 前端改版验收报告

2026-11-08，Asia/Shanghai；基线 `fa571d8`，验收运行基于当前改版源码，随后整理本地提交。当前分支 `codex/gateway-oidc-authorization`。

前端改版及自动化工程验证已完成。71个浏览器场景在三个引擎共213项全部通过，0失败、0跳过、0 flaky；42个单元/组件集成用例全部通过。类型检查、lint及临时目录生产构建退出码均为0。按78项验收标准：55项通过，20项部分通过，3项未验证。完整产品验收尚未完成，原因详见未覆盖项。

## 已交付行为

- 统一分组导航、侧栏/手机抽屉、顶栏、面包屑、单行可滚动标签及浅色/暗色/系统主题；用户、角色、权限、应用、审计、个人中心与认证入口统一迁移。
- 每页独立查询/草稿/缓存；切Tab恢复URL、分页、筛选和滚动，刷新等待异步列表高度后恢复滚动；关闭事务可取消，重开释放旧状态。
- 防止乱序读取覆盖、首访重复请求、并发401刷新悬挂、旧账号响应回写；写操作防重复，失败保留上下文，成功后的读取失败保留已完成事实。
- 权限完整基线跨页读取，8105项API授权及超过1000项读取验证；候选每批100继续加载，部分成功后只提交剩余差异。
- 应用非幂等创建结果未知时先核对，更新/删除保留完成步骤；一次性密钥仅在内存，手机成功结果直接展示，关闭时确认已保存，复制失败可见。
- Reka弹窗、确认、菜单、抽屉和Tooltip；修复WebKit鼠标点击不建立按钮焦点时的关闭恢复，字段错误关联及首错误定位。

## 验证与测量

| 检查 | 实际结果 | 证据 |
|---|---|---|
| vue-tsc / ESLint | 退出码0；lint零错误、零警告 | [verification.log](evidence/after/verification.log) |
| Vitest | 42/42 | [原始结果](evidence/after/unit-results.json)、[逐例索引](evidence/after/test-index.md) |
| Chromium 158.0.8127.18 | 71/71 | [浏览器JSON](evidence/after/e2e-results.json) |
| Firefox 159.0 | 71/71 | 同上 |
| WebKit 26.5，mac14冻结revision2251 | 71/71 | 同上、[环境](evidence/after/environment.json) |
| 临时目录生产构建 | 436150 bytes；逐文件gzip合计162120 bytes；assets前后哈希相同 | [逐文件测量](evidence/after/build-manifest.json) |
| 多视口/主题 | 1440×960 / 1280×800 / 768×1024 / 390×844 / 360×800；light/dark及system行为 | [截图和复现](evidence/README.md) |
| 对比度 | 4个页面实际浅/暗可见文本抽样至少4.5:1 | [浅色样本](evidence/after/contrast-light.json)、[暗色样本](evidence/after/contrast-dark.json) |

macOS14上的Playwright1.65与冻结WebKit命令协议不兼容，fixture仅在Darwin23使用`playwright-webkit1.61.0`匹配驱动，其他环境使用当前驱动。上下文和trace使用公开API，同场景不skip。本机WebKit默认Tab跳过链接/按钮，使用实测可遍历的Option+Tab完成键盘回归。这是WebKit引擎验证，真实Safari、iOS和Android设备另列未验证。

测试均使用独立上下文、虚构账号/密钥及受控接口。所有fetch/XHR被拦截；未匹配业务接口返回501并使测试失败，未处理页面异常也使测试失败。生产预览端口5188，不依赖现有业务后端。初轮失败和修复历史见 [failure-history.json](evidence/after/failure-history.json)，最新结论只来自最终完整运行。

gzip数值为逐文件压缩大小之和，不是实测首屏网络传输量；没有声称性能提升百分比。共享包源码没有改动，锁文件仅cobotusr importer变化；根workspace显式禁止playwright-webkit自动安装脚本，浏览器安装由开发者手动执行。具体依赖解析与范围见 [dependency-scope.json](evidence/after/dependency-scope.json)。

## C001–C078 逐项结果

“通过”限于本条描述的已执行前端验证；有未执行子场景保持“部分通过”。真实认证Cookie契约的例外在C013明确列出，不把localStorage白名单当作所有浏览器存储都无token。

| 检查 | 状态 | 可复现证据 | 未覆盖或限定 |
|---|---|---|---|
| C001 | 通过 | [E065](evidence/after/test-index.md#e065)、[E010](evidence/after/test-index.md#e010) | 已执行范围见关联用例。 |
| C002 | 部分通过 | [E066](evidence/after/test-index.md#e066)、[E048](evidence/after/test-index.md#e048) | 展开与小屏入口、折叠创建用户已验证；折叠状态所有入口未逐个执行。 |
| C003 | 通过 | [E057](evidence/after/test-index.md#e057) | 已执行范围见关联用例。 |
| C004 | 通过 | [E058](evidence/after/test-index.md#e058)、[E031](evidence/after/test-index.md#e031) | 已执行范围见关联用例。 |
| C005 | 部分通过 | [E063](evidence/after/test-index.md#e063)、[E035](evidence/after/test-index.md#e035) | 实际多个中文页面标签横滚及真实href已通过；未加入超长动态标题fixture。 |
| C006 | 通过 | [U005](evidence/after/test-index.md#u005)、[E054](evidence/after/test-index.md#e054) | 已执行范围见关联用例。 |
| C007 | 通过 | [U002](evidence/after/test-index.md#u002)、[E056](evidence/after/test-index.md#e056) | 已执行范围见关联用例。 |
| C008 | 部分通过 | [U004](evidence/after/test-index.md#u004) | 延迟关闭期间新导航/重复关闭已通过；延迟确认同时换账号未作为独立用例执行。 |
| C009 | 通过 | [U001](evidence/after/test-index.md#u001)、[E055](evidence/after/test-index.md#e055) | 已执行范围见关联用例。 |
| C010 | 通过 | [U001](evidence/after/test-index.md#u001) | 已执行范围见关联用例。 |
| C011 | 通过 | [U028](evidence/after/test-index.md#u028)、[E071](evidence/after/test-index.md#e071)、[E031](evidence/after/test-index.md#e031) | 已执行范围见关联用例。 |
| C012 | 部分通过 | [U029](evidence/after/test-index.md#u029)、[E071](evidence/after/test-index.md#e071) | 坏JSON、Storage读写拒绝已通过；过期版本和未知路由快照分支仅代码核对，未逐分支注入。 |
| C013 | 部分通过 | [U028](evidence/after/test-index.md#u028)、[E018](evidence/after/test-index.md#e018)、[E034](evidence/after/test-index.md#e034) | localStorage/sessionStorage及工作台快照不写密码、密钥、授权参数和结果；既有认证Cookie仍保存token，严格“所有浏览器持久化无token”不成立，未改变后端认证契约。 |
| C014 | 通过 | [U025](evidence/after/test-index.md#u025)、[E061](evidence/after/test-index.md#e061)、[E060](evidence/after/test-index.md#e060) | 已执行范围见关联用例。 |
| C015 | 通过 | [E056](evidence/after/test-index.md#e056)、[E061](evidence/after/test-index.md#e061) | 已执行范围见关联用例。 |
| C016 | 通过 | [U018](evidence/after/test-index.md#u018)、[E068](evidence/after/test-index.md#e068)、[E013](evidence/after/test-index.md#e013) | 已执行范围见关联用例。 |
| C017 | 部分通过 | [E055](evidence/after/test-index.md#e055)、[E057](evidence/after/test-index.md#e057) | 侧栏/标签/后退保留草稿已通过；全部程序导航与前进组合未逐个执行。 |
| C018 | 部分通过 | [E062](evidence/after/test-index.md#e062)、[E055](evidence/after/test-index.md#e055) | beforeunload原生保护、关闭/取消草稿已通过；真实关闭浏览器窗口未操作。 |
| C019 | 通过 | [U034](evidence/after/test-index.md#u034)、[E062](evidence/after/test-index.md#e062) | 已执行范围见关联用例。 |
| C020 | 通过 | [E064](evidence/after/test-index.md#e064)、[E021](evidence/after/test-index.md#e021) | 已执行范围见关联用例。 |
| C021 | 通过 | [E058](evidence/after/test-index.md#e058)、[U041](evidence/after/test-index.md#u041) | 已执行范围见关联用例。 |
| C022 | 通过 | [E063](evidence/after/test-index.md#e063)、[E048](evidence/after/test-index.md#e048) | 已执行范围见关联用例。 |
| C023 | 通过 | [U031](evidence/after/test-index.md#u031)、[E064](evidence/after/test-index.md#e064) | 已执行范围见关联用例。 |
| C024 | 通过 | [E063](evidence/after/test-index.md#e063)、[E059](evidence/after/test-index.md#e059) | 已执行范围见关联用例。 |
| C025 | 通过 | [U033](evidence/after/test-index.md#u033)、[E048](evidence/after/test-index.md#e048)、[E030](evidence/after/test-index.md#e030) | 已执行范围见关联用例。 |
| C026 | 通过 | [U032](evidence/after/test-index.md#u032)、[U025](evidence/after/test-index.md#u025) | 已执行范围见关联用例。 |
| C027 | 通过 | [U023](evidence/after/test-index.md#u023)、[U024](evidence/after/test-index.md#u024)、[E060](evidence/after/test-index.md#e060) | 已执行范围见关联用例。 |
| C028 | 部分通过 | [E021](evidence/after/test-index.md#e021)、[E048](evidence/after/test-index.md#e048) | 延迟用户选项不阻塞记录首屏已通过；每类选项和客户端失败组合未穷举。 |
| C029 | 通过 | [E011](evidence/after/test-index.md#e011)、[U007](evidence/after/test-index.md#u007) | 已执行范围见关联用例。 |
| C030 | 通过 | [E030](evidence/after/test-index.md#e030)、[E032](evidence/after/test-index.md#e032)、[E012](evidence/after/test-index.md#e012) | 已执行范围见关联用例。 |
| C031 | 通过 | [E014](evidence/after/test-index.md#e014)、[E011](evidence/after/test-index.md#e011)、[E019](evidence/after/test-index.md#e019) | 已执行范围见关联用例。 |
| C032 | 部分通过 | [E028](evidence/after/test-index.md#e028)、[E012](evidence/after/test-index.md#e012) | 实际键盘编辑/确认/Esc/关闭及删除触发对象焦点已通过；焦点循环边界未穷举。 |
| C033 | 通过 | [U015](evidence/after/test-index.md#u015)、[U016](evidence/after/test-index.md#u016)、[E013](evidence/after/test-index.md#e013)、[E020](evidence/after/test-index.md#e020) | 已执行范围见关联用例。 |
| C034 | 通过 | [E029](evidence/after/test-index.md#e029)、[E033](evidence/after/test-index.md#e033)、[E028](evidence/after/test-index.md#e028) | 已执行范围见关联用例。 |
| C035 | 通过 | [U008](evidence/after/test-index.md#u008)、[U006](evidence/after/test-index.md#u006)、[E061](evidence/after/test-index.md#e061) | 已执行范围见关联用例。 |
| C036 | 部分通过 | [E014](evidence/after/test-index.md#e014)、[E030](evidence/after/test-index.md#e030) | 完整角色选项及修改成功、刷新失败保留成功事实已通过；写失败保留选择未单独注入。 |
| C037 | 通过 | [E013](evidence/after/test-index.md#e013)、[E014](evidence/after/test-index.md#e014) | 已执行范围见关联用例。 |
| C038 | 通过 | [E012](evidence/after/test-index.md#e012) | 已执行范围见关联用例。 |
| C039 | 部分通过 | [E020](evidence/after/test-index.md#e020) | 两类覆写允许/拒绝/恢复继承已通过；快速换对象与多页候选组合未独立覆盖。 |
| C040 | 部分通过 | [E015](evidence/after/test-index.md#e015) | API method及资源CRUD payload已通过；所属应用筛选重置页码未单独执行。 |
| C041 | 通过 | [U011](evidence/after/test-index.md#u011)、[U009](evidence/after/test-index.md#u009)、[E024](evidence/after/test-index.md#e024)、[E023](evidence/after/test-index.md#e023) | 已执行范围见关联用例。 |
| C042 | 通过 | [U035](evidence/after/test-index.md#u035)、[E024](evidence/after/test-index.md#e024)、[E023](evidence/after/test-index.md#e023) | 已执行范围见关联用例。 |
| C043 | 通过 | [E023](evidence/after/test-index.md#e023)、[E024](evidence/after/test-index.md#e024) | 已执行范围见关联用例。 |
| C044 | 通过 | [U019](evidence/after/test-index.md#u019)、[E017](evidence/after/test-index.md#e017) | 已执行范围见关联用例。 |
| C045 | 通过 | [E025](evidence/after/test-index.md#e025)、[U020](evidence/after/test-index.md#u020) | 已执行范围见关联用例。 |
| C046 | 部分通过 | [E048](evidence/after/test-index.md#e048)、[E019](evidence/after/test-index.md#e019) | 应用分页组件、客户端信息及更新/删除已通过；客户端读取慢/失败组合未单独注入。 |
| C047 | 通过 | [U021](evidence/after/test-index.md#u021)、[E018](evidence/after/test-index.md#e018) | 已执行范围见关联用例。 |
| C048 | 通过 | [U013](evidence/after/test-index.md#u013)、[E026](evidence/after/test-index.md#e026)、[E027](evidence/after/test-index.md#e027)、[E019](evidence/after/test-index.md#e019) | 已执行范围见关联用例。 |
| C049 | 通过 | [E049](evidence/after/test-index.md#e049)、[E018](evidence/after/test-index.md#e018)、[E019](evidence/after/test-index.md#e019) | 已执行范围见关联用例。 |
| C050 | 通过 | [E011](evidence/after/test-index.md#e011)、[E032](evidence/after/test-index.md#e032) | 已执行范围见关联用例。 |
| C051 | 通过 | [E021](evidence/after/test-index.md#e021) | 已执行范围见关联用例。 |
| C052 | 通过 | [E022](evidence/after/test-index.md#e022)、[E033](evidence/after/test-index.md#e033) | 已执行范围见关联用例。 |
| C053 | 通过 | [E001](evidence/after/test-index.md#e001)、[E005](evidence/after/test-index.md#e005)、[E002](evidence/after/test-index.md#e002) | 已执行范围见关联用例。 |
| C054 | 通过 | [E006](evidence/after/test-index.md#e006)、[E007](evidence/after/test-index.md#e007)、[E008](evidence/after/test-index.md#e008) | 已执行范围见关联用例。 |
| C055 | 通过 | [E009](evidence/after/test-index.md#e009)、[U028](evidence/after/test-index.md#u028) | 已执行范围见关联用例。 |
| C056 | 通过 | [E010](evidence/after/test-index.md#e010) | 已执行范围见关联用例。 |
| C057 | 通过 | [E046](evidence/after/test-index.md#e046)、[E039](evidence/after/test-index.md#e039)、[E069](evidence/after/test-index.md#e069) | 已执行范围见关联用例。 |
| C058 | 部分通过 | [E035](evidence/after/test-index.md#e035)、[E046](evidence/after/test-index.md#e046)、[E051](evidence/after/test-index.md#e051) | 五视口局部表格横滚及多个实际标签已通过；超长动态列/标题未覆盖。 |
| C059 | 部分通过 | [E048](evidence/after/test-index.md#e048)、[E049](evidence/after/test-index.md#e049) | 全部模块代表页、通用列表loading/error/empty及应用部分失败/密钥有截图；非每个模块所有状态均截图。 |
| C060 | 部分通过 | [U022](evidence/after/test-index.md#u022)、[E050](evidence/after/test-index.md#e050) | 偏好恢复、系统监听与挂载前主题脚本已验证；未记录第一帧视频来量化主题闪烁。 |
| C061 | 通过 | [E051](evidence/after/test-index.md#e051)、[E064](evidence/after/test-index.md#e064) | 已执行范围见关联用例。 |
| C062 | 部分通过 | [E052](evidence/after/test-index.md#e052)、[E053](evidence/after/test-index.md#e053) | 首页、用户、应用、登录的实际浅/暗可见文本抽样≥4.5；全模块全状态完整对比度审计未完成。 |
| C063 | 部分通过 | [E051](evidence/after/test-index.md#e051)、[E028](evidence/after/test-index.md#e028) | 焦点、跳转入口、h1、aria-live语义已验证；实际读屏播报未验证。 |
| C064 | 部分通过 | [E046](evidence/after/test-index.md#e046)、[E039](evidence/after/test-index.md#e039) | 用户页面所有可见按钮/标签在五视口测量已通过；其他模块控件未逐个量测。 |
| C065 | 通过 | [验证日志](evidence/after/verification.log)、[构建测量](evidence/after/build-manifest.json) | 已执行范围见关联用例。 |
| C066 | 通过 | [U001](evidence/after/test-index.md#u001)、[U002](evidence/after/test-index.md#u002)、[E036](evidence/after/test-index.md#e036)、[E037](evidence/after/test-index.md#e037) | 已执行范围见关联用例。 |
| C067 | 通过 | [三引擎完整结果](evidence/after/e2e-results.json)、[环境](evidence/after/environment.json) | 已执行范围见关联用例。 |
| C068 | 未验证 | — | 需要实际Chrome/Edge/Firefox/Safari、iOS Safari和Android Chrome设备，当前仅自动化引擎与模拟视口。 |
| C069 | 未验证 | — | 缺少已授权测试后端账号与写入范围，本轮没有向真实业务后端发写请求。 |
| C070 | 未验证 | — | 缺少已授权真实OIDC测试客户端与回调环境，本轮协议流程均为模拟接口。 |
| C071 | 通过 | [依赖及共享范围](evidence/after/dependency-scope.json) | 已执行范围见关联用例。 |
| C072 | 通过 | [构建测量](evidence/after/build-manifest.json)、[E063](evidence/after/test-index.md#e063)、[首页截图](evidence/after/dashboard-1440-light.png) | 已执行范围见关联用例。 |
| C073 | 通过 | 本报告、[进度](progress.md)、[证据说明](evidence/README.md) | 已执行范围见关联用例。 |
| C074 | 通过 | [E036](evidence/after/test-index.md#e036) | 已执行范围见关联用例。 |
| C075 | 通过 | [E055](evidence/after/test-index.md#e055) | 已执行范围见关联用例。 |
| C076 | 通过 | [E023](evidence/after/test-index.md#e023) | 已执行范围见关联用例。 |
| C077 | 通过 | [E037](evidence/after/test-index.md#e037) | 已执行范围见关联用例。 |
| C078 | 部分通过 | [E034](evidence/after/test-index.md#e034) | 同上下文账号A→慢读→失效→B→旧响应→OIDC同意模拟通过；真实认证/OIDC尚未验证。 |

## 实现与验证边界

最新Web Interface Guidelines源在本机多次读取失败，未取得可引用的完整规范内容；本报告依据已批准需求、实际DOM/颜色/几何/键盘断言和截图评估，未声称完成最新规范逐条审核。此前按find-skills发现并采用的前端技能用于方向与实施，`/ground-first`没有可调用工具，实际采用代码、浏览器和接口证据先行。

仍待完成的范围包括部分压力/组合场景、全模块全状态对比度与控件几何量测、实际读屏和真实设备，以及真实后端认证/权限与OIDC联调。需要有明确授权的测试账号、可写数据、测试客户端及实际设备；前端模拟结果不替代这些结论。

本轮没有推送或部署。仓库中范围外已有assets占位文件删除未纳入本次提交，其他业务配置文件未处理。保留旧OAuth地址到统一应用页的兼容跳转，未向旧页面伪造全量搜索、事务或汇总统计能力。
