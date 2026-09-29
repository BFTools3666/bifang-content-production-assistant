# AGENTS.md

## 项目使命

本仓库是终端下游项目“毕方内容生产助手”（Bifang Content Production Assistant，项目 ID：`bifang_content_production_assistant`）。当前版本 `0.2.17`，负责人 `arden`，目标平台为 Windows、macOS，接口为 CLI、GUI。批量 AI 内容工作台 Product Spec 已批准；当前 MVP 仍须以真实可观察产物和对应验证结论声明完成程度。

本项目不得再实例化、初始化或派生其他项目；不存在的派生入口不得重建。工程层升级只能使用 `$desktop-upgrade-harness`，且默认先预览并保护产品代码、身份、策略、许可证和项目记忆。

## 启动门禁

1. 先读本文件，再读 `docs/AGENT_POLICY.md` 的 YAML frontmatter 与字段语义；仅在创建/执行用户可见 Task、显式并行、构建、发布或修改持久策略时继续读对应章节。
2. 写入前确认当前 Git 顶层、分支和工作区状态，保护已有修改；重叠内容不能安全处理时停止。不得虚构命令、能力、测试、构建或平台结论。
3. 先判定任务类型，只读取下表命中的事实源与 Skill；发生缺失或冲突时再扩大读取。
4. 产品目标取自最新已批准 `docs/product_spec/`；改变目标、边界、约束或成功标准时使用 `$desktop-define-product`，不从普通实现请求推断新范围。

## 按任务渐进读取

| 当前任务 | 精确读取 | 执行入口 |
|---|---|---|
| 定义或改变产品目标、边界、约束、成功标准 | `docs/product_spec/`（若存在）与相关最新 ADR | `$desktop-define-product` |
| 范围清楚的实现、修复或用户可感知优化 | `docs/ENGINEERING_RULES.md`、所选接口事实、版本状态 | `$desktop-implement-change`、`$desktop-manage-version` |
| Rust core 或 CLI | `docs/RUST_CLI_TEMPLATE.md`、`docs/CLI_CONTRACT.md` | `$desktop-add-cli-adapter` 或 `$desktop-implement-change` |
| GUI 展示、交互或桌面能力 | `docs/GUI_APP_PROFILE.md`、`docs/design_standards/README.md` 后只读命中的标准、`docs/RUST_CLI_TEMPLATE.md` | 精确命中的 GUI Skill |
| 新功能、Bug 修复、推送或发布 | `docs/AGENT_POLICY.md` 的 Git 生命周期；发布时读 `docs/RELEASE.md` | `$desktop-manage-git-lifecycle` |
| 创建或检查用户可见 Task | `docs/AGENT_POLICY.md` 的“用户可见 Task 开关、粒度与创建门禁” | Codex 项目/Task 工具；策略启用或用户明确要求时调用 |
| 当前 Task 内显式并行 | `docs/AGENT_POLICY.md` 的内部 Subagent/Worktree 规则 | `$desktop-run-parallel-worktrees` |
| 恢复、重要阻断或跨会话交接 | `docs/project_status/README.md` 与存在时的最新状态；存在活动计划时再读最新 Work Plan | `$desktop-plan-change` 仅在触发时使用 |
| Windows GUI 本地试包 | 根 Cargo 平台/接口元数据 | `$desktop-build-tauri-local-install` |
| 显式构建、发布候选或完整验收 | `docs/RELEASE.md` 与当次候选事实 | 精确命中的构建、测试或验收 Skill |
| 工程层升级 | 当前项目事实、来源锁（若存在）与升级 Skill | `$desktop-upgrade-harness`，默认预览 |

## 始终生效的边界

- 执行持久策略：`user_owned_tasks: disabled`、`superpowers: disabled`、`parallel_worktree_subagents: enabled`、`acceptance_smoke: enabled`、`e2e_hint: disabled`。启用只表示在精确触发且获得所需授权时可用；安全、渠道和产品硬要求优先。
- `user_owned_tasks: disabled` 时普通请求直接在当前 Task 完成，不自动创建或拆分左侧 Task；用户明确要求创建时仍执行。内部 plan、Subagent、Worktree、brief、report、review 和 checkpoint 不是用户可见 Task，不使用标题合同，也不占用 Task 序号。
- Core-first 是硬规则：接口无关的领域类型、值域、跨字段关系、业务默认值、状态转换、用例和稳定错误属于 core；CLI/GUI 只负责协议、展示、交互、宿主能力与映射。
- 日常开发只运行本次需要的非空测试；不得因任务复杂自动增加持久计划、全仓检查、构建、冒烟、E2E、Verification 或人工复核。
- 版本只通过 `$desktop-manage-version` 管理。根 `Cargo.toml` 是当前版本事实源，`.harness/version-state.json` 是受保护的周期与稳定 ID 去重状态。Minor/Patch 固定为 `0..99`；历史下位分量 `100` 不再兼容，出现时版本门禁失败关闭。
- 对“完成、可用、已验证”的声明必须来自真实可观察结果；中性 scaffold、Mock、源码片段或开发预览不能替代候选验收。
- 安全/隐私、数据迁移、破坏性操作、生产/付费/凭据副作用、外部兼容契约、签名、发布和跨平台最终候选必须进入对应专用门禁。
- Product Spec、ADR、Changelog、Product Status、Work Plan 与 Verification 只在各自独立事件触发时创建；当前不存在不是缺陷。范围外问题记录到 `docs/TECH_DEBT.md`。

## 用户可见 Task 开关、标题与门禁

