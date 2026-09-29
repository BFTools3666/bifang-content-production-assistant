# 毕方内容生产助手

**English name:** Bifang Content Production Assistant

**Project ID:** `bifang_content_production_assistant`

这是一个面向批量内容运营的本地桌面工作台。用户可以在飞书式 Grid 中批量录入标题，按固定 SOP 生成正文、简介、配图和格式产物，并对失败阶段进行精确重试、编辑和人工审核。首版 MVP 功能与 macOS 开发级 Fixture E2E 已完成；当前版本仍未进入发布候选或渠道发布。

## 当前事实

| 项目 | 值 |
|---|---|
| 负责人 | arden |
| 初始化日期 | 2026-09-10 |
| 当前版本 | `0.2.17` |
| 产品状态 | MVP 功能与开发级 macOS Fixture E2E 已完成；未发布 |
| 产品规格 | `Approved` |
| 目标平台 | Windows、macOS |
| 接口 | CLI、GUI |
| 反馈渠道 | 未定义 |
| 发布渠道 | 未定义 |
| 产物格式 | Markdown、HTML、图片 |

根 `Cargo.toml` 是版本、目标平台与接口组合的唯一事实来源；`.harness/version-state.json` 保存受保护的版本周期和稳定 ID 去重状态。

## 产品边界

- `bifang_content_production_assistant_core` 承载任务、SOP 顺序/执行前置依赖/产物语义、版本、重试、审核和稳定错误等接口无关规则。
- `bifang_content_production_assistant_cli` 提供只读 `sop describe`，以及对显式本地工作区的 `article list`、`article get`；排序、精确查找、脱敏投影和稳定错误由 shared core 定义。文章查询只以 SQLite read-only 模式读取现有数据库，不读取 stdin、Provider、凭据或网络，也不提供写入或批量执行入口，主体验仍位于 GUI。
- `bifang_content_production_assistant_gui` 提供工作台、飞书式 Grid、详情审核、Prompt 模板、AI 服务配置和运行记录。GUI 的九项初始化选择、Logo、深链、快捷键空契约和 UI 标准见 [`docs/GUI_APP_PROFILE.md`](docs/GUI_APP_PROFILE.md)。
- GUI 固定接入 system-locale、updater、window-state 和 dialog 基线；官方 Tauri updater 仍未配置且零出站，dialog 仅向主窗口提供 `dialog:default`，不授予通用文件系统权限。
- GUI 已接入 CloudBase 环境 `bftoos-sh-d3ga7hahz59229f0a` 的匿名日活统计和 application-version v1 版本发现。统计默认关闭并可在设置页撤回；版本凭据缺失时保持零请求，当前仓库不保存生产 Key/Secret。
- 已启用的通知与开机自启设置默认关闭；深链接只恢复主窗口；全局快捷键当前没有动作、默认 chord 或 OS 注册。

## 使用与开发入口

开发环境以根 `Cargo.toml` 和 GUI `package.json` 为准：Rust `1.95`、Node.js `^24.15.0 || >=26.0.0`、pnpm `>=11.24.0`。安装前端依赖并启动真实 Tauri 开发应用：

```bash
cd bifang_content_production_assistant_gui
pnpm install --frozen-lockfile
pnpm tauri dev
```

`pnpm dev` 只启动 Vite 前端，缺少 Tauri IPC 时不能完成持久化、凭据或批次流程。CLI 可用以下只读入口查看四阶段 SOP、执行前置依赖、产物和审核门禁：

```bash
cargo run -p bifang_content_production_assistant_cli -- sop describe
cargo run -p bifang_content_production_assistant_cli -- --json sop describe
```

要查询桌面应用已经保存的文章，必须明确传入包含 `content-workbench.sqlite3` 的现有绝对应用数据目录：

```bash
cargo run -p bifang_content_production_assistant_cli -- article list --app-data-dir /absolute/path/to/app-data
cargo run -p bifang_content_production_assistant_cli -- article list --app-data-dir /absolute/path/to/app-data --json
cargo run -p bifang_content_production_assistant_cli -- article get --app-data-dir /absolute/path/to/app-data --id ARTICLE_ID --json
```

文章命令不会自动寻找、创建或迁移工作区，也不会输出本地图片路径；旧 schema 需先由当前桌面应用升级。JSON 模式继续使用 `ok`、`data`、`error`、`meta` 四字段顶层信封；成功命令名为 `sop.describe`、`article.list` 或 `article.get`，显式 `--json` 的参数错误使用 `cli.parse`、退出码 `2`，且 stderr 保持为空。

GUI 的普通运行路径使用本地 SQLite 工作区，以及用户明确保存的 OpenAI-compatible 文本/图片服务或火山方舟 Ark v3 图片服务。API 密钥只写入系统凭据库；没有完整默认服务或凭据的阶段保持零出站并返回可恢复错误。测试夹具仅在 debug 构建同时显式启用 E2E 开关、并提供通过校验的独立系统临时目录时出现；它生成的是确定性测试内容，不代表真实 AI、内容质量或生产数据。

## MVP 交付边界

当前范围以 [已批准 Product Spec](docs/product_spec/20260929_product_spec.md) 为准，完成证据汇总于 [MVP 工作计划](docs/work_plan/20260913_work_plan.md)。当前已实现任务/SOP 状态机、SOP 与文章只读 CLI、本地持久化、OpenAI-compatible 文本与图片适配器、火山方舟图片适配器、Grid 批量操作、详情审核、Prompt 模板、分阶段 AI 服务配置、运行记录，以及批次暂停/继续/取消和重启恢复。Core-first 规则要求业务逻辑进入 shared core，CLI 与 GUI 保持薄层。

