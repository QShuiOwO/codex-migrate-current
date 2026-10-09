# Codex Migrate Current

[English](README.md) · [下载](https://github.com/QShuiOwO/codex-migrate-current/releases) · [验证记录](docs/current-validation.md)

这是基于 [ChenglongLi777/codex-migrate](https://github.com/ChenglongLi777/codex-migrate) 的社区 Fork，用于迁移 **ChatGPT Desktop 中本地 Codex 代码对话**。上游基线为 v1.0.8、提交 `37f5300c15447921baf3b98d8ef09d07a3a60008`；保留原仓库提交历史、署名和 MIT 许可证。本项目与 OpenAI 不存在隶属或官方背书关系。

当前版本 **1.1.0-current.2** 为 **Windows x64 预发布版**。2026-10-08 验证的适配目标是 ChatGPT Desktop MSIX `OpenAI.Codex 26.1002.7124.0`，配套原生运行时为 `codex-cli 0.162.0-alpha.2`。不支持迁移 Chat/Work 云端账号历史。

## 更新内容

- 识别新版分页历史和消息完成事件。
- 自动补齐分叉对话祖先，重算父历史改写后的字节边界。
- 通过原生 App Server 重建项目、多根工作区、对话名称和归档状态。
- 刷新受影响的历史显示缓存，遍历全部分页并校验已完成消息。
- 备份 SQLite 数据库、rollout 和索引；原生注册失败时自动回滚。
- 保留 legacy 历史，HTML 导出包含分叉祖先消息。

命令行和中英文 GUI 共用更新后的迁移引擎。完整细节见 [CURRENT-VERSION.md](CURRENT-VERSION.md)。

## 下载安装

从 [本 Fork 的 Releases](https://github.com/QShuiOwO/codex-migrate-current/releases) 下载 `Codex-Migrate-Current-1.1.0-current.2-Windows-x64.zip` 及对应 `.sha256` 文件，解压后：

- `codex-migrate-gui.exe`：图形界面。
- `codex-migrate.exe`：命令行。
- `SHA256SUMS.txt`：程序校验值。
- `LICENSE`、`THIRD_PARTY_NOTICES.txt` 和文档：许可证、署名和使用说明。

Release 程序由 GitHub Actions 使用 Windows MSVC 构建，尚未代码签名，需要 [Microsoft Visual C++ v14 x64 运行库](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)。若提示缺少 `VCRUNTIME140.dll`，安装微软提供的 x64 Redistributable。运行前核对 ZIP 的 SHA-256：

```powershell
(Get-FileHash .\Codex-Migrate-Current-1.1.0-current.2-Windows-x64.zip -Algorithm SHA256).Hash.ToLower()
```

上游的 macOS/Linux 源码支持仍保留；本次适配没有这些平台的同等集成验证，也不提供对应发布包。

## 使用流程

1. 备份源 `.codex`，备份包含对话和配置，应妥善保管。
2. 实际导入或回滚前，完全退出 ChatGPT Desktop 和所有 Codex CLI 会话。
3. 扫描备份，选择对话，映射全部工作区根，审阅 dry-run 计划。分叉祖先可能自动加入选择。
4. 执行导入，检查结果后再继续聊天，保留工具输出的事务 ID。

工具接受 `.codex`、`Codex_backup` 和上游旧版精简 `Codex` 目录，不会复制项目源码。分页历史必须使用与桌面应用配套的 `codex.exe`。程序自动寻找 `%LOCALAPPDATA%\OpenAI\Codex\bin\*\codex.exe`；必要时通过 `CODEX_MIGRATE_CODEX_BIN` 显式指定。

```powershell
.\codex-migrate.exe scan 'D:\Backup\Codex_backup'
.\codex-migrate.exe import 'D:\Backup\Codex_backup' `
  --thread '实际对话UUID' --map 'C:\旧项目=D:\新项目' --dry-run

# 审阅计划后，去掉 --dry-run 执行实际迁移。
# 多根工作区为每个根增加 --map。
.\codex-migrate.exe rollback '实际事务ID'
```

`--history-only '旧cwd'` 创建历史占位目录，不注册实际项目。`--codex-home` 指定目标历史目录；若存在 `CODEX_SQLITE_HOME`，还必须核对它，因为该变量独立指定数据库目录。先试迁移时，将两者都设为隔离目录。全部命令可运行 `codex-migrate --help` 查看。

## 验证与限制

针对上述运行时，已通过 **39 项 Rust 测试和 16 项合成数据集成场景**，格式检查和 Clippy 也通过。新增 GUI 导出/选项/预览/原生导入合成流程通过。MSVC 发布包会在发布前下载复核，最终二进制结果见 Release 说明；没有在真实账号中完整点击执行 GUI 导入。详情见 [验证记录](docs/current-validation.md)。

云端账号历史、附件实体、`thread_attachments`、生成物、其他系统和未来运行时均不在本次验证范围。回滚恢复的是迁移前快照，可能覆盖之后新增的对话或修改；应在继续聊天前验收，晚些时候回滚前另做当前备份。

## 从源码构建

```powershell
git clone https://github.com/QShuiOwO/codex-migrate-current.git
cd codex-migrate-current
cargo test --locked --all-targets --features gui
cargo build --locked --release --features gui --bins
```

验证使用 Rust 1.99.0；Windows 还需要兼容的 C/C++ 工具链。可选的运行时测试使用 Python 3.11+，仅生成合成数据：

```powershell
python .\tests\current_runtime.py --codex-bin 'C:\实际运行时\codex.exe' `
  --migrate-bin "$PWD\target\release\codex-migrate.exe"
```

Actions 负责 Windows 测试、第三方许可收集及打包。版本标签先创建预发布草稿，完成检查后通过工作流的 `publish` 输入发布。维护流程见 [docs/publishing.md](docs/publishing.md)。

## 许可证与来源

[MIT](LICENSE)，保留原声明 `Copyright (c) 2026 codex-migrate contributors`；本 Fork 的修改同样按 MIT 发布。来源见 [UPSTREAM.json](UPSTREAM.json)，二进制依赖和字体许可见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

问题请提交至 [本 Fork 的 Issues](https://github.com/QShuiOwO/codex-migrate-current/issues)。报告前删除私密对话、凭据和路径，不要上传完整 `.codex`。其他说明见 [CONTRIBUTING.md](CONTRIBUTING.md)、[SECURITY.md](SECURITY.md) 和 [TRADEMARKS.md](TRADEMARKS.md)。

## 导入失败日志（current.2）

失败后自动展开详细日志，可点击“复制日志”或“保存日志”。预览/导入的完整日志自动保存在 `%LOCALAPPDATA%\codex-migrate\logs\`，界面显示文件路径；回滚不会删除日志。日志包含实际运行时、数据根、目录映射、失败步骤、完整错误链和回滚结果。

绑定项目时，为 GUI 列出的额外目录指定本机文件夹；只选主项目目录可能不足。缺失映射会在预览阶段指出具体路径。详见 [当前版本说明](CURRENT-VERSION.md#current2-导入修复与失败日志)。
