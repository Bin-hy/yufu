# 产品可用性实施与验收记录

基线：455d08bcf3f453b5cbb5a246ce1cc899ddf3ebcb。日期：2026-11-13，Asia/Shanghai。

四份设计文档已获批准；进入实现。所有 checklist 行为项目尚待执行，完整发布未完成。

## 环境

- Node.js v22.23.4、pnpm 11.25.0、Cargo 1.97.1。
- 默认沙箱不能连接 Docker socket 或绑定测试端口；已通过受控执行权限运行可销毁数据库及回环测试，没有使用业务数据库。
- CodeGraph 工具未在本会话提供，使用具体文件阅读；没有将历史验证结果算成本次结果。

## 任务进度

| 范围 | 状态 | 实际结果 |
| --- | --- | --- |
| T001 基线 | 已记录 | 初始只有本需求的未跟踪文档；未有实现修改 |
| T002—T008 能力与证据 | 进行中 | 先建立验证工具与准确能力状态 |
| T009—T011 启动配置 | 局部通过 | LaunchUrl、旧 payload 往返及说明规范化单元测试通过；集成覆盖仍在扩展 |
| T013—T019 成员访问及启动 | 进行中 | 已实现环境/名称服务端筛选、来源及重新准入后的 303；管理员 explain 和完整撤权矩阵未完成 |
| T020—T021 控制台入口 | 进行中 | 配置编辑、审批差异、生产默认筛选、来源与同源链接已有实现；三引擎尚未验收 |
| T022—T101 其余实施 | pending | 按任务依赖推进，不以设计文档代替实现 |

## 验收证据

尚未取得完整发布通过证据。下列为本地检查结果，源码继续变化时需重跑，不能作为最终候选完整证据。后续记录包含 C编号、状态、实际结果、完整命令/用例、源码身份、环境/时间、附件及摘要；远端三平台结果缺失时保留 blocked。

### 本地执行记录（2026-11-13，实施中）

- `python3.12 -m unittest discover -s scripts/tests -v`：27 项通过（此前基线；源码变更后需更新）。
- `cargo test -p yufu --lib -- --test-threads=1`：默认沙箱 89 通过/5 端口权限失败；受控权限重跑 94 项全部通过。
- `cargo test -p yufu-root --test mod product_launch_http -- --test-threads=1`：1 项通过，使用独立 PostgreSQL 容器；已检查未登录拒绝、查询参数注入拒绝、HTTPS 303、no-store/no-referrer、服务端筛选和维护来源。
- `cargo test -p yufu-root --test mod identity_console -- --test-threads=1`：旧接口回归 4 项全部通过。
- 在 `web` 执行 `./node_modules/.bin/vitest run`：13 个文件、104 项全部通过。
- 在 `web` 执行 `./node_modules/.bin/vue-tsc -b --pretty false`：退出 0；改动文件 ESLint 退出 0。
- pnpm 11 默认脚本执行触发依赖状态检查并尝试访问仓库，沙箱拒绝；已停止该进程，前端改用现有安装的同一工具执行，未修改依赖版本或锁文件。

尚缺：完整人员流程、结构化审计、全量待办、部署诊断、运行监测/告警、三引擎、性能、升级恢复及远端三平台候选证据。

### 应用申请流程跟进（2026-11-13）

用户要求检查 Key 与完整申请流程。已自动生成标识及各环境 Audience，折叠自定义标识，修正第三方归属和 local_only 确认；完善环境回执、生产深链；阻止未保存配置提交；保存失败不再误报成功。`identity-ui.test.ts` 与 `approval-snapshot.test.ts` 共 18 项通过，类型检查及 Web 构建通过。Chromium 两条真实场景已通过：自动标识创建 Web A、SPA B、开发协作/交接、测试用户登录，以及生产申请/审批/分配/生效、未分配访问拒绝。前端全量 108 项通过；全量 ESLint、类型检查、Web 构建、官网 check 和工作区 Clippy 均退出 0。Python 工具测试 29 项通过。

实际浏览器命令（从仓库根目录）：

