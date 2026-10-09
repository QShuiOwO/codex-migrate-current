## Codex Migrate Current 1.1.0-current.2

修复 GUI 导入只显示 “failed / 已回滚” 的问题：显示完整错误链，失败自动展开日志，并提供复制、保存功能。预览/导入日志自动写入 `%LOCALAPPDATA%\codex-migrate\logs\`，界面显示文件路径，回滚不会删除日志。

绑定项目时，GUI 显示额外工作目录和权限目录，并允许选择本机位置。主项目的子目录继承主项目映射；缺失映射在预览阶段被拒绝。Windows 优先使用 Desktop 自带运行时，显式 `CODEX_MIGRATE_CODEX_BIN` 覆盖仍有效。

目标：ChatGPT Desktop `26.1002.7124.0`，运行时 `codex-cli 0.162.0-alpha.2`。验证包含 39 项常规 Rust 测试、1 项单独启用的 GUI 导出/选项/预览/原生导入流程，以及 16 项合成数据原生场景。失败日志覆盖底层原因、初始化 stderr 和回滚结果。没有在真实账号中完整点击导入，也没有拿到用户目标机器的底层失败日志；不能仅凭版本号确认此次失败的唯一原因。

下载 Windows x64 ZIP 解压后运行 `codex-migrate-gui.exe`。需要 Microsoft Visual C++ v14 x64 运行库。实际迁移前完全退出 Desktop 和所有 Codex CLI 会话，并保留备份。若失败，请复制详细日志或提供界面显示的 `.log` 文件内容；分享前核对路径与线程 ID。

基于原作者 MIT 项目的社区 Fork；只迁移本地代码对话，附件实体、生成物、项目源码和 Chat/Work 云端账号历史不在范围内。许可与归属保留在发布包中。