- 用户可见 Task 只使用 `Task {序号} | {当前进度} | {单一结果}`。单一结果全程不变，进度只取 `已分配`、`运行中`、`检查中`、`已完成`；不识别历史四字段标题格式。
- 新结果在同一非空 `hostId` 与精确 `projectId` 下，合并 `list_threads(limit=50)` 的当前/pinned 结果与同宿主逐页 `list_archived_threads(limit=50)` 的归档结果，只从合规标题最大序号加一；空历史才从 1 开始，缺号不回填。宿主或历史无法可靠枚举时不得猜测序号。
- `user_owned_tasks: enabled` 时，交付物类型、生命周期阶段、外部副作用、禁止范围或验收责任越出当前固定边界前自动创建新 Task；证明同一结果的测试、review、checkpoint 和必要同范围修复不拆。每个项目同一时间只允许一个写入型 active 用户可见 Task，Task0 只能协调。
- 创建前用 `list_projects` 精确绑定保存项目与 `projectId`；Git 项目使用独立 Worktree，非 Git 项目使用 Local，禁止 projectless。一个结果只调用一次 `create_thread`；只有真实 `threadId` 到手并用 `list_threads` 核对标题、项目、cwd 和状态，再核对干净工作区与起始提交后才能写入。仅有 `clientThreadId` 或任一事实不符时保持零实现，不创建替代 Task。
- 真实进入新进度时至多更新一次标题并按真实 `threadId` 有界复读；更新失败必须报告，但不推翻已经完成的工作，也不得虚写 `已完成`。Git Task 首次产品写入前由 `$desktop-manage-git-lifecycle start` 创建并登记 `feature-*` 分支；内部并行仍只在用户明确要求且策略允许时使用互不重叠所有权的 `codex/unit-*`。

## Skills 地图

项目 Skills 位于 `.agents/skills/`；命中后完整读取对应 `SKILL.md` 与其精确引用。

- 产品与开发：`$desktop-define-product`、`$desktop-plan-change`、`$desktop-implement-change`、`$desktop-refactor-code`、`$desktop-manage-version`、`$desktop-manage-git-lifecycle`、`$desktop-configure-git-commits`、`$desktop-run-parallel-worktrees`、`$desktop-rename-project-identity`、`$desktop-upgrade-harness`。
- CLI/GUI：`$desktop-add-cli-adapter`、`$desktop-add-gui-adapter`、`$desktop-add-gui-system-locale`、`$desktop-add-gui-updater`、`$desktop-add-gui-window-state`、`$desktop-add-gui-dialog`、`$desktop-add-gui-system-tray`、`$desktop-add-gui-system-notifications`、`$desktop-add-gui-autostart`、`$desktop-add-gui-single-instance`、`$desktop-add-gui-deep-link`、`$desktop-add-gui-global-shortcut`、`$desktop-prepare-gui-app-identity`、`$desktop-prepare-gui-support-surfaces`、`$desktop-extract-i18n-strings`。
- 构建与验收：`$desktop-build-rust-release`、`$desktop-build-tauri-local-install`、`$desktop-build-tauri-release`、`$desktop-prepare-cross-platform-release`、`$desktop-collect-release-artifacts`、`$desktop-prepare-release`、`$desktop-test-gui-release-performance`、`$desktop-test-final-artifact-e2e`、`$desktop-verify-delivery`。
- 环境：`$desktop-check-development-environment` 只在受管环境真实失败恢复或精确触发时使用，不例行重跑。

## 约束地图

| 约束或事实 | 唯一来源 |
|---|---|
| 产品身份、初始状态与未知渠道 | `README.md` |
| Agent 能力、用户可见 Task 开关/标题/创建门禁与 Git 生命周期 | `docs/AGENT_POLICY.md` |
| 平台、接口与当前版本 | 根 `Cargo.toml` 的 workspace package/metadata |
| 版本周期与稳定 ID 状态 | `.harness/version-state.json`（受保护） |
| 工程、Core-first、依赖、测试与记录触发 | `docs/ENGINEERING_RULES.md`、`docs/RUST_CLI_TEMPLATE.md` |
| CLI 机器接口 | `docs/CLI_CONTRACT.md` |
| GUI 配置、Logo、深链与快捷键契约 | `docs/GUI_APP_PROFILE.md` |
| UI 匹配与密度 | `docs/design_standards/README.md` 及命中的标准 |
| 构建与发布 | `docs/RELEASE.md`；活动候选证据只在忽略的 `release/` |
| 当前进度与下一步 | `docs/project_status/README.md` 及存在时的最新日期状态 |
| 技术债与已知限制 | `docs/TECH_DEBT.md` |
| 开发分支、Worktree 与发布周期资源 | Git common-dir 的生命周期清单；它不是 tracked 资产 |
| 当前发布选择 | `.harness/release-context.json`，仅由 `$desktop-prepare-release` 在真实发布时创建 |
| 工程层来源与升级 | `.harness/upstream-lock.json`（若存在）与 `$desktop-upgrade-harness` |
| 商业许可 | `LICENSE.zh-CN.md`、`LICENSE.en.md` |

## 每次任务的最小闭环

1. 确认仓库边界与任务类型，只读精确事实源和 Skills。
2. 在授权范围内实施，保护已有修改，只运行本次需要的非空测试或最小替代检查。
3. 仅更新被独立事件触发的权威记录；同范围失败修复后重跑，新增副作用或范围变化先取得授权。
4. 报告实际变化、实际验证、未执行项与剩余风险；不把日常开发或中性脚手架描述为发布就绪。