```sh
COBOT_SSO_PW_ARGS='--project=chromium enterprise-sso-developer-onboarding.spec.ts enterprise-sso-production-governance.spec.ts' npm_config_verify_deps_before_run=false cargo test -p yufu-root --features webui --test mod identity_browser_cross_site_three_engines_login_sso_and_logout -- --nocapture --test-threads=1
```

Playwright 2 passed，Rust 外层 1 passed；只代表 Chromium 的这两条场景，不能因 Rust 测试名称含 three_engines 而记成三引擎通过。初次运行因缺少 Chromium 失败，安装当前 Playwright 所需引擎后重跑通过。Web A 的自动生成标识已由真实 API 断言；日志中的应用标识已改为读取实际服务端字段，避免继续使用旧固定字符串。

在途审计实现：条件与 cursor 单元测试 2 项通过；新关系表、事务关联、时间/UUID keyset HTTP 搜索已接入，跨页筛选和错条件 cursor 集成测试 1 项通过；控制台搜索、历史回填及完整性能/授权矩阵尚未完成。

最后一次全量 Rust 单元重跑的自动权限审核及一次重试均超时，未进入测试进程；没有判定代码失败，也未将其记为通过。已完成的针对性单元、隔离数据库、Chromium 和 Clippy 结果如上。先前 94 项全量单元通过属于当时源码，完整最终候选仍需重新采集。

安全沙箱内的替代检查 `cargo test -p yufu --lib domain::identity -- --test-threads=1` 已完成，48 项全部通过，涵盖本次修改的身份领域规则。


### 官网截图与容器超时修正（2026-11-13）

详见 [排查记录](container-build-investigation.md)。用户确认非人工取消；结合 2h 0m 15s 与旧 job 的 120 分钟上限，判断为超时。未能从连接器读取该远端 job 的 annotations，因此没有把本地推断标为远端确认。

官网八张图重拍并逐张查看，新增注册基础信息、开发配置和可访问应用图；中英文指南共享实际截图。官网 check 0 errors/0 warnings，build 27 页及8图通过。

容器改为双原生 runner 并行、架构独立缓存、digest 冒烟后合并、发布前校验双架构 manifest，保留预发布标签边界。docs/ 与官网内容不进入构建上下文，但 website/package.json 保留。actionlint v1.7.12 三个工作流通过；Dockerfile --check 无警告；6 个元数据正反例、7 段 bash 语法、5 个 manifest 正反例通过；scratch 上下文导出实测仅包含官网 package.json。没有触发/重跑远端 Actions 或推送镜像；原生构建实际耗时待远端证据。


### 本地两小时重新登录排查（2026-11-13）

用户最终更正为本地环境。只读取配置的非秘密时长字段：conf/yufu.local.toml 未覆盖 identity.sso_session_ttl_seconds，源码默认 43200；旧 auth.exp_seconds=7200。本机4000监听进程为 target/debug/yufu。没有读取业务会话表或用户浏览器凭据，尚不能认定两小时现象的直接原因已被定位。

确认并修复路由守卫缺陷：身份恢复发生临时网络/服务错误时不再把已有登录直接跳为密码登录，而是中止本次导航并保留原页面。初次恢复失败提供重试原会话的入口；401 清理身份与临时错误后再显示密码表单；重试跳转仍限制为内部工作区。

身份会话/UI 针对性22项通过，前端全量112项通过；类型检查、ESLint通过。隔离 PostgreSQL 与测试时钟用例 identity_browser_center_lifetime_is_independent_of_two_hour_legacy_jwt 通过：auth.exp_seconds=7200 时，超过两小时中心会话仍200，身份和原expires_at不变，十二小时边界401。此用例未连接本地业务数据库。模板和中英文配置说明已明确 SSO 与旧 JWT 的区别，未改用户本地配置和既有会话到期时间。

只读进程参数核对确认：本机4000端口的 yufu 实际使用 conf/yufu.local.toml；仅输出配置路径及白名单时长字段，没有输出完整命令或秘密。该文件仍没有 SSO 时长覆盖。Web 构建已完成，官网配置说明检查通过。此次已修复可复现的故障误跳转；用户实际约两小时退出的直接触发仍未从真实会话/响应中证实，不能标记根因闭合。
