# Go SDK 迁移 Plan

## 架构与接口

- 将 `sdk/go` 的内容复制到相邻的 `yufu-sdk-go` 仓库根目录；模块仍为 `github.com/Bin-hy/yufu-sdk-go`，公开 API、Go 1.22 最低版本和 `Version` 不变。
- 保留 `examples/sdk/go` 在服务端仓库，以继续使用现有 Docker、服务端构建及配置生成脚本。其 `go.mod` 改为依赖独立仓库的真实提交版本，移除永久 `replace`；开发者可用临时 `go.work` 进行跨仓库联调。
- SDK 的 `internal/jose`、`internal/pkce` 及所有测试原样迁移；不引入依赖或业务行为变更。

## 仓库与文档

- 复制 Apache-2.0 `LICENSE`，在 README 标注来源仓库及迁移基线提交。
- 独立仓库添加最小 `.gitignore` 和 GitHub Actions：Go 1.22.x / stable 矩阵执行格式检查、`go test -race -count=1 ./...` 和 `go vet ./...`。
- 修正 SDK README 中的暂定路径说明，以及跨仓库的示例、协议文档链接。
- 更新服务端 README、中英文官网 SDK 指南、示例 README；历史策略、验收证据不修改。
- 删除 Rust CI 中已失效的 `sdk/go/**` 忽略路径。

## 迁移与版本

1. 复制后逐文件校验摘要，确认所有 Go 源码、测试、模块文件完全一致。
2. 在独立目录运行测试及静态检查；用临时 workspace 验证服务端示例。
3. 初始化独立 Git 仓库，关联 `git@github.com:Bin-hy/yufu-sdk-go.git`，提交并推送到空远端的 `main`。不创建版本标签，使用 Go 根据提交生成的伪版本。
4. 服务端示例依赖真实远端伪版本，重新进行无 replace/workspace 的下载、测试与检查。
5. 全部通过后才删除服务端 `sdk/go`；服务端改动保留在工作区供审阅。

## 授权

用户已明确要求后续文档一起整理并直接完成迁移、测试和删除，按此执行，不再逐份审批。
