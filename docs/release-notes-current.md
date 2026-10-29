## Codex Migrate Current 1.1.0-current.6

修复 Desktop `26.1023.6621.0` / `codex-cli 0.165.0-alpha.1.2` 下再次导入已有记录时误报“目录不一致”并回滚的问题。新运行时会使用带 `\\?\` 前缀的 Windows 路径；current.6 在目录映射、数据库和对话文件校验中识别等价路径，同时继续拒绝真正不同的目录。

普通记录、归档记录、“仅恢复历史”及分叉祖先的重复导入均已验证。此前 current.5 失败的同一合成目标，用新版直接重试成功，没有新增重复对话或项目。保留选择性导出、额外目录自动创建、完整失败日志及回滚功能。

推荐下载 `Codex-Migrate-Current-1.1.0-current.6-Standalone-Windows-x64.exe`，直接运行，无需同目录 CLI 或额外 VC++ 运行库。MIT 署名与完整第三方许可内置；新版历史导入仍需要目标电脑安装匹配的 Desktop / Codex 运行时。

验证：53 项常规 Rust 测试、1 项 GUI 原生流程、23 项合成原生迁移场景通过，包括项目重绑定和回滚；独立 EXE 单文件启动正常。原生验证使用 Windows GNU 构建和 state schema 62。未操作真实账号数据，未进行真实 UNC 共享、Linux 或干净 Windows 虚拟机同等验收。

附带 GitHub Actions 构建的 Windows x64 MSVC ZIP（GUI 与 CLI，需要 VC++ v14 x64 运行库）、各自校验值及独立程序构建/验证信息。本次仍为预发布版。仅处理本地 Codex 代码对话；项目文件、附件和云端 Chat/Work 历史不在迁移范围内。