OpenAI 正文链路与火山方舟图片链路已由用户在 macOS 应用中使用系统凭据库内的真实配置手动确认成功；本轮没有在证据中记录或输出密钥，也没有验证 Windows。后续如需异常 Provider/凭据库故障验证、签名/公证、发布候选、安装包、推送、tag 或渠道发布，必须由用户另行授权并进入对应门禁；开发级人工结果不能替代这些工作。

初始化本身不授权创建远端、推送、发布、处理凭据或操作生产环境。新功能或独立问题修复按 `docs/AGENT_POLICY.md` 使用受管 Git 生命周期；只有用户明确说“推送”或“发布”时才进入对应外部流程。

当前 `user_owned_tasks: disabled`，普通请求直接在当前 Task 完成；用户仍可明确要求创建一个用户可见 Task。用户可见 Task 使用 `Task {序号} | {当前进度} | {单一结果}`，内部 plan、Subagent 与 Worktree 不使用该标题合同，也不占用项目序号。完整开关、结果边界、绑定和零写入门禁见 [`docs/AGENT_POLICY.md`](docs/AGENT_POLICY.md)。

## 项目结构

- `AGENTS.md`：轻量启动门禁、任务路由、Skills 与约束地图。
- `docs/AGENT_POLICY.md`：schema v3 持久策略、用户可见 Task 开关/标题/创建门禁与 Git 生命周期。
- `docs/ENGINEERING_RULES.md`：工程、Core-first、测试与记录触发规则。
- `docs/RUST_CLI_TEMPLATE.md`：Rust core 与适配器技术基线。
- `docs/CLI_CONTRACT.md`：CLI 机器接口契约。
- `docs/GUI_APP_PROFILE.md`：本项目 GUI 的已确认配置与身份事实。
- `docs/design_standards/`：可机械匹配的 GUI 设计标准。
- `.agents/skills/`：终端项目开发、构建、验收和工程层升级能力。

本项目不得继续实例化或初始化其他项目；需要同步较新的工程规则时使用 `$desktop-upgrade-harness`，默认先预览并保护产品代码、本项目身份、策略、许可证和项目记忆。

## 验证与发布

产品代码基线 `9dd96eb47fd85d5ed4da58722470e021ec0882a6` 的 Rust workspace 188/188（CLI 7、core 44、GUI 137）通过；其后的前端 UI 收口通过 Vitest 12 个文件 77/77、typecheck、lint 与 production frontend build。产品化 CLI 的独立 Worktree 产品代码/文档提交分别为 `3ce780988d6a48ee35636c7af1f25ec2883b6901`、`9eb2c7d7e4c684b6abefed3948c3158c7ea0e77a`，主功能分支对应提交分别为 `4eeb7836627654766beb3ef0cb84fc311313d173`、`941e5e3c86a6ed5c3854a1e5fb80cef18ac59478`；后续运行时清理/证据提交 `291c2a1ac6ef6deb9cdf92ccb3533d6ba7ba6d45`、`8fb55935e0104cdf10cddd351567cd6e20140fbf` 分别映射为主功能分支 `f9abf4224186ec47fffa5f268e4840abbec7c45c`、`bd50dbc37b6553c698e19c353c052f8ac89fcb4e`。主控在该合入 HEAD 上重新验证 Rust workspace 190/190（CLI 9、core 44、GUI 137）、前端 12 个文件 77/77、typecheck、lint、`cargo fmt --all -- --check`、本次 TypeScript Prettier 与 `git diff --check` 均通过。桌面验收的强化计划证据提交为 `063e07a946cdc9c67c46be672bfdeece12e5272e`。

由该产品代码基线全新构建的 macOS arm64 debug Tauri `.app`，在隔离 Fixture 数据库中完成 16/16 个必需真实窗口场景；最终 35 张独立 JPEG 的文件名、字节、尺寸和 SHA-256 与清单逐项一致。macOS 系统剪贴板正常复制已通过；强制制造系统剪贴板拒绝为非阻断 `NOT RUN`，并保留为剩余风险。

以上是开发级验证，不能代替当前版本的发布候选验收；Windows 或真实 Provider 异常/凭据库故障仍缺少宿主验证。构建与发布规则见 [`docs/RELEASE.md`](docs/RELEASE.md)，剩余风险见 [`docs/TECH_DEBT.md`](docs/TECH_DEBT.md)。

文章只读 CLI 变更在当前 macOS 开发宿主通过 core 84/84 与 CLI 21/21：其中 12 个文章 CLI 黑盒用例覆盖排序、详情脱敏、人类/JSON 输出、只读前后数据库字节与目录文件集合不变、缺失数据库不创建、路径/schema/损坏库/缺失文章错误，以及 stdin 不阻塞。共享 schema 常量调整另通过 GUI crate `cargo check`；本次未执行 GUI E2E、发布候选构建或 Windows 原生验收。

## 许可

本项目是专有商业软件，不是开源项目。使用、复制或分发前，请阅读 [`LICENSE.zh-CN.md`](LICENSE.zh-CN.md) 或 [`LICENSE.en.md`](LICENSE.en.md)。
macOS DMG 中的应用也随包携带完整的中英文许可文件：在 Finder 中对应用选择“显示包内容”，打开 `Contents/Resources/Legal/` 即可阅读。
