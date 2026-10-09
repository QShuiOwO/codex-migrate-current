## Codex Migrate Current 1.1.0-current.1

基于 ChenglongLi777/codex-migrate v1.0.8 的 MIT 社区适配版；首次发布仅提供 Windows x64 GUI 和 CLI。

适配验证目标：ChatGPT Desktop `OpenAI.Codex 26.1002.7124.0`，原生运行时 `codex-cli 0.162.0-alpha.2`。支持本地 Codex 代码对话的分页历史、分叉祖先、原生多根项目、归档状态及完整事务回滚；原生注册失败会自动回滚。

验证：33 项 Rust 测试、13 项独立合成数据原生运行时场景通过；格式和 Clippy 通过。发布二进制由 GitHub Actions 使用 Windows MSVC 构建，下载的 MSVC 程序也在本机通过了全部 13 项原生场景，GUI 隔离启动正常。未在真实账号中完整点击导入。

**范围：** 不迁移 Chat/Work 云端账号历史、附件实体、`thread_attachments`、生成物或项目源码；macOS/Linux 和未来运行时未做同等集成验证。实际迁移或回滚前完全退出 ChatGPT Desktop 和所有 Codex CLI 会话，并备份数据。先审阅 dry-run，导入验收后再继续聊天。

下载 ZIP 后核对对应 `.sha256`，解压运行 `codex-migrate-gui.exe` 或 `codex-migrate.exe`。包内保留原 MIT 许可，包含依赖、字体、Rust 工具链许可和使用说明。程序未代码签名，需要 [Microsoft Visual C++ v14 x64 运行库](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)。

See [README](https://github.com/QShuiOwO/codex-migrate-current/blob/main/README.md) for English instructions and [validation](https://github.com/QShuiOwO/codex-migrate-current/blob/main/docs/current-validation.md) for the tested scope.
