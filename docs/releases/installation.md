# yufu 二进制安装

从 [GitHub Releases](https://github.com/Bin-hy/yufu/releases) 下载与你的操作系统和 CPU 对应的安装包及同版本 `SHA256SUMS`。
安装包内嵌 WebUI，不需要在部署机器上安装 Rust、Node.js 或 pnpm。运行服务仍需要 PostgreSQL 和可用的可信 NTP 来源。

## 选择安装包

| 平台 | 文件名（以 v1.2.3 为例） | 构建环境 |
| --- | --- | --- |
| Linux x86_64 | `yufu-v1.2.3-x86_64-unknown-linux-gnu.tar.gz` | Ubuntu 22.04，GNU/glibc 动态运行环境 |
| Windows x86_64 | `yufu-v1.2.3-x86_64-pc-windows-msvc.zip` | Windows Server 2022，MSVC |
| macOS Intel | `yufu-v1.2.3-x86_64-apple-darwin.tar.gz` | macOS 15 Intel |
| macOS Apple Silicon | `yufu-v1.2.3-aarch64-apple-darwin.tar.gz` | macOS 15 ARM64 |

Linux 包不是 musl 静态包，Alpine 不在本次支持范围；动态依赖以该版本 Actions 构建日志为准。
macOS 包的构建最低系统版本设为 15.0，其他 macOS 版本未验证。
Windows 目标系统需具备相应的 Microsoft Visual C++ 运行库；安装包不包含该运行库。
当前包没有代码签名或 Apple 公证，操作系统可能显示未验证发布者提示；通过可信发布来源核对校验和后，按组织的软件准入策略安装。

## 校验下载

将安装包和 `SHA256SUMS` 放在同一目录。以下命令针对已经下载的包核对摘要：

Linux：

```sh
sha256sum --check --ignore-missing SHA256SUMS
```

macOS：

```sh
shasum -a 256 --check --ignore-missing SHA256SUMS
```

Windows PowerShell（将文件名替换为所下载版本）：

```powershell
$archive = 'yufu-v1.2.3-x86_64-pc-windows-msvc.zip'
$entry = Get-Content SHA256SUMS | Where-Object { $_.EndsWith("  $archive") }
if (@($entry).Count -ne 1) { throw 'Missing or ambiguous checksum entry' }
$expected = ($entry -split '  ', 2)[0]
$actual = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
if ($actual -ne $expected) { throw 'SHA-256 mismatch' }
```

确认实际安装包有匹配的校验记录并通过检查，再进行解包。

## 解包与配置

Linux/macOS：

```sh
tar -xzf yufu-v1.2.3-x86_64-unknown-linux-gnu.tar.gz
cd yufu-v1.2.3-x86_64-unknown-linux-gnu
cp yufu.example.toml yufu.toml
```

macOS 使用对应的 `apple-darwin` 包名和目录。Windows PowerShell：

```powershell
Expand-Archive yufu-v1.2.3-x86_64-pc-windows-msvc.zip -DestinationPath .
Set-Location yufu-v1.2.3-x86_64-pc-windows-msvc
Copy-Item yufu.example.toml yufu.toml
```

编辑 `yufu.toml`，至少完成：

- `[database].url`：独立 PostgreSQL 数据库连接，使用部署环境的凭据。
- `[auth.super_admin]`：管理员用户名与强密码，取消示例中对应字段的注释。
- `[oidc].issuer`：固定的公开服务地址，生产使用 HTTPS。
- `[oidc].rsa_private_key_path`：签名私钥路径。生产环境**不会**自动生成，启动前必须存在可读的
  PKCS#8 PEM（文件头 `-----BEGIN PRIVATE KEY-----`）；仅在显式本机开发模式
  （`--allow-insecure-loopback-cookies`）下缺失才会生成。
- `[auth].origins`：允许的完整浏览器来源，包含协议、主机和端口。
- `[security_clock]`：网络可达的可信 NTP 来源及所需一致来源数量。

默认私钥路径是 `conf/oidc_rsa_private.local.pem`。安装包没有 `conf` 目录，首次启动前须创建它，
或将配置改为已有可写目录中的路径。生成生产私钥：

```sh
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out oidc_rsa_private.pem
chmod 600 oidc_rsa_private.pem
```

配置 `/var/lib/yufu/oidc_rsa_private.pem` 等绝对路径时，须确保运行服务的账户可读。
备份数据库、私钥、配置与不能重新获取的应用凭据。不要将不同 issuer 的配置交叉用于已有数据库。

完整参考：[快速开始](https://cobotusr.pages.dev/getting-started/) · [配置项](https://cobotusr.pages.dev/reference/configuration/) · [生产部署](https://cobotusr.pages.dev/operations/deployment/)。

## 启动与首次登录

检查安装版本（无需数据库）：

```sh
./yufu --version
./yufu --help
```

本机 HTTP 开发：

```sh
mkdir -p conf
./yufu --config yufu.toml --allow-insecure-loopback-cookies
```

此参数要求 listener 和 issuer 都为 loopback。默认访问 `http://127.0.0.1:4000`，使用配置中的管理员账户登录。
生产部署使用 HTTPS 和默认 Secure Cookie，不添加本机 HTTP 参数：

```sh
./yufu --config /etc/yufu/yufu.toml
```

Windows 在 PowerShell 中以 `.\yufu.exe` 替换 `./yufu`，可使用 `New-Item -ItemType Directory -Force conf` 创建目录。
认证时钟异常会阻止服务就绪；可先运行 `--config yufu.toml --check-security-clock` 独立诊断。
运行后检查 `/api/v1/health/ready`，并通过 `/.well-known/openid-configuration` 验证协议发现。

## Linux systemd

Linux 包附带 `yufu.service`，其路径约定为：

- 可执行文件：`/usr/bin/yufu`。
- 配置：`/etc/yufu/yufu.toml`。
- 工作与状态目录：`/var/lib/yufu`，由 systemd 的 `StateDirectory=yufu` 管理。
- 服务账户：`DynamicUser=yes`，不要依赖固定 UID。

将文件安装到对应位置，配置数据库、HTTPS 入口和文件权限后，再启用服务。
签名私钥建议配置为 `/var/lib/yufu/oidc_rsa_private.pem`，避免依赖不存在的相对 `conf` 目录；
已有私钥应按 DynamicUser/StateDirectory 的访问方式配置权限，不要随意放宽私钥权限。

```sh
sudo install -m 0755 yufu /usr/bin/yufu
sudo install -d -m 0755 /etc/yufu
# 安装已经编辑完成的配置，不要直接启用未配置的示例
sudo install -m 0640 yufu.toml /etc/yufu/yufu.toml
sudo install -m 0644 yufu.service /etc/systemd/system/yufu.service
```

启用前为配置设置服务可读的访问方式，例如在该服务的 systemd drop-in 中设置 `SupplementaryGroups=yufu-config`，
创建对应系统组并把配置文件组设置为该组，保持 `0640`；动态用户通过该补充组读取配置。
准备好私钥存储和访问权限后执行：

```sh
sudo systemctl daemon-reload
sudo systemctl enable --now yufu
sudo systemctl status yufu
```

Windows/macOS 本次提供手动运行的可执行文件；不附安装器或系统服务自动注册。

## 升级

先备份数据库、私钥和配置，查阅该版本说明，再在维护窗口替换可执行文件。
首次启动会执行版本化迁移；回退不能只替换旧二进制，应按备份与恢复方案处理数据库版本。
包中的 `BUILD-INFO.json` 记录来源 tag、提交、target 和构建环境，可用于审计与问题定位。
