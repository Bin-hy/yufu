# Go SDK 迁移 Tasks

- T1：记录 SDK 原始文件摘要，将源码、测试和 README 复制到独立仓库；复制许可证。验证：除明确修改的 README 外，原文件摘要全部相同。
- T2（依赖 T1）：更新独立 README，添加 CI、忽略文件。验证：Go 格式检查、race 测试与 vet 全通过；确认最低版本矩阵存在。
- T3（依赖 T2）：使用临时 workspace 同时加载 SDK 与服务端示例。验证：示例离线契约测试、race 检测和 vet 通过，integration tag 编译通过。
- T4（依赖 T3）：初始化独立仓库、提交并推送 main；查询真实 Go 伪版本。验证：远端 main 与本地提交一致，模块可下载。
- T5（依赖 T4）：移除示例 go.mod 的本地 replace，固定真实伪版本；更新服务端 README、示例 README、中英文指南和 Rust CI 过时路径。验证：无 workspace、无 replace 的示例测试与 vet 通过。
- T6（依赖 T5）：再次比较迁移前后的 SDK 文件摘要，确认无丢失后删除 `sdk/go`。验证：目录不存在、所有当前说明没有失效路径、官网检查通过；填写验收结果。

## 文件清单

- 新仓库：原 `sdk/go/**`、`LICENSE`、`.gitignore`、`.github/workflows/ci.yml`。
- 服务端修改：`README.md`、`examples/sdk/go/{go.mod,go.sum,README.md}`、`website/src/content/docs/{guides,en/guides}/go-sdk.mdx`、`.github/workflows/rust.yml`。
- 服务端删除：`sdk/go/**`。
- 迁移记录：`docs/specs/go-sdk-migration/{spec,plan,task,checklist}.md`。
