# Ubuntu 24.04 与 Windows 双向迁移

`1.1.0-current.8` 统一版本支持将 Windows ChatGPT Desktop 的**本地 Codex 对话**迁移到 Ubuntu 24.04 Codex CLI，也支持将 Ubuntu 的会话导出为同一备份格式，再由 Windows 导入。普通 ChatGPT 云端聊天导出不属于此格式。

## Ubuntu 安装和启动

先安装并登录 Codex CLI，确认 `codex --version` 可用。新版分页备份需要支持分页历史的运行时；本次验证使用 Ubuntu 24.04 x64、Codex CLI 0.166.0。旧运行时无法读取该格式时，导入会失败并自动回滚。

已构建的 Ubuntu 压缩包解压后，运行 `./codex-migrate-gui`。也可运行 `./codex-migrate --help`。GUI 使用系统 X11/Wayland 和文件选择器；Ubuntu Desktop 通常已具备这些环境，纯服务器可只使用 CLI。

从源码编译：

```bash
sudo apt update
sudo apt install build-essential pkg-config libx11-dev libxkbcommon-dev libwayland-dev libgl1-mesa-dev fonts-noto-cjk xdg-desktop-portal-gtk
# 按 https://rustup.rs 的指引安装 Rust；正常位置为 ~/.cargo 和 ~/.rustup。
. "$HOME/.cargo/env"
cargo test --locked --all-targets --features gui
cargo build --locked --release --features gui --bins
./target/release/codex-migrate-gui
```

程序优先使用 PATH 中的 Codex；Ubuntu 从桌面启动时，也会查找 `~/.local/bin`、`~/.npm-global/bin`、`~/.cargo/bin` 和 nvm 安装目录，并识别 npm 包中的原生运行时。特殊安装方式可以指定：

```bash
export CODEX_MIGRATE_CODEX_BIN=/absolute/path/to/codex
```

## Windows → Ubuntu

1. 在 Windows 导出所选对话，将生成的 `Codex_backup` 文件夹复制到 Ubuntu。完整备份和旧精简 `Codex` 备份仍可读取；备份文件夹改名为 `backup` 也可以。
2. 正式导入前退出使用目标历史库的 Codex / Desktop 会话。
3. 打开 Ubuntu GUI，选择“导入后使用 → Codex CLI”（Linux 默认）。来源选择备份，目标历史库通常保持 `~/.codex`。
4. 勾选会话，为每个旧项目选择**Ubuntu 的工作文件夹**；额外工作区、权限目录也要映射。工作文件夹与 `~/.codex` 历史库是两个不同的位置。
5. 预览并导入。缺失的目标目录会在正式导入时创建；项目代码和附件文件需要另外复制。
6. 在所选工作文件夹打开终端，运行 `codex resume`，选择会话并回车。登录和模型配置沿用目标机器。

CLI 等价示例：

```bash
./codex-migrate scan /media/usb/backup
./codex-migrate import /media/usb/backup --target cli \
  --thread '会话UUID' \
  --map 'C:\Users\Alice\Project=/home/alice/Project' \
  --map 'D:\Assets=/home/alice/Assets' --dry-run
# 审阅结果后用同样参数去掉 --dry-run 执行。
cd /home/alice/Project
codex resume
# 也可直接恢复指定会话：
codex resume '会话UUID'
```

CLI 模式将明确选中的会话注册为交互会话，取消其归档状态，并采用目标工作目录配置中的模型服务商，确保匹配 CLI 的目录、来源、服务商和归档筛选。自动补齐的分叉祖先保留归档状态；它们的历史仍可供子会话读取。原始来源和服务商元数据保存在 rollout 中，用于重复导入和往返比较。不会修改对话正文中的旧路径。

Windows 源路径按 Windows 规则匹配（包括盘符大小写、`\\?\` 和 UNC）；Unix 源路径始终区分大小写。目标目录的符号链接会解析到实际位置，匹配 CLI 的工作目录过滤。

若使用 `--codex-home /other/history`，启动 CLI 时也需设置 `CODEX_HOME=/other/history`。如使用 `CODEX_SQLITE_HOME`，迁移和运行 Codex 时两者必须一致；它是独立的数据库目录设置。测试时应将两个目录都指向隔离位置。`--history-only` 仍为历史占位模式，不会把会话绑定到真实项目。

## Ubuntu → Windows

GUI 的“导出对话”可勾选会话并保存到 `Codex_backup`。CLI：

```bash
./codex-migrate export "$HOME/.codex" --output-parent /media/usb/export \
  --thread '会话UUID'
```

不传 `--thread` 是完整备份；选择性导出只包含所选会话及必要祖先、索引和迁移元数据。若数据库单独存放在 `CODEX_SQLITE_HOME`，优先用 GUI 或选择性导出。将备份复制到 Windows，选 Desktop 模式并映射到 Windows 工作目录：

```powershell
.\codex-migrate.exe import 'E:\export\Codex_backup' --target desktop `
  --map '/home/alice/Project=D:\Projects\Project' `
  --map '/home/alice/Assets=D:\Assets' --dry-run
```

Windows CLI 目标可显式使用 `--target cli`。Desktop 模式保留备份中的归档状态并注册原生项目；Linux 默认 `cli`，Windows/macOS 默认 `desktop`。建议两端都使用 current.8，尤其是相同 UUID 的重复/往返导入。旧版 current.6 没有 `--target` 参数，虽然能读取选择性备份，但不识别新增的元数据比较规则。

## 验证与限制

本次环境和通过数量见 [Ubuntu 验证记录](ubuntu-validation.md)。

Rust 单元/集成测试涵盖跨系统路径、元数据转换、预览和回滚。原生测试用隔离目录与合成对话，不读取真实历史、不调用在线模型：

```bash
python3 tests/current_runtime.py --codex-bin "$(command -v codex)" --migrate-bin target/release/codex-migrate
python3 tests/cli_runtime.py --codex-bin "$(command -v codex)" --migrate-bin target/release/codex-migrate
python3 tests/inline_fork_runtime.py --codex-bin "$(command -v codex)" --migrate-bin target/release/codex-migrate
python3 tests/cli_tui.py --codex-bin "$(command -v codex)"
```

`current_runtime.py` 验证 Desktop 引擎，`cli_runtime.py` 验证 Windows 格式备份导入、CLI 筛选/恢复、分页分叉、旧历史、选择性再导出、服务商适配与失败回滚。`inline_fork_runtime.py` 验证包含父会话头的完整分叉记录可独立导出、重新导入并恢复。`cli_tui.py` 在伪终端中实际运行 `codex resume`，选择会话并确认历史和输入框已显示，不提交新消息。Ubuntu 导出后通过 Desktop 引擎回导在 Linux 原生运行时上验证；本次没有在实体 Windows Desktop 上执行新增改动，Windows 构建/回归由 CI 检查。附件实体和项目源码不由迁移工具搬运。

导入结果保留事务 ID，可在继续聊天前运行 `codex-migrate rollback ID`。回滚恢复导入前快照，继续聊天后再回滚可能覆盖新内容。官方 CLI 的恢复入口见 [Codex CLI 文档](https://learn.chatgpt.com/docs/codex/cli)。
