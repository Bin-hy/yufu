# Tag 自动打包与发版 Tasks

依据已批准的 [spec.md](spec.md) 和 [plan.md](plan.md)。本文件待批准；批准后进入验收清单设计，四份文档全部批准后才开始实现。

## 文件清单

| 操作 | 文件 | 职责 |
| --- | --- | --- |
| 新建 | `.github/workflows/release.yml` | tag 触发、预检、质量门禁、原生构建和最终发布 |
| 新建 | `scripts/release.py` | 版本校验、安装包生成、归档验证与校验和 |
| 新建 | `scripts/tests/test_release.py` | 发布工具的成功、边界与失败路径测试 |
| 新建 | `docs/releases/installation.md` | 随包安装说明及平台要求 |
| 修改 | `README.md` | 保留已完成的改写，补充下载和 tag 发版步骤 |
| 新建 | `docs/specs/tag-release/checklist.md` | 下一审批阶段编写的可执行验收清单 |
| 新建 | `docs/specs/tag-release/verification.md` | 实现后的实际验证结果与未执行项 |

不修改 Rust 业务代码、版本号、锁文件、现有 CI、官网部署流程及本地配置。

## T1：确认工具版本与发布命令契约

**文件：** `.github/workflows/release.yml`、`scripts/release.py`（本任务只确定实现输入，不提前写后续行为）。
**依赖：** 全部设计文档批准。
**步骤：**
1. 根据官方仓库确定 checkout、setup-node、setup-python、artifact Actions 的版本和提交 SHA，记录版本注释。
2. 核对四个平台 runner label 与 Rust target，并确定 Rust 工具链、安全检查工具及 pnpm 的安装方式。
3. 固定 `validate`、`package`、`checksums` 子命令参数、错误退出约定及工作流输出字段。

**验证：** 官方来源可追溯；CLI 帮助能显示参数；必填参数缺失返回非零。实现记录写入 `verification.md`。

## T2：先写版本校验测试，再实现预检

**文件：** `scripts/tests/test_release.py`、`scripts/release.py`。
**依赖：** T1。
**步骤：**
1. 使用临时 TOML 与 lockfile 编写稳定/预发布版本、非法格式、前导零、manifest 不匹配及三个 workspace 包锁版本不匹配的测试。
2. 实现严格 tag 解析及 workspace/lockfile 交叉校验，错误提示说明需要更新的版本来源。
3. 实现版本和 prerelease 输出，工作流输出通过环境指定的输出文件写入，不把 tag 拼入命令代码。

**验证：** `python3 -m unittest discover -s scripts/tests -v`；上述测试先观察失败，再实现至通过；`v1.2.3-rc.1` 正确标记为预发布。

## T3：编写随包安装说明

**文件：** `docs/releases/installation.md`。
**依赖：** T1。
**步骤：**
1. 编写四平台包选择、解包、配置、首次管理员登录和启动命令。
2. 区分本机 HTTP 参数与生产 HTTPS，说明数据库和可信时钟前置要求。
3. 添加 Linux/macOS SHA-256 与 Windows PowerShell 校验步骤；Linux systemd 指向现有模板约定的路径。
4. 说明 GNU/glibc 构建、Windows/macOS 手动运行及未签名/未公证状态，不承诺未测平台兼容性。

**验证：** 命令与配置示例路径在仓库中存在；二进制启动参数与 `--help` 一致；校验示例使用最终安装包命名规则。

## T4：先写归档生成测试，再实现白名单打包

**文件：** `scripts/tests/test_release.py`、`scripts/release.py`。
**依赖：** T2、T3。
**步骤：**
1. 用临时文件测试 tar.gz/zip 命名、同名顶层目录、配置及说明复制、Linux 专属服务模板、Unix 可执行权限。
2. 实现按 target 选择格式与包内白名单，不递归复制工作区目录。
3. 检查已构建 WebUI 的入口及静态资源；以参数数组执行二进制 `--version`，不连接数据库。
4. 写入 `BUILD-INFO.json`，包含 tag、版本、提交、target 与构建环境。
5. 对缺二进制、版本不符、缺前端产物及非支持 target 明确失败。

**验证：** 发布工具单测通过；检查虚构私钥、本地配置和 node_modules 不进入归档；Linux 包含模板，其他平台不包含。

## T5：先写归档完整性测试，再实现汇总校验

**文件：** `scripts/tests/test_release.py`、`scripts/release.py`。
**依赖：** T4。
**步骤：**
1. 测试完整四包、缺包、多包、错误版本/target、损坏归档、目录逃逸、符号链接和元数据不匹配。
2. 实现归档清单验证；检查必需文件、顶层目录、文件类型、权限及构建元数据。
3. 实现严格四包汇总与排序稳定的 `SHA256SUMS`，使用流式散列计算。
4. 将打包后复核与汇总时复核共用同一套验证逻辑。

**验证：** 发布工具单测通过；独立计算的摘要与 `SHA256SUMS` 一致；修改任一安装包后校验失败。

## T6：编写 workflow 触发与预检 job

**文件：** `.github/workflows/release.yml`。
**依赖：** T1、T2、T5。
**步骤：**
1. 配置 tag push 触发、只读默认权限、按 tag 互斥及禁用自动取消。
2. 添加 `validate` job，检出事件提交、安装 Python、执行工具单测和版本预检。
3. 将有效版本及 prerelease 状态作为 job outputs，下游不自行重新拼接或推断。
4. 所有 checkout 禁用凭据持久化，配置超时，禁止手动入口绕过预检。

