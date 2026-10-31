## Codex Migrate Current 1.1.0-current.8

修复选择性导出分叉会话时误报 `source contains duplicate thread IDs; resolve duplicate rollout files before export` 的问题。建议遇到该提示的 current.7 用户升级；无需删除原始对话文件。

部分分叉记录同时包含自身会话头和复制进来的父会话头。旧扫描器使用后出现的父 ID 覆盖子 ID，使不同会话被当成重复记录。current.8 以首条会话头确定 ID、工作目录、模型服务商和继承信息，继续保留全部历史消息。真正重复的会话文件仍会被阻止导出。

升级后关闭旧迁移工具，打开新版并重新读取/刷新对话列表，再勾选导出。正式导出前，退出使用目标历史库的 Desktop / Codex CLI 会话。

两个平台共用同一源码和版本：

- `Codex-Migrate-Current-1.1.0-current.8-Windows-x64.zip`：GUI 与 CLI，需 VC++ v14 x64 运行库。
- `Codex-Migrate-Current-1.1.0-current.8-Ubuntu-24.04-x64.tar.gz`：GUI 与 CLI，解压后运行 `./codex-migrate-gui`。
- 两个压缩包均附 SHA-256 文件，内含许可证、第三方许可和程序校验值。

合成回归覆盖多个内联分叉的扫描、选择性导出、再次导出、源文件保持不变、真正重复 ID 的拒绝，以及真实 Codex 导入/恢复父子消息。本机 Ubuntu 常规 Rust 测试增至 59 项；Windows、Ubuntu CI 和统一产物校验须通过后才能进入发布流程。

保留 current.7 的 Windows ↔ Ubuntu 迁移、Desktop/CLI 导入目标、路径/服务商适配、分页历史和事务回滚。本次仍为 Pre-release；Windows 编译与测试不等于 Desktop 实机验收，项目源码、附件实体和云端聊天历史仍不由该工具迁移。
