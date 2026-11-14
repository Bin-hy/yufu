# Go SDK 迁移 Checklist

- [x] AC1：所有 SDK 原文件完整迁移，Go 实现和测试摘要保持一致。
- [x] AC1：独立 SDK 的格式检查、race 测试和 vet 通过。
- [x] AC1：验证成功后服务端 `sdk/go` 已删除。
- [x] AC2：服务端示例使用真实独立版本，无永久本地 replace。
- [x] AC2：示例 race 契约测试及 vet 通过，真实联调测试可编译。
- [x] AC3：当前 README 和中英文指南不再指向已删除的实现路径；官网检查通过。
- [x] AC4：许可证已复制，CI 覆盖最低 Go 版本和 stable。
- [x] AC5：独立仓库 origin 正确，远端 main 与本地提交一致。
- [x] AC5：空白临时消费项目可通过远端下载 SDK，导入并编译；记录实际伪版本。

## 实际验证记录

2026-11-14 验收完成。

- 原服务端基线：`9742e68ec7f137b06627993e7a98781b10b5273e`。
- 迁移目标：`/Users/binhy/Binhy-Projects/yufu-sdk-go`；独立仓库提交：`877b814b66a1d9b5dbe0a000d3dfdfb3a54f1655`。
- 初次复制时校验全部 17 个原文件 SHA-256；删除前再次确认原文件集合未变化、原文件摘要未变化，除有意更新的 README 外全部 16 个 Go/模块文件与目标逐字节一致。许可证逐字节一致。
- 独立 SDK：Go 1.26.2 `go test -race -count=1 ./...`、`go vet ./...` 和 gofmt 检查通过。
- 最低版本：Go 1.22.12 的 race 测试与 vet 通过。当前 macOS 无法运行旧 Go 内部链接器生成的测试程序（missing LC_UUID），因此本地测试增加 `-ldflags=-linkmode=external`；Linux CI 无需此参数。
- [GitHub CI](https://github.com/Bin-hy/yufu-sdk-go/actions/runs/43562346208)：completed / success，Go 1.22.x 与 stable 矩阵通过。
- 服务端示例：先通过临时 workspace 验证迁移副本，再在 `GOWORK=off` 且无 replace 条件下，完成 `go mod tidy`、`go test -race -count=1 ./...`、`go vet ./...` 和 `go test -tags=integration -c`。
- 独立消费项目 `/tmp/yufu-sdk-migration-consumer`：`go get github.com/Bin-hy/yufu-sdk-go@latest` 成功；导入 SDK 后 `go run .` 输出 `0.1.0`。
- 真实远端依赖版本：`v0.0.0-20261114053601-877b814b66a1`。本次未创建 SemVer 标签，Go 根据 main 提交解析出伪版本；SDK 内部 Version 保持 `0.1.0`。
- 官网：直接执行已安装的 `website/node_modules/.bin/astro check`，0 errors / 0 warnings / 0 hints。`pnpm check:website` 包装入口尝试重新安装依赖且被环境限制中断，故使用相同底层检查命令完成验证。
- 原 `sdk/go` 已在上述验证完成后删除；历史设计、验收证据保留。
- 本次未启动真实 Docker / issuer 联调；仅确认现有真实联调入口编译通过。现有联调脚本和服务端基础设施未迁移。
- SDK 独立仓库已提交并推送；服务端仓库迁移改动保留在工作区供审阅。
