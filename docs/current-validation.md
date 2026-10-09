# 当前版本验证记录

日期：2026-10-08。副本版本：`1.1.0-current.1`。

本机目标：Windows x64，ChatGPT Desktop MSIX `OpenAI.Codex 26.1002.7124.0`，实际原生运行时 `codex-cli 0.162.0-alpha.2`。

构建工具：Rust `1.99.0`（GNU），LLVM-MinGW `20261006`（MSVCRT），依赖由现有 Cargo.lock 解析。便携工具链放在工作区 `.toolchain/`，没有更改系统 PATH。

## 源码回归

`cargo test --features gui`：**33 项通过，0 失败**。

- 核心单元测试 23 项，包括旧格式、路径映射、父历史排序、缺失/循环依赖、分页根目录、管理式权限路径与分叉字节边界。
- GUI 逻辑测试 4 项。
- CLI 回归 6 项，验证旧 SQLite 后备模式、扫描、导出、重复导入修复与回滚。

`cargo clippy --all-targets --features gui`：通过，无警告。

`cargo fmt --all -- --check`：通过。

`cargo build --release --features gui`：CLI 和 GUI 的 Windows x64 发布程序构建成功。

## 当前原生运行时集成

脚本：[tests/current_runtime.py](../tests/current_runtime.py)。结果：[current-runtime-results.json](current-runtime-results.json)。**13 项通过，0 失败**，覆盖：

1. dry-run 不写入目标，自动展开并排序分叉父历史。
2. 全新目标数据根初始化；分页分叉、已归档父对话、Unicode 名称、多根项目；源对话与源业务数据库未改变。
3. 分页分叉 HTML 同时包含父记录和子记录的消息。
4. 重复导入、备份目录改名后再次导入不产生重复对话历史或项目。
5. 已缓存的分页分叉重新绑定路径；手动回滚后状态库、显示历史、rollout 和兼容索引与操作前一致。
6. 205 条消息跨越三页读取，验证没有仅读取第一页；无源数据库的 JSONL 备份重复导入仍复用项目。
7. 父历史缺失在写入前被拒绝。
8. 合法记录已经导入后，遇到运行时不支持的消息类型导致失败，自动回滚恢复原有业务数据。
9. legacy 对话使用当前运行时后保持 legacy 模式。
10. 缺少原生运行时的分页迁移在写入前被拒绝。
11. 已归档分页对话迁移后仍归档，历史可读取。
12. 仅历史模式为分叉建立占位工作区，不创建实际项目。
13. 单独的 `CODEX_SQLITE_HOME` 正确接收数据库；回滚全新目标删除新增状态库与显示历史库。

这些场景运行在 `test-results/` 内的合成数据根；没有读取正式配置、凭据或真实对话，没有调用 `turn/start`，没有发起模型推理。集成脚本的对比关注业务数据库与文件内容，不要求 SQLite 文件的物理字节布局一致。

## 2026-10-09 发布包复核

[GitHub Actions CI](https://github.com/QShuiOwO/codex-migrate-current/actions/runs/37923815341) 使用 Rust 1.99.0、`x86_64-pc-windows-msvc` 完成格式检查、Clippy、33 项测试、GUI/CLI release 构建、230 个已解析依赖的许可收集及打包，全部通过。

下载该构建产物后，核对 ZIP 和两个程序的 SHA-256、原 MIT 许可、依赖/字体许可、Rust 工具链版权文档以及构建提交信息。使用包内 MSVC CLI 再次运行上述 **13 项原生运行时场景，全部通过**；最新汇总记录在 `current-runtime-results.json`，已移除本机输出目录。GUI 在隔离的 `CODEX_HOME` / `CODEX_SQLITE_HOME` 下启动后保持运行，随后关闭测试进程；没有执行完整 GUI 导入。

MSVC 发布程序导入 `VCRUNTIME140.dll`，运行要求为 Microsoft Visual C++ v14 x64 Redistributable；README 给出微软官方下载说明。正式对话、账号配置和凭据仍未参与测试。

## 验证边界

本记录证明本机版本的原生历史/项目接口与迁移逻辑可以配合工作。没有在真实账号中实际导入，也没有对 GUI 做完整点击验收；没有跨平台集成结果。云端 Chat/Work 历史、附件实体与其他生成物不在本次迁移实现和验证范围。完整说明见 [CURRENT-VERSION.md](../CURRENT-VERSION.md)。
