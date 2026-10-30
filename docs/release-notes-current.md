## Codex Migrate Current 1.1.0-current.7

Windows x64 与 Ubuntu 24.04 x64 统一维护：同一源码、版本号与备份格式，在同一个 Release 提供两个平台包。保留 Desktop 项目注册、选择性导出、分叉祖先、分页历史、额外目录创建与事务回滚。

Ubuntu GUI 默认使用 Codex CLI 导入模式。读取 Windows 的 `backup` / `Codex_backup`，选择会话和本机工作文件夹，导入后在该目录运行 `codex resume` 即可选择会话继续。Ubuntu 也可导出备份，在 Windows 映射到本机目录后导入。建议两端都升级到 current.7，特别是相同会话的往返合并。

新增 `--target cli|desktop`；操作系统与导入目标独立，Windows 也可使用 CLI 模式。CLI 模式恢复所选归档/非交互记录的列表可见性，适配目标服务商，保留祖先历史与原始元数据。跨系统映射支持 Windows 盘符大小写、UNC、扩展路径与权限 glob，Unix 路径保持大小写区分。

下载匹配系统的包及其校验文件：

- `Codex-Migrate-Current-1.1.0-current.7-Windows-x64.zip`：GUI 与 CLI，需 VC++ v14 x64 运行库。
- `Codex-Migrate-Current-1.1.0-current.7-Ubuntu-24.04-x64.tar.gz`：GUI 与 CLI，解压后运行 `./codex-migrate-gui`。

Ubuntu 本机验收使用 Rust 1.99.1 / Codex CLI 0.166.0，包含 56 项常规 Rust 测试、GUI 原生流程、23 项 Desktop 引擎场景、9 项 CLI/往返场景及真实 `codex resume` 界面测试。发布工作流要求 Windows 和 Ubuntu 的 CI 构建与测试均成功。Windows Desktop 实机验收范围单独记录，不将跨平台 CI 等同于 Desktop 端到端验收。

本次仍为预发布版。仅迁移本地 Codex 代码对话，项目源码、附件实体和 Chat/Work 云端历史需要另行处理。测试只使用合成记录；导入前退出使用目标历史库的 Codex / Desktop，验收后再继续聊天。
