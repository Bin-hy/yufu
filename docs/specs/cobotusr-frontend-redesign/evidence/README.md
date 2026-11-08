# 前端改版证据与复现

最终结论见 [验收报告](../acceptance-report.md)。2026-11-08 的改版源码已通过42项单测和71场景×3引擎，共213项浏览器检查。原始结果及每个C编号的关联测试都可从报告进入。真实后端、真实OIDC及实际设备尚未验证。

## 改版验证复现

从仓库根执行：

```sh
pnpm install --frozen-lockfile --ignore-scripts
pnpm --filter @cobotweb/cobotusr exec node node_modules/@playwright/test/cli.js install chromium firefox webkit
```

在macOS14 / Darwin23上，额外安装冻结WebKit对应的驱动浏览器；fixture会自动选用该版本，其余平台沿用当前Playwright：

```sh
pnpm --filter @cobotweb/cobotusr exec node node_modules/playwright-webkit/cli.js install webkit
```

运行检查及浏览器测试：

```sh
pnpm --filter @cobotweb/cobotusr check
pnpm --filter @cobotweb/cobotusr lint
pnpm --filter @cobotweb/cobotusr test
pnpm --filter @cobotweb/cobotusr test:e2e --workers=3
```

浏览器测试会自动在临时目录构建生产产物、启动5188端口预览并清理临时目录。所有fetch/XHR均由fixture拦截，未匹配请求返回501且导致断言失败，未处理页面异常也会失败。测试不会向真实业务后端写入。macOS WebKit默认Tab跳过链接/按钮，键盘测试使用实测可遍历的Option+Tab。该测试引擎不是实际Safari或iOS设备结论。

单独检查临时生产构建：

```sh
cobotusr_build_dir="$(mktemp -d)"
pnpm --filter @cobotweb/cobotusr exec vite build --outDir "$cobotusr_build_dir"
```

此命令不覆盖仓库assets；本轮完整验证中的assets前后文件内容哈希相同，临时构建已清理。逐文件产物与gzip测量见 [build-manifest.json](after/build-manifest.json)。gzip合计是每个文件分别压缩后的大小总和，不代表真实首屏传输量。

## 最终证据

- [逐例索引](after/test-index.md)：71个E2E场景与42个单测的文件、标题及结果，验收报告按编号关联。
- [e2e-results.json](after/e2e-results.json)：最终三引擎213项全部通过，0跳过；[unit-results.json](after/unit-results.json) 为42项单测结果。
- [verification.log](after/verification.log)：最终类型、lint、单测和临时构建命令与退出码。
- [environment.json](after/environment.json)：macOS14.6、Node24.21.0、pnpm12.10.1和实际浏览器版本；WebKit26.5是冻结revision2251，匹配驱动1.61.0。
- [dependency-scope.json](after/dependency-scope.json)：仅cobotusr的锁文件importer改变，共享源码无改动；workspace禁用playwright-webkit自动下载脚本。
- [failure-history.json](after/failure-history.json)：初轮及修复前失败摘要；原始冗长初轮JSON已归并为摘要，最终结论来自最终完整运行。
- [contrast-light.json](after/contrast-light.json)、[contrast-dark.json](after/contrast-dark.json)：首页/用户/应用/登录四页实际可见文本背景、颜色和对比度；不是全模块全状态审计。

HTML报告在本机`/tmp/cobotusr-playwright-report`，重跑会覆盖。失败trace写入app的test-results并被gitignore排除；冻结WebKit驱动通过公开API单独附加trace。本次最终完整运行无失败。

## 实现截图

图片全部来自Chromium生产预览和虚构业务数据。`fullPage`不会展开正文内部滚动容器，列表截图代表当时可视区域。

| 范围 | 文件 |
|---|---|
| 五尺寸×浅/暗 | `after/users-{1440,1280,768,390,360}-{light,dark}.png` |
| 桌面业务与入口 | `after/*-1440-light.png`：首页、创建用户、角色、两类权限/分配、应用、授权、记录、个人中心、登录/注册、403/404、OIDC授权 |
| 通用列表状态 | [加载](after/users-loading.png)、[失败](after/users-error.png)、[空数据](after/users-empty.png) |
| 应用结果 | [部分失败](after/application-partial-failure-1440-light.png)、[桌面密钥](after/application-secret-1440-light.png)、[手机密钥](after/application-secret-390-dark.png) |
| 手机认证 | [登录](after/login-390-dark.png)、[注册](after/register-390-dark.png)、[OIDC登录](after/oidc-login-390-dark.png)、[OIDC授权](after/consent-390-dark.png) |

手机密钥截图中的`TEST_SECRET_MEMORY_ONLY`为专用模拟标记。密码、真实token和真实密钥没有写入证据。

## 基线记录

`browser-baseline.json` 是 2026-11-08 在基线提交 `fa571d8` 上的运行记录。截图采用模拟数据；没有连接真实后端。

## 复现

在仓库根目录启动 cobotusr：

```sh
pnpm --filter @cobotweb/cobotusr dev --host 127.0.0.1 --port 5178
```

另一个终端安装临时测试依赖并运行脚本；机器需已安装 Google Chrome：

```sh
npm install --prefix /tmp/cobotusr-audit-tools --no-audit --no-fund --ignore-scripts playwright
node docs/specs/cobotusr-frontend-redesign/evidence/browser-audit.cjs
```

脚本使用独立浏览器上下文、虚构 token，拦截全部 XHR / Fetch 并返回模拟响应。删除请求仅由拦截器模拟失败，不访问真实服务。结果写入 `/tmp/cobotusr-browser-baseline.json`，截图写入 `/tmp/cobotusr-before-*.png`；运行不会自动覆盖本目录存档。

## 数据解释

- `home`：首页重定向导致两个固定标签。
- `initialUserRequests`：首访同一个用户分页出现两个请求。
- `usersBeforeSwitch` / `usersAfterTabReturn`：第 3 页切回变为第 1 页。
- `collapsedSidebar` / `collapsedLinksAfterClick`：折叠态父菜单不能打开页面。
- `mobile`：390px 视口正文宽度只剩 134px。
- `outOfOrderPagination`：URL 指向第 3 页，表格显示第 2 页模拟用户。
- `dialogBeforeConfirm`：弹窗存在但没有 dialog 语义。
- `dialogWhilePending` / `dialogAfterFailure`：请求中、失败后确认弹窗都已关闭。
- `beforeLogout` / `afterSecondLogin`：清除用户并重新进入后，旧标签仍在。该步骤验证前端状态清理，没有执行真实登录协议。
- `pageErrors`：本次浏览器运行捕获到的未处理页面异常。

脚本是记录现状的探针，包含等待时间和模拟数据，不能替代改版后的正式断言测试，也不能用于证明真实认证及授权操作已经通过。
