# 官网截图与容器发布排查

日期：2026-11-13，Asia/Shanghai。

## 取消原因与证据边界

用户提供的 [job](https://github.com/Bin-hy/yufu/actions/runs/43284013350/job/131728544513) 在 2h 0m 15s 后 cancelled，并明确确认没有人工取消。最后一行是 BuildKit `#43 5416.1 Compiling toml ...`，并无 Rust 编译错误。

改动前本地 `.github/workflows/docker.yml` 的发布 job 限制为 `timeout-minutes: 120`。根据这组证据，判断是命中任务超时上限，15 秒差值可包含停止/清理时间。远端连接器对该 run/job 返回 404，GitHub CLI 未登录，因此没有声称读取过远端 annotations 或核实该次 run 的精确源码 SHA。

旧配置在 amd64 runner 使用 QEMU 编译 arm64 Rust；同一 builder 的两个架构还共用 `/src/target` 的 locked mount。Docker 官方说明 QEMU 对编译等 CPU 密集任务明显更慢。registry cache 不保存 cache mount 内容；原先整个仓库 COPY 还会让官网截图更新使 Rust 层失效。

## 本地修正

- 双架构原生 matrix：amd64 → ubuntu-24.04；arm64 → ubuntu-24.04-arm。保留每个编译任务的 120 分钟边界。
- 编译前验证原生 CPU；架构独立的 registry cache 和 target mount；前端产物在 BUILDPLATFORM 构建。
- 构建先推 digest，在对应架构执行版本冒烟，再上传 digest。仅两边成功后合并；dry-run 校验 manifest 恰好有 linux/amd64 与 linux/arm64 后才移动标签。
- 普通版本保留 commit/version/minor/latest；预发布只保留 commit/version；在编译前拒绝非法版本和预发布标志。
- docs/ 目录与官网源码/截图排除出 Docker 上下文，保留 website/package.json 给 pnpm workspace。
- 没有触发远端 run、推送镜像或修改已发布版本；真实性能改善需由下一次远端原生构建确认。

## 官网截图

已使用实际 Vue 页面、拦截的虚构 API 响应重拍五张现有图，并新增：

- console-registration.png：名称与用途，自定义标识默认折叠。
- console-registration-development.png：启动配置、自动 Audience 与回调；长表单完整截图，未遮挡步骤和底部按钮。
- console-apps.png：生产筛选、多来源和启动入口。

共八张图已逐张查看；中英文控制台指南共享同一组图片，文案说明截图为虚构演示数据。截图脚本拒绝外部请求和所有写请求，无业务数据或真实凭据。

## 验证

- 截图脚本：8 张实际生成，未出现意外请求或页面异常。
- 官网 check：0 errors / 0 warnings；build：27 页及 8 个优化图像生成。
- Dockerfile：`docker buildx build --check --file deploy/docker/Dockerfile .` 无警告；这是静态检查，不代表双架构编译通过。
- 元数据脚本：6 个正反例通过；7 个 shell 块通过 bash -n；matrix 与 needs 约束通过。
- manifest 脚本：5 个正反例通过，缺少 digest、缺少平台和意外 OS 时不发布标签；允许 provenance 的 unknown/unknown 项。
- actionlint v1.7.12 对 docker.yml、release.yml、rust.yml 三个工作流检查通过。旧 v1.7.7 不识别 rust.yml 的 YAML alias，按上游发布记录改用支持 anchors 的 v1.7.12 后通过，未为旧工具改写合法配置。
- scratch 文件导出实测：website 构建上下文只包含 website/package.json，内容与原文件一致；截图和源码树未进入上下文。

资料：[GitHub timeout](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idtimeout-minutes)、[native multi-platform](https://docs.docker.com/build/building/multi-platform/)、[cache mount](https://docs.docker.com/reference/dockerfile/#run---mounttypecache)。
