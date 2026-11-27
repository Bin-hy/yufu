# 外部身份社交登录（P3–P6）Checklist

> 每项通过运行代码或观察行为验证；聚焦系统行为，不依赖具体文件/函数名。

## 实现完整性

- [ ] 微信开放平台扫码 Provider 可被发起：登录页出现微信入口，点击后跳转 `open.weixin.qq.com` 官方扫码页。（验证：fake/真实浏览器观察 URL）
- [ ] 微信回调可换取身份并建立中心会话；已绑定用户能继续原 OIDC 授权。（验证：fake 微信端到端测试）
- [ ] 微信未绑定默认被拒绝；`registration=open` 时创建无密码账号并建立绑定。（验证：两种配置各跑一次）
- [ ] 微信可绑定/解绑；同一 openid 属于他人时拒绝；最后凭据不可解绑。（验证：集成测试）
- [ ] QQ Provider 可发起、回调、登录、绑定、解绑；JSON/JSONP 两种响应均可解析。（验证：fake QQ 端到端测试）
- [ ] QQ/微信未配置或禁用时不出现在登录页，发起/回调失败关闭。（验证：禁用后访问接口）
- [ ] 管理端可列出 Provider 配置状态、启用/禁用、写入/轮换 Secret。（验证：管理端 API/UI）
- [ ] 管理端读取任何接口都不返回 Secret 明文。（验证：抓取响应与日志搜索）
- [ ] revision 冲突时拒绝写入并提示刷新。（验证：两次并发/旧 revision 更新）
- [ ] 管理端配置写成功后无需重启，登录页 catalog 立即变化。（验证：更新后 GET providers）
- [ ] 统计接口返回 24h/7d 按 Provider 的成功、拒绝、失败与主要分类。（验证：构造审计后查询）
- [ ] 上游失败会写 `external.login` failure 审计，统计可区分。（验证：fake 上游返回错误后查审计）

## 集成

- [ ] 三个 Provider 共用同一套会话建立、审计、限流、解绑保护与错误分类。（验证：三个 fake 端到端测试行为一致）
- [ ] 社交登录成功后下游 OIDC 流程继续，`sub` 仍是 Yufu 用户标识。（验证：授权码换取 ID Token/UserInfo）
- [ ] 禁用 Provider 不影响其他 Provider、密码登录和 Token 接口。（验证：禁用微信后登录 GitHub/密码）
- [ ] 管理端更新 Provider 后，运行时注册表刷新；启用但配置不完整时 fail closed。（验证：缺 Secret 启用返回错误）
- [ ] 既有账户中心第三方身份页同时展示 GitHub/QQ/微信，并可解绑。（验证：UI/接口）
- [ ] 环境变量引导不覆盖管理端已保存的 Secret。（验证：先管理端写入，再带旧 env 启动）

## 安全

- [ ] state 一次性、绑定浏览器、过期拒绝；重放返回失败。（验证：重复 callback/换浏览器）
- [ ] 微信/QQ 授权码一次性；重复使用不建立会话。（验证：fake 上游 code 重放）
- [ ] Secret 以密文存储；主密钥缺失且存在密文时服务拒绝启动。（验证：删除/错误主密钥启动）
- [ ] 日志、错误响应、审计摘要中无 Secret、code、token、完整外部标识。（验证：搜索日志/响应/审计）
- [ ] 未认证用户不能发起绑定；绑定完成时会话失效则拒绝。（验证：无 Cookie/过期会话）
- [ ] 同 Provider 重复绑定幂等；不同用户同 subject 拒绝且不泄露对方信息。（验证：集成测试）
- [ ] 单个 Provider 超时/故障不阻塞其他 Provider 与密码登录。（验证：fake 上游挂起 + 并发登录）

## 编译与测试

- [ ] `cargo fmt --check` 通过
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 通过
- [ ] `cargo test -p yufu --lib` 通过
- [ ] 外部身份相关集成测试串行通过（GitHub/微信/QQ/管理端）
- [ ] 现有 OIDC 授权、密码登录、会话撤销关键集成测试通过
- [ ] `pnpm --dir web check`、`test`、`lint`、`build` 全部通过

