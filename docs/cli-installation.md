# 命令行安装与使用

CLI 是个人共享记忆的主要安装入口。发行包包含 `lingshu`、后端、内置 PostgreSQL、stdio MCP 和客户端网页；运行时无需 Rust、Node、Python、Docker、Redis 或 Qdrant。基础聊天仍需用户配置 Ollama 或自备模型服务；无模型时本地记忆与 MCP 仍可使用，检索回退到 SQL。

## 安装

源码实现不代表发行包已发布。下列下载命令只有在新的 CLI release 上传后才可使用；私有仓库需要相应访问权限，也可以用 `gh release download` 下载归档后离线安装。现有桌面安装包不包含新 CLI。

当前仓库为私有仓库。有访问权限的用户安装 GitHub CLI 并执行 `gh auth login` 后，可在线安装：

```sh
gh release download v1.6.2 --repo LingeringDream/LingShu --pattern install.sh --dir /tmp --clobber
sh /tmp/install.sh --version v1.6.2
```

Windows 使用 PowerShell：

```powershell
gh release download v1.6.2 --repo LingeringDream/LingShu --pattern install.ps1 --dir "$env:TEMP" --clobber
& "$env:TEMP\install.ps1" --version v1.6.2
```

登录过 `gh` 时，安装器通过它下载归档及校验文件；自定义发行镜像仍使用 HTTP 下载。下列无需登录的 curl / Invoke-WebRequest 命令适用于公开可访问的发行地址。

macOS（Apple Silicon / Intel）和 Linux x64（正式发布基准 glibc 2.35+）：

```sh
curl -fL https://github.com/LingeringDream/LingShu/releases/latest/download/install.sh -o /tmp/lingshu-install.sh
sh /tmp/lingshu-install.sh
```

默认程序目录为 `~/.local/share/lingshu-cli`，稳定入口为 `~/.local/bin/lingshu`。若该目录未在 PATH 中，将它加入 PATH；也可直接运行 `~/.local/bin/lingshu`。安装脚本不编辑 shell 配置，不使用 sudo。

Windows x64（PowerShell）：

```powershell
Invoke-WebRequest https://github.com/LingeringDream/LingShu/releases/latest/download/install.ps1 -OutFile "$env:TEMP\lingshu-install.ps1"
& "$env:TEMP\lingshu-install.ps1"
```

程序安装到 `%LOCALAPPDATA%\LingShu\cli`，命令放在 `%LOCALAPPDATA%\LingShu\bin` 并加入用户 PATH；安装后打开新终端。

指定版本使用 `--version vX.Y.Z`。安装器检测平台，下载归档及 SHA-256 校验文件，先验证再切换版本。Linux 包还记录实际最低 glibc 版本，安装器在切换前检查；自行在较新发行版构建的包可能要求更高版本。`LINGSHU_DOWNLOAD_BASE_URL` 可指定同样目录结构的发行镜像；校验值应来自可信发布源。

离线安装（校验文件必须与归档同目录，名称为 `归档文件名.sha256`）：

```sh
sh scripts/install.sh --archive /path/lingshu-x86_64-unknown-linux-gnu.tar.gz
```

```powershell
& .\scripts\install.ps1 --archive C:\Downloads\lingshu-x86_64-pc-windows-msvc.zip
```

两种安装器都支持绝对路径 `--prefix DIR` 与 `--bin-dir DIR`。程序路径可包含空格；配置接入使用稳定入口，升级不需要重写 MCP 配置。

## 日常命令

```sh
lingshu start                 # 后台运行；首次初始化数据库可需两分钟
lingshu status               # 未运行或未就绪时退出码为 1
lingshu status --json
lingshu connect cursor
lingshu connect claude
lingshu connect codex
lingshu open                 # 启动服务并打开网页控制台
lingshu open --no-browser    # 只输出控制台地址
lingshu logs --lines 50
lingshu stop                 # 优雅关闭后端及内置 PostgreSQL
lingshu update               # 更新程序，保留数据；原先运行则自动恢复
lingshu update --archive /path/new-bundle.tar.gz
lingshu uninstall            # 删除 CLI 程序，保留记忆和配置
lingshu uninstall --purge    # 明确删除 CLI 数据，无法恢复
```

`connect` 保留其他 MCP 服务、模型设置和已有注释（Codex TOML），修改前生成相邻备份。格式损坏时拒绝覆盖；可用 `--config FILE` 指定配置文件，用 `--print` 只输出接入片段。重启 AI 工具使配置生效。

生成的 MCP 配置调用 `lingshu mcp`，按需启动后台服务；stdout 仅输出 JSON-RPC，启动和诊断写 stderr。也可直接配置此命令。`LINGSHU_BASE_URL` 显式指定已有服务时，MCP 桥接不启动托管后端，沿用原有 `LINGSHU_TOKEN` / 本地会话鉴权。

这是一项用户后台服务，不自动注册开机启动。关闭启动终端后服务继续运行；用 `stop` 停止。控制端口只绑定 loopback，停止请求需每次启动生成的私有凭据。CLI 不凭过期 PID 杀进程，也不接管已占用端口的其他服务。

Windows 卸载在 CLI 退出后完成，需先关闭使用该 MCP 的 AI 工具以释放程序文件；日志路径由命令输出。卸载不删除其他应用的 MCP 配置和备份，用户可自行移除 `lingshu-memory` 条目。

## 数据与端口

| 平台 | 默认 CLI 数据目录 |
| --- | --- |
| Linux | `$XDG_DATA_HOME/lingshu`，默认 `~/.local/share/lingshu` |
| macOS | `~/Library/Application Support/LingShu/cli` |
| Windows | `%LOCALAPPDATA%\LingShu\cli-data` |

用 `LINGSHU_HOME` 或全局 `--home DIR` 指定独立目录，数据目录必须在程序安装目录之外；不能将文件系统根目录或整个用户主目录作为数据目录。配置、随机密钥、日志与数据库在此目录中，升级只切换程序，旧程序版本保留以便排查；不会迁移已有桌面客户端数据。密钥不可泄露或随意改写，尤其会影响已加密的集成令牌。

默认 HTTP 端口为 8080，数据库端口为 55433，与桌面数据库隔离。若 HTTP 端口被占用，用 `lingshu start --port 8081`；运行后应先停止再改端口。多个 CLI 实例需独立 `--home`，并分别在 `config.json` 设置不冲突的 HTTP / 数据库端口。

开发者使用已有 PostgreSQL 时，在**首次初始化此目录前**设置 `LINGSHU_DATABASE_URL`，该地址保存于私有配置中，并禁用内置数据库。普通安装无需该变量。退出或升级不会删除外部数据库，即使执行 `--purge` 也只删除 CLI 本地目录。

## 构建与发布

维护者安装 Rust 1.88、Node 22、Python 3.12，在仓库根目录运行：

```sh
cd frontend && npm ci && cd ..
python scripts/package-cli.py
```

产物为 `dist/cli/lingshu-<target>.tar.gz`（Windows 为 zip）及 `.sha256`。支持 Linux x64、macOS arm64/x64、Windows x64；每个目标在原生机器构建内置数据库。`--no-build --binaries DIR --web DIR` 可打包已有二进制和客户端网页；`--no-embedded-db` 只供开发，需外部 PostgreSQL。

标签发布工作流在四个平台构建并跑安装、记忆、MCP、升级与卸载回归，全部 CLI 目标通过后上传 CLI 包和安装脚本并发布。桌面包独立上传，不阻塞 CLI 发布。发布需要推送新的版本标签；仅构建源码不会使公网安装命令可用。
