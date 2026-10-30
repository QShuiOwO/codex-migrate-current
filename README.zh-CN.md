# Codex Migrate Current

[English](README.md) · [下载](https://github.com/QShuiOwO/codex-migrate-current/releases) · [Ubuntu 指南](docs/ubuntu-cli.md) · [版本记录](CHANGELOG.md)

基于 [ChenglongLi777/codex-migrate](https://github.com/ChenglongLi777/codex-migrate) 的社区 Fork，用于在 **Windows ChatGPT Desktop 与 Ubuntu Codex CLI** 之间迁移本地 Codex 代码对话。两端共用源码、备份格式和版本号，也支持同平台迁移。不迁移普通 ChatGPT / Chat / Work 云端账号历史。

当前版本：**1.1.0-current.7**。上游基线为 v1.0.8、提交 `37f5300c15447921baf3b98d8ef09d07a3a60008`；保留原仓库历史、署名及 MIT 许可证。本项目与 OpenAI 不存在隶属或官方背书关系。

## 下载与启动

在 [Releases](https://github.com/QShuiOwO/codex-migrate-current/releases) 选择相同版本的两个平台包；尚未发布的版本可从对应 PR 的 Actions 获取构建产物，也可从源码构建。

| 平台 | current.7 发布文件 | 解压后启动 GUI |
| --- | --- | --- |
| Windows x64 | `Codex-Migrate-Current-1.1.0-current.7-Windows-x64.zip` | `codex-migrate-gui.exe` |
| Ubuntu 24.04 x64 | `Codex-Migrate-Current-1.1.0-current.7-Ubuntu-24.04-x64.tar.gz` | `./codex-migrate-gui` |

每个包包含 GUI、CLI、说明、许可和程序校验值，另附压缩包 `.sha256`。Windows MSVC ZIP 需要 [Microsoft Visual C++ v14 x64 运行库](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)。Ubuntu GUI 使用系统 X11/Wayland 和文件选择器；命令行不需要桌面环境。

```powershell
# Windows：核对压缩包校验值
(Get-FileHash .\Codex-Migrate-Current-1.1.0-current.7-Windows-x64.zip -Algorithm SHA256).Hash.ToLower()
```

```bash
# Ubuntu：核对、解压和启动
sha256sum -c Codex-Migrate-Current-1.1.0-current.7-Ubuntu-24.04-x64.tar.gz.sha256
tar -xzf Codex-Migrate-Current-1.1.0-current.7-Ubuntu-24.04-x64.tar.gz
cd Codex-Migrate-Current-1.1.0-current.7-Ubuntu-24.04-x64
./codex-migrate-gui
```

Windows 单文件独立 EXE 构建流程仍保留在 `scripts/build-standalone.ps1`。它是单独的可选打包流程，只有在相同版本的 Release 中实际附带时才作为该版本下载项；历史 current.6 EXE 不包含 current.7 的跨平台改动。

## Windows → Ubuntu

1. Windows GUI 点击“导出对话”，勾选会话并导出 `Codex_backup`。必要的分叉祖先会自动包含，默认不选中任何对话。
2. 把备份和所需项目文件分别复制到 Ubuntu。正式导入前退出使用目标历史库的 Desktop / Codex CLI 会话。
3. Ubuntu GUI 选择“导入后使用 → Codex CLI”（Linux 默认），读取备份。历史库通常保持 `~/.codex`。
4. 为所选项目选择 Ubuntu 工作文件夹，并映射额外工作区/权限目录；预览后导入。工作文件夹与 `.codex` 历史库是两个不同位置。
5. 在该工作文件夹打开终端，运行 `codex resume`，选择迁移会话继续。沿用本机 Codex 登录和模型配置。

支持 `backup`、`Codex_backup`、`.codex` 及旧精简 `Codex` 目录。CLI 模式将明确选中的归档/非交互记录转为活动交互会话，保留自动补齐祖先的归档状态。目标服务商按本机配置适配，原始元数据保留用于重复和往返导入。

```bash
./codex-migrate scan /media/usb/backup
./codex-migrate import /media/usb/backup --target cli \
  --map 'C:\Users\Alice\Project=/home/alice/Project' --dry-run
# 检查计划后用同样参数去掉 --dry-run，再运行：
cd /home/alice/Project
codex resume
```

## Ubuntu → Windows

Ubuntu GUI 可按项目或会话选择性导出；CLI 可重复使用 `--thread`：

```bash
./codex-migrate export "$HOME/.codex" --output-parent /media/usb/export \
  --thread '会话UUID'
```

复制 `Codex_backup` 到 Windows，使用 GUI 的 Desktop 模式，或：

```powershell
.\codex-migrate.exe import 'E:\export\Codex_backup' --target desktop `
  --map '/home/alice/Project=D:\Projects\Project' --dry-run
# 检查计划后去掉 --dry-run，完成后重新打开 Desktop。
```

建议两端都更新到 current.7，特别是对相同 UUID 进行往返合并时。**操作系统与导入目标独立**：Windows 也可选择 `--target cli`。Desktop 模式保留备份中的归档状态，并注册项目和多根工作区。Linux 默认 `cli`，Windows/macOS 默认 `desktop`；macOS 尚无本次同等验证。

## 功能与数据范围

- 共用迁移引擎，支持 legacy 与分页历史、分叉祖先、名称、项目、多根工作区及 HTML 导出。
- Windows 源路径支持盘符大小写、UNC 和扩展路径；Unix 源路径保持大小写区分。
- 预览不创建工作目录；正式导入时创建缺失目录，失败自动恢复数据库、rollout 和索引快照。
- 选择性备份只包含所选会话、必要祖先、索引和迁移元数据。不传 `export --thread` 时是完整目录备份，可能包含配置等敏感数据。
- 项目源码、附件实体、`thread_attachments` 和生成文件需要另外转移。对话正文中的旧路径不改写。

`--codex-home` 指定目标历史库；`CODEX_SQLITE_HOME` 独立指定数据库目录。使用自定义位置时，迁移与启动 Codex 必须使用相同设置。Windows 自动寻找 Desktop 运行时，Ubuntu 支持 PATH 和 npm/nvm 安装；特殊路径可设置 `CODEX_MIGRATE_CODEX_BIN`。新版分页历史要求兼容的原生运行时。

导入会输出事务 ID，可使用 `codex-migrate rollback ID` 回滚。回滚会恢复导入前快照，应在继续聊天前验收；晚些时候回滚可能覆盖新增内容。诊断日志在 Windows 的 `%LOCALAPPDATA%\codex-migrate\logs`、Ubuntu 的 `~/.local/share/codex-migrate/logs`，也可用 `CODEX_MIGRATE_LOG_DIR` 指定。

## 构建、测试与维护

Rust 依赖由 `Cargo.toml` / `Cargo.lock` 统一管理，公共源码放在 `src/`，平台打包脚本放在 `scripts/`。功能分支经 Windows、Ubuntu CI 检查后合并回 `main`，不长期维护平台分叉。

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --features gui -- -D warnings
cargo test --locked --all-targets --features gui
cargo build --locked --release --features gui --bins
```

CI 使用 Rust 1.99.0；Ubuntu 本机验证使用 1.99.1 / Codex CLI 0.166.0。Ubuntu 构建依赖、原生合成测试和终端验证见 [Ubuntu 指南](docs/ubuntu-cli.md) 与 [验证记录](docs/ubuntu-validation.md)。Windows 既有 Desktop 26.1023.6621.0 / Codex 0.165.0-alpha.1.2 验证保存在 [历史记录](docs/current-validation.md)；current.7 的 Windows 编译/测试以对应 CI 结果为准，不能代替 Desktop 实机验收。

统一发布流程等待两端构建成功，校验两个平台的版本和 SHA-256 后生成同一预发布草稿；详见 [发布维护流程](docs/publishing.md)。完整适配历史见 [CURRENT-VERSION.md](CURRENT-VERSION.md)。

## 许可证与反馈

[MIT](LICENSE)，保留 `Copyright (c) 2026 codex-migrate contributors`。来源见 [UPSTREAM.json](UPSTREAM.json)，第三方许可见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

问题请提交至 [Issues](https://github.com/QShuiOwO/codex-migrate-current/issues)，删除私密对话、凭据和本机路径后再分享日志，不要上传完整 `.codex`。其他说明见 [CONTRIBUTING.md](CONTRIBUTING.md)、[SECURITY.md](SECURITY.md)、[TRADEMARKS.md](TRADEMARKS.md)。
