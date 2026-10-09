# 当前 ChatGPT Desktop 适配版

这是基于 [ChenglongLi777/codex-migrate](https://github.com/ChenglongLi777/codex-migrate) 的 [社区 Fork](https://github.com/QShuiOwO/codex-migrate-current)。基线为上游 `37f5300c15447921baf3b98d8ef09d07a3a60008`、v1.0.8；本 Fork 版本为 `1.1.0-current.2`，不是上游发布版，也不是 OpenAI 官方工具。

2026-10-08 的适配目标是这台 Windows 电脑的 ChatGPT Desktop `OpenAI.Codex 26.1002.7124.0`，对应原生运行时 `codex-cli 0.162.0-alpha.2`。测试使用独立目录中的合成对话，没有迁移或改写真实 `.codex`。

## 可执行程序

Windows x64 发布包从本 Fork 的 Releases 下载，解压后的程序为：

- `codex-migrate.exe`：命令行程序。
- `codex-migrate-gui.exe`：图形界面，沿用原项目的操作流程，迁移引擎已更新。

程序可以自动寻找 `%LOCALAPPDATA%\OpenAI\Codex\bin\*\codex.exe`；也可以显式设置 `CODEX_MIGRATE_CODEX_BIN`。分页历史需要使用与桌面应用匹配的原生运行时。

## current.2 导入修复与失败日志

2026-10-09 修复 GUI 只显示“failed / 已回滚”的问题。现在保留完整错误链，失败自动展开日志，可复制或另存。预览和导入会自动生成独立日志，Windows 默认目录为 `%LOCALAPPDATA%\codex-migrate\logs\`，错误中会显示确切文件位置；`CODEX_MIGRATE_LOG_DIR` 可指定其他日志目录。日志在目标对话目录之外，回滚不会删除它。

绑定项目时，GUI 列出记录中额外的工作目录和权限目录。为每项选择本机文件夹；确实希望合并到主项目目录时可点击“使用项目目录”。主项目的子目录沿用主项目映射，不需要逐项选择。仅恢复历史仍使用占位目录。缺失映射现在会在预览/写入前报错。

Windows 自动发现优先使用 Desktop 自带运行时，再查 PATH；显式的 `CODEX_MIGRATE_CODEX_BIN` 仍具有最高优先级。日志记录程序/运行时版本、运行时位置、数据根、映射、事务、失败步骤、完整错误及原生进程 stderr 尾部。它不主动收集对话正文或凭据，凭据字段相关诊断行会被遮蔽；分享日志前核对其中的本机路径和线程 ID。

目标机器此次失败尚未获得底层日志，不能仅凭版本号确认唯一原因。复测覆盖合成备份经过 GUI 选项构建和真实运行时导入，不等同于真实账号的完整鼠标点击验收。

## 本次修改

| 原有问题 | 现在的处理 |
|---|---|
| 新版 `history_mode=paginated` 和 `item_completed` 元数据未识别 | 识别分页历史、用户消息、结构化 `source` 和新版线程元数据；保留分页模式与原生名称 |
| 只选择分叉子对话可能漏掉父记录 | 自动包含 `history_base` 的祖先，按父记录在前的顺序导入；缺失或循环依赖直接失败 |
| 映射 cwd 后父记录字节长度变化 | 按父记录的新内容重算 `end_byte_offset`，保留独占 ordinal 截止位置 |
| 原生注册失败后直接写数据库会制造“成功”假象 | 对分页历史和新版项目数据库要求原生注册成功；遍历全部历史页并核对已完成消息 ID，失败自动回滚 |
| 新版项目列表来自数据库/API | 使用 `project/import` 和 `thread/metadata/update` 重建项目、多根目录和线程归属；重复导入复用项目 |
| 仅修改 cwd，其他工作区根仍是旧路径 | 映射 `runtime_workspace_roots`、`workspace_roots`、权限中的明确目录字段；额外根未映射则报错 |
| 历史显示缓存与改写后的 JSONL 不一致 | 只失效相关线程及其后代的显示投影，由运行时重建；保留实时历史表 |
| 已归档记录无法直接 resume | 临时通过原生接口解除归档，完成注册/校验后恢复归档 |
| 只备份状态库，回滚不完整 | 对数据根、SQLite 根和 `sqlite/` 中现有 `.sqlite`/`.db` 使用在线备份；跟踪新增数据库、原生移动的 rollout、索引和旧版项目 JSON |
| 新版本缺少 session_index 行被误判损坏 | 分页/内部对话不强制要求每条都存在于旧索引；迁移仍维护兼容索引 |
| HTML 导出遗漏仅存在于显示事件中的消息与分叉祖先 | 解析新版用户/助手完成事件，并按 `history_base` 合并父历史 |

原有 legacy 格式仍可使用；只有旧 schema、旧历史在无法使用运行时时才保留 SQLite 后备模式。当前格式不会静默降级。

## 使用方法

执行实际迁移或回滚前，完全退出 ChatGPT Desktop 和所有 Codex CLI 会话。当前工具会识别 `ChatGPT.exe` 和 `Codex.exe`。备份源目录后，先查看 dry-run 结果；自动补齐的父对话也会出现在预览中。

```powershell
# 若自动发现的运行时不正确，指定与桌面应用一致的 codex.exe。
$env:CODEX_MIGRATE_CODEX_BIN = 'C:\实际安装路径\codex.exe'

# 扫描备份（不会实际导入）
.\codex-migrate.exe scan 'D:\Backup\Codex_backup'

# 查看迁移计划，OLD=NEW 必须是对应的绝对目录。
.\codex-migrate.exe import 'D:\Backup\Codex_backup' `
  --thread '实际对话UUID' --map 'C:\旧项目=D:\新项目' --dry-run

# 审阅计划后，去掉 --dry-run 执行迁移。
# 迁移输出的 transaction id 可用于回滚。
.\codex-migrate.exe rollback '实际TRANSACTION_ID'
```

同一项目有多个工作区根时，给每个旧根提供 `--map`。只需保留历史时使用 `--history-only '旧cwd'`，工具将创建迁移历史目录，且不把它注册成实际项目。

`--codex-home` 指定 JSONL 和配置所在目录；`CODEX_SQLITE_HOME` 指定数据库目录，未设置时随目标 home。若环境中已有这个变量，指定自定义 home 时也必须核对它，避免把测试导入数据库指向正式数据根。

```powershell
# 指定完全隔离的目标，适合先试迁移。
$env:CODEX_SQLITE_HOME = 'D:\MigrationPreview'
.\codex-migrate.exe import 'D:\Backup\Codex_backup' `
  --codex-home 'D:\MigrationPreview' --thread '实际对话UUID' `
  --map 'C:\旧项目=D:\新项目' --dry-run
```

实际需要正式数据根时，应恢复正确的 `CODEX_SQLITE_HOME` 或删除这个临时环境变量。GUI 也遵循这两个数据根设置。

## 验证与构建

回归与运行时集成测试结果记录在 `docs/current-validation.md`。集成测试脚本会在 `test-results/` 下新建目录，从头生成合成记录，只调用历史/项目 API，不调用 `turn/start` 或模型推理。

```powershell
.\scripts\build-current.ps1 -Mode Test -Gui
.\scripts\build-current.ps1 -Mode Release -Gui
python .\tests\current_runtime.py --codex-bin 'C:\实际安装路径\codex.exe' `
  --migrate-bin "$PWD\target\release\codex-migrate.exe"
```

没有工作区便携工具链时，脚本使用系统 Cargo。也可直接运行 `cargo test --features gui` 和 `cargo build --release --features gui`。

## 已知范围

- 适配的是 ChatGPT Desktop 中本地 Codex 代码对话的 `.codex` 存储。Chat/Work 云端账号历史及 `codex-dev.db` 云端目录不能靠导入本地 rollout 迁移，本副本没有实现此类账号迁移。
- 迁移对话历史和项目关系，不自动复制项目源码、附件实体或 generated artifacts，也没有搬运 `thread_attachments` 等关联数据。含附件的备份需要另外保留这些实体与路径；本文不将附件完整性列为通过项目。
- 验证覆盖本机 Windows 版本与原生协议。GUI 已构建和运行逻辑测试；未通过实际桌面界面完成一次正式账号导入。macOS/Linux、其他版本和组织策略配置未做同等集成验证。
- 权限只映射已识别的结构化路径，不改写对话正文中的历史命令、系统说明和路径文本。恢复执行新任务时仍需按新设备环境审核工作区与权限。
- 回滚恢复迁移时的数据库和文件快照。迁移之后新增的对话或修改也可能被旧快照覆盖；应在后续继续聊天前完成验收，需要回滚时先备份当前数据。空的测试/历史目录可能保留。
- App Server 的项目与分页接口仍可能随版本变化；不保证未来版本永久兼容。新接口不匹配时应失败并回滚，而不是静默写入旧索引。

本 Fork 的中英文入口说明见 `README.md` 和 `README.zh-CN.md`。原许可和署名文件均保留，上游原始说明仍可从基线提交查看。