**验证：** `actionlint .github/workflows/release.yml`；静态检查 tag 条件、并发键、只读权限及输出引用正确；非法 tag 测试返回非零。

## T7：编写发布质量门禁 job

**文件：** `.github/workflows/release.yml`。
**依赖：** T6。
**步骤：**
1. 添加依赖 `validate` 的 Ubuntu `verify` job，安装 Rust/Node/pnpm 和锁定依赖。
2. 构建控制台并执行 test/check/lint，安装现有集成测试需要的 Playwright 浏览器及系统依赖。
3. 执行 Rust fmt、Clippy、串行全 target/feature 测试及 doctest。
4. 添加 cargo-machete 与 cargo-audit，不静默忽略依赖或审计错误。
5. 确保各步骤默认失败即终止，不通过 `continue-on-error` 放行。

**验证：** actionlint 通过；核对所有已批准门禁均有命令；在本机可用环境执行对应检查并记录结果，资源或平台不足的项目明确留待 Actions 验证。

## T8：编写四平台构建与 artifact 上传

**文件：** `.github/workflows/release.yml`。
**依赖：** T4、T5、T7。
**步骤：**
1. 添加 Linux x64、Windows x64、macOS x64/ARM64 的原生 runner/target 矩阵，关闭 fail-fast。
2. 每个矩阵项安装依赖并构建前端，再以 `--locked --release --features webui` 构建指定 target 的 yufu。
3. 执行本平台二进制的版本与帮助 smoke test，记录环境及 Linux 动态依赖。
4. 调用打包工具并上传唯一命名的当前 run artifact，缺文件时报错。

**验证：** actionlint 通过；本机支持的平台实际构建、运行、打包及解包验证；四个平台的 smoke test 和 archive 校验均配置在各自 job 中，未远端执行前不标记跨平台通过。

## T9：编写最终汇总与 Release 草稿状态处理

**文件：** `.github/workflows/release.yml`。
**依赖：** T8。
**步骤：**
1. 添加仅此 job 授予 `contents: write` 的 `publish`，显式依赖全部矩阵构建成功。
2. 下载当前 run 四个 artifact，并生成严格匹配的校验清单。
3. 核对远端 tag 解引用提交未变化；查询 Release 时仅将 404 视作不存在。
4. 已公开 Release 直接拒绝；同 tag、同提交且带本流程标记的草稿允许复用，其他草稿拒绝修改。
5. 创建/更新带自动变更记录、来源及安装校验提示的草稿，正确设置 prerelease 属性。

**验证：** actionlint 通过；用本地可控 `gh` 替身演练不存在、匹配草稿、其他草稿、已公开、权限失败及 tag 变化等分支，断言失败分支不执行附件覆盖或公开动作；不访问真实写 API。

## T10：实现附件校验后公开与失败重试

**文件：** `.github/workflows/release.yml`。
**依赖：** T9。
**步骤：**
1. 只对已确认归属的同提交草稿清理旧附件，上传四个安装包和 `SHA256SUMS`。
2. 复核附件集合、大小，并下载到临时目录验证全部摘要。
3. 全部正确后才公开，稳定版本与预发布类型分别处理。
4. 上传或验证失败时保留未公开草稿；不删除 tag，不修改公开版本。

**验证：** 用 `gh` 替身分别演练完整成功、上传中断、缺附件、摘要不符和失败重跑，断言仅完整成功路径触发公开；actionlint 通过。

## T11：更新 README 的发布使用入口

**文件：** `README.md`。
**依赖：** T3、T8、T10。
**步骤：**
1. 在既有专业版内容中增加 GitHub Releases 下载入口和四平台安装包表。
2. 写明修改 workspace 版本、更新锁文件、验证并提交、创建附注 tag、推送指定 tag 的顺序。
3. 提供稳定与预发布示例，并链接随包安装说明、校验方法和失败重试约定。
4. 不实际创建 tag、提交或推送，不覆盖本次 README 既有修改。

**验证：** 检查 README 链接、命令、包名和发布工具保持一致；`git diff --check` 通过；命令示例无真实凭据。

## T12：按验收清单验证并记录交付边界

**文件：** `docs/specs/tag-release/verification.md`、已批准的 `checklist.md`。
**依赖：** T1—T11。
**步骤：**
1. 执行已批准 checklist 的每项验证，保存命令、实际结果和必要环境信息。
2. 汇总单测、workflow 静态检查、本机实际构建/打包及发布替身演练证据。
3. 区分已验证、失败阻塞及待 GitHub 远端验证项；不将本地 mock 结果写成真实 Release 已发布。
4. 检查 diff 仅涉及批准文件，原有 README 改写保留，无私钥、配置或其他敏感数据。

**验证：** 每项验收状态可追溯到证据；格式检查通过；不存在真实 tag/Release 写入；最终报告说明远端构建及发版尚未执行。

## 执行顺序

```text
T1 → T2 → T3 → T4 → T5 → T6 → T7 → T8 → T9 → T10 → T11 → T12
```

按顺序实施。实现前先批准独立的验收清单；每个任务完成验证后再标记状态。后续如需调整已批准范围，记录原因并确认，不用放宽门禁掩盖失败。
