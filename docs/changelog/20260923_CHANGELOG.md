# 2026-09-23 变更记录

## 变化

- `BUGFIX-FULL-PRODUCT-IDENTITY-CORRECTION`（所需版本 `0.2.13`）：项目完整统一为“毕方内容生产助手” / `Bifang Content Production Assistant`，根目录、CLI/core/GUI 目录、crate/package、CLI 二进制、Tauri identifier、深链接、凭据服务、偏好键、E2E 环境变量、测试路径、当前文档和许可证全部切换到 `bifang_content_production_assistant` / `bifang-content-production-assistant` 派生身份。
- 当前维护树删除被撤销的错误身份决定与对应变更记录，不保留兼容别名或旧身份回退；旧应用命名空间中的本地数据、设备偏好和系统凭据不会自动读取、复制或删除，Git 提交历史不做破坏性改写。
- `FEATURE-ARTICLE-READ-CLI`（所需版本 `0.2.12`）：产品 CLI 增加 `article list --app-data-dir <DIR>` 与 `article get --app-data-dir <DIR> --id <ID>`，可用人类文本或全局 `--json` 信封只读查看现有桌面工作区中的文章列表和单篇当前产物。
- 列表使用与 Grid 相同的创建时间/稳定 ID 倒序；详情只返回当前 Markdown、简介、HTML、图片存在性、编辑修订和历史数量，不输出当前或历史图片本地路径。
- CLI 要求明确的现有绝对应用数据目录，只以 SQLite read-only、禁止创建模式读取当前 schema v11；不会创建、迁移、恢复或保存工作区，也不访问图片文件、Provider、凭据、网络、stdin 或外部进程。
- 路径/ID 无效、资源不存在、工作区不可用、快照/schema 无效分别提供稳定错误码与退出码；缺失数据库不会因查询被创建。
