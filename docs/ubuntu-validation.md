# Ubuntu 24.04 验证记录

日期：2026-10-30。本机验证针对 current.7 统一版本源码；发布状态见仓库 Releases。

环境：Ubuntu 24.04.5 LTS x86_64、Rust 1.99.1、Codex CLI 0.166.0（原生 Linux 运行时）、Wayland 桌面。

| 检查 | 结果 |
| --- | --- |
| Rust 单元/集成测试（含 GUI 常规测试） | 56 项通过；另 1 项可选原生测试单独运行 |
| GUI 原生导出、预览、导入测试 | 1 项通过 |
| 原有 Desktop 引擎原生迁移场景 | 23 项通过 |
| 新增跨平台 CLI 迁移场景 | 9 项通过 |
| 实际 `codex resume` 终端界面 | 列表出现迁移会话，回车后显示父/子历史与输入框 |
| Ubuntu Wayland GUI 启动 | 正常，未崩溃 |
| `cargo fmt` / Clippy `-D warnings` | 通过 |
| release 构建与 Linux 打包 | 通过，包含 GUI、CLI、依赖许可和 SHA-256 |

新增场景包括 Windows 盘符大小写及扩展路径、`backup` 父目录读取、所选归档/非交互记录转为 CLI 可见、正确目录筛选、分页祖先和 legacy 消息恢复、重复导入与回滚、Ubuntu 再导出后通过 Desktop 引擎导入、自定义模型服务商及变更服务商后的重导入。

所有会话由测试生成，历史库位于隔离目录；没有修改用户真实 `.codex`，也没有提交模型请求。终端测试使用 loopback 测试服务商，仅打开恢复后的会话。项目源码、附件实体迁移不在本次范围。Windows 格式和 Desktop 导入引擎在 Ubuntu 上测试；新增代码尚未经过实体 Windows Desktop 实机验证。Windows/Ubuntu 远程构建与测试结果以对应 PR 或版本标签的 GitHub Actions 检查为准。

可复现命令见 [Ubuntu 使用指南](ubuntu-cli.md)。原生场景结果保存在运行环境的 `test-results/current-runtime-results.json`、`test-results/cli-runtime-results.json`；终端记录为对应合成测试目录的 `resume-tui.txt` 和 `resume-tui.raw`。