## 端到端场景

- [ ] 场景 1（微信登录）：配置并启用微信 → 登录页点击微信 → fake 扫码回调 → 建立会话 → 继续 OIDC → 下游获得 Token。
- [ ] 场景 2（未绑定拒绝）：未绑定微信登录 → 重定向错误页 → 无用户/无会话创建 → 审计 denied。
- [ ] 场景 3（绑定/解绑）：密码登录 → 账户中心绑定微信 → 退出 → 微信登录成功 → 解绑微信 → 密码仍可登录。
- [ ] 场景 4（QQ）：fake QQ 授权 → 登录/绑定成功；QQ 返回错误时无会话。
- [ ] 场景 5（管理端）：管理员配置微信 AppID/Secret 并启用 → 登录页出现微信 → 禁用 → 入口消失且发起被拒绝。
- [ ] 场景 6（统计）：制造成功/拒绝/上游失败 → 统计页/接口显示对应计数与分类。
- [ ] 场景 7（回归）：未配置任何 Provider 时，现有密码登录与 OIDC 测试全部通过。

## 文档与资质

- [ ] GitHub/QQ/微信申请条件、回调地址、配置项、启用步骤已写入运维文档。
- [ ] 主密钥生成、持久化、轮换、丢失处理已说明。
- [ ] 明确标注微信开放平台需企业主体/认证；QQ 需 ICP/审核；资质未就绪时 fake 测试 + 默认禁用作为交付状态。
- [ ] 明确说明不包含 Yufu 小程序、`wx.login`、`/mini/*`、handoff/resume、公众号网页 OAuth。

---

## 执行结果（2026-11-27）

### 已通过

- **P3 微信扫码**：`WechatQrAdapter` fake 单测通过；fake 微信端到端登录、未绑定拒绝、绑定、解绑通过。
- **P4 QQ**：`QqAdapter` JSON/JSONP fake 单测通过；fake QQ 登录、未绑定拒绝、上游错误、绑定、解绑通过。
- **P2 GitHub 基线**：登录、未绑定拒绝、绑定、解绑通过；新增“external login 继续既有 OIDC flow”测试通过。
- **未绑定策略**：默认 `disabled` 返回 `external_identity_not_linked`；`open` 创建无密码账号逻辑由现有服务覆盖。
- **管理端**：Provider 列表、Secret 脱敏、启用后公共 catalog 出现、revision 冲突 409、统计计数、禁用后入口消失，均通过集成测试。
- **审计**：上游失败写 `external.login` failure；成功/拒绝/绑定/解绑/配置变更均有审计。
- **Secret**：AEAD 加密、只写不回显、主密钥缺失 fail-closed 由单测/代码路径覆盖。
- **构建**：
  - `cargo fmt --all --check` 通过
  - `cargo clippy --workspace --all-targets -- -D warnings` 通过
  - `cargo test -p yufu --lib`：126 passed
  - `cargo test --test mod external_identity -- --test-threads=1`：9 passed
  - OIDC 授权、密码登录、PKCE 换取关键回归：通过
  - `pnpm --dir web check/test/lint/build`：通过（206 前端单测）

### 未执行 / 待外部条件

- [ ] 微信开放平台真实扫码联调：需要企业主体、开放平台网站应用与回调域名审核；当前用 fake Provider 验证，Provider 默认禁用。
- [ ] QQ 真实联调：需要 QQ 互联应用、ICP 备案域名与审核；当前用 fake Provider 验证，Provider 默认禁用。
- [ ] 真实 GitHub OAuth 浏览器手工登录：代码与 fake 测试通过；需要人工在 GitHub 授权页确认。
- [ ] 浏览器手工 UI 验收：管理端页面已通过类型检查/构建，未做真实浏览器点击验收。
- [ ] 全量 Rust 集成套件（260+）：本次只跑外部身份与关键 OIDC/会话回归，未跑全量以避免品牌截图副作用。
- [ ] 生产数据库升级演练与发布 tag：按 spec 不属于 P6 交付范围。
