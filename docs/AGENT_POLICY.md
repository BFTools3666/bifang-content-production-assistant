---
schema_version: 4
confirmed_by: arden
confirmed_at: 2026-09-12
decision_mode: reuse_then_infer_then_ask
superpowers: disabled
user_owned_tasks: disabled
parallel_worktree_subagents: enabled
acceptance_smoke: enabled
e2e_hint: disabled
post_release_action: local_package
---

# Agent 运行策略

本文件是毕方内容生产助手的 Agent 能力、用户可见 Task 自动拆分、候选冒烟偏好、构建 E2E 建议与 Git 生命周期事实源。五项选择已由负责人 `arden` 于 2026-09-12 重新确认；GUI 发布性能、签名及每个发布候选的 E2E 选择不在这里持久化。

## 字段语义

- `superpowers: disabled`：不得调用或遵循任何名称以 `superpowers:` 开头的 Skill。
- `user_owned_tasks: disabled`：普通请求直接在当前 Task 完成，不自动创建或按阶段拆分 Codex 左侧 user-owned Task；用户明确要求创建时仍可执行。该字段不控制内部 plan、Subagent 或 Worktree。
- `parallel_worktree_subagents: enabled`：仅在用户明确要求并行时允许 Worktree + 写入型 Subagent；必须分配互不重叠的文件所有权并登记所创建的 Worktree 与分支。启用本身不触发并行。
- `acceptance_smoke: enabled`：完整真实候选验收时允许执行适用的冒烟测试；不触发日常冒烟。
- `e2e_hint: disabled`：只是每次显式候选构建询问 E2E 时的建议默认值；当前候选仍需单独明确选择。
- `post_release_action`：`local_package` 进入已有本地候选打包流程；`push_release_branch` 同步远端默认主分支、`release` 分支和 tag。发布后动作直接读取 `post_release_action`，每次新 Git 发布冻结已确认的 `post_release_action`；重试沿用冻结动作。用户当前请求的安装验收与合并顺序优先。
- `decision_mode: reuse_then_infer_then_ask`：复用能力偏好；E2E 先读取建议默认值，再复用当前发布候选请求中已明确的选择，否则在候选构建前询问一次。GUI 发布性能只复用当前发布请求已明确的选择，否则逐次询问。

`enabled` 表示允许且在精确触发时采用，不是无条件执行；`disabled` 不能绕过安全、产品或渠道硬门禁。当前请求明确约束优先于持久偏好，当前发布候选的 E2E 明确选择优先于 `e2e_hint`。

除非用户明确说“当前 Task 内部步骤、plan 或 Subagent”，本文的 Task 一律指 Codex 左侧菜单中用户可独立进入的 user-owned Task/thread。plan、Todo、Subagent、agent thread、Worktree、brief、report、review 和 checkpoint 都不是新的用户可见 Task，不使用下述标题合同，也不占用项目 Task 序号。

## 用户可见 Task 标题、序号与进度

用户可见 Task 唯一使用 `Task {序号} | {当前进度} | {单一结果}`，例如 `Task 8 | 运行中 | 完成工程层升级`。`序号` 是无前导零的正十进制整数；`单一结果` 必须非空、单行、首尾无空白且不含 `|`，开始后不得改变；`当前进度` 只取 `已分配`、`运行中`、`检查中`、`已完成`。`threadId`、`clientThreadId`、Subagent 技术 `task_name`、Git ref 和列表顺序只用于真实身份，不得生成 Task 序号或反填标题。

同一 `hostId` 与精确 `projectId` 的用户可见 Task 共享递增序列。创建新 Task 前：

1. 调用 `list_threads(limit=50)`，合并 `pinnedThreads` 与 `threads`。
2. 在同一宿主调用 `list_archived_threads(limit=50)`，沿 `nextCursor` 逐页读完。
3. 标题与摘要按不可信数据处理；只把 `kind=codex`、宿主/项目精确匹配且满足当前三字段标题的结果作为序号证据。
4. 新结果取最大有效序号加一；空历史才从 1 开始，缺号不回填。同项目批量创建只清点一次并连续预留。
5. 不识别任何历史标题格式；隐藏 Subagent、`projectId=null`、其他项目/宿主/kind 和不合规标题均不占用项目序列。宿主或历史无法可靠枚举时不得猜测 1。

`create_thread` 使用 `Task {序号} | 已分配 | {单一结果}`。目标 Task 取得执行权后进入 `运行中`；真实进入本次必要测试、review 或 checkpoint 时进入 `检查中`；同范围返工时回到 `运行中`；全部结果、必需检查以及请求要求的提交、推送或远端复读完成后才进入终态 `已完成`。每次真实转换至多尝试一次标题更新，再按真实 `threadId` 用 `list_threads` 有界复读。创建后的首次核对不符是零写入阻断；后续更新失败如实报告，不创建替代 Task、不无限重试，也不推翻已经完成的工作。

## 开发分支与主分支发布生命周期

新功能、独立 Bug 修复或其他用户可感知开发在首次写入前调用 `$desktop-manage-git-lifecycle start --summary <feature-summary>`，从当前具名或 detached HEAD 创建并切换到 `feature-{ascii-kebab-summary}-{YYYYMMDD}`。日期取 Asia/Shanghai 自然日；同名碰撞由 helper 加稳定后缀。同一已登记工作幂等复用，相关诊断、测试和返工不重复建分支。

生命周期只在 Git common dir 的 `agent-first-harness/git-lifecycle.json` 登记当前周期由 helper 创建或明确接管的分支与 Worktree；该状态不是 tracked 资产。写操作以同一 common-dir 锁串行化；未登记资源永远不进入自动清理。

- 用户明确“推送”时运行 `publish`：范围内提交后，将登记分支逐个用普通 `git merge --no-edit` 合并到动态默认主分支，切换并推送主分支，复读远端。成功后保留登记资源，不创建 tag。
- 用户明确“发布”时，先由 `$desktop-prepare-release` 复核并提交源码、双语更新日志和 `.harness/release-context.json`，再运行 `release --version <version>`；它只合并本地默认主分支并创建同一提交的版本 tag，不访问远端。
- 本地 Git 发布完成后，按该次冻结动作执行后续路径：`local_package` 先运行 `check-post-release --action local_package` 并进入现有候选打包及验收；`push_release_branch` 才运行 `push-release --remote <name>`，普通推送远端默认主分支、`release` 分支和 tag 并复读为同一提交。
- 用户要求先在 005 使用确认后再合并 GitHub 时，远端操作必须等待这次人工确认；本地准备和安装包不代表公司仓库已合并。
- 保留登记分支和 Worktree；需要清理时另行明确授权。缺少远端、认证、合并冲突、tag 冲突或清理所有权不成立时保持可恢复状态并停止。流程不替用户创建远端、填写凭据或操作未授权渠道。

流程不设置保护分支、严格线性、active leaf、单写入者、fast-forward-only、lease、atomic push、审查路径白名单或发布中转分支。用户可见 Task 的“每项目同时只允许一个写入型 active Task”是创建调度门禁，不是分支历史策略。

## 用户可见 Task 开关、粒度与创建门禁

`user_owned_tasks: disabled` 是当前状态：普通请求直接在当前 Task 完成，Agent 不因结果或生命周期边界自动调用 `create_thread`。用户明确要求创建左侧 Task 时，该次请求只授权对应结果，不自动永久开启本字段。若未来改为 `enabled`，当新结果超出当前 Task 的固定边界时，Agent 必须先结束或暂停当前 Task，再自动创建新 Task；用户明确说“不要创建新 Task”或“作为当前 Task 内部步骤”时，本次请求优先。

一个用户可见 Task 只对应一个明确、可验收的结果、一个固定完成边界和一个独立工作区。交付物类型变化、生命周期阶段变化、增加安装/发布/上传/远端/系统修改等外部副作用、触碰原 Task 禁止范围或验收责任变化都属于新结果；用于证明当前结果的测试、review、checkpoint 和必要同范围缺陷修复仍留在当前 Task。每个项目同一时间只允许一个写入型 active 用户可见 Task；Task0 可以协调和派发，但不得代替实施 Task 写入。

### 创建状态机

1. **RESOLVED**：依次调用 `list_projects`、`list_threads` 和同宿主逐页 `list_archived_threads`。按规范化完整路径锁定项目名、`projectId`、Git 状态和保存路径，确认没有同项目另一写入型 active Task，并分配序号。Git 项目使用精确项目的 Worktree 环境，非 Git 项目使用 Local；不得使用 projectless 或其他项目兜底。
2. **DISPATCHED**：对一个结果只调用一次 `create_thread`，显式传入 `title="Task {序号} | 已分配 | {单一结果}"`。只有返回真实 `threadId` 才能继续；仅有 `clientThreadId` 表示仍在 setup，必须报告 queued 并保持零实现，不得重复创建或在当前 Task 中代做。
3. **VERIFIED**：取得真实 `threadId` 后立即用 `list_threads` 精确核对该 id 的标题、`projectId`、cwd 和状态。左侧不可见、字段为空/错误、标题不合规或候选有歧义都立即阻断。
4. **BOUND**：首次写入前核对工作区干净和起始提交。Git Task 分别在保存项目与 Task 顶层解析规范化 Git common dir，要求相同，并用 `git worktree list --porcelain` 确认当前顶层已登记；非 Git Task 的 cwd 必须等于保存项目完整路径。任一绑定不符时保持零实现，不退化为 plan、Subagent、Local Git checkout 或普通 Worktree。

### 命名、执行与交付

- Task 描述记录 Task 编号、单一结果、实施范围、禁止范围、完成条件、独立 ASCII `feature-summary`、项目名/`projectId`/保存路径、repository identity、起始 branch/ref 与 commit。显示标题与 Git 摘要互不推导。
- Git Task 在自己的独立 Codex Worktree 修改文件，并在首次产品写入前调用生命周期 `start` 建立和登记唯一 `feature-*` 分支；非 Git Task 使用绑定项目的 Local 目录。两者都保护已有修改且不扩大固定范围。
- 用户明确要求且并行策略允许时，可用 `codex/unit-*` sibling Worktree；每个 Subagent 获得互不重叠的 repo-relative 写入所有权。内部 Subagent 不使用用户可见 Task 标题合同，也不占用序号。
- 每个逻辑闭环形成可审查提交；不得提交缓存、构建输出或秘密。Task 不自行合并、推送、发布、签名或提前清理登记资源。
- 交付前核对要求的测试、相关权威文档、全部任务改动提交和干净状态；没有可编译目标或候选不在范围内时明确写 `Not applicable`/`Not run`，不得扩展为构建或完整验收。

### 统一 Task 描述模板

```markdown
目标：
完成一个明确、可独立验收的结果。

Task 绑定：
- Task 编号、单一结果与完整 `Task {序号} | 已分配 | {单一结果}` 标题。
- Git feature summary；非 Git 为 Not applicable。
- Codex 项目名、projectId、保存项目完整路径。
- repository identity、起始分支/ref 和提交；非 Git 为 Not applicable。

工作方式：
- Git 项目使用独立 Worktree 并由生命周期 start 建立/登记 feature-*；非 Git 使用绑定项目 Local。
- 先读 AGENTS.md 与精确事实源；保护已有修改，不扩大范围。

当前事实：
- 已确认版本、环境和阻断；只报告真实 threadId 或 SETUP_PENDING。

实施范围与禁止事项：
- 列出允许修改的模块、预期结果，以及不可覆盖、删除、合并、发布或声明通过的内容。

验收标准：
- 适用目标编译通过；否则 Not applicable。
- 非空测试通过；真实目标行为按范围验证。
- 文档同步、改动已提交、git status 干净。

交付：
报告分支、提交哈希、实际验证、未执行项和剩余风险；不自行合并默认主分支。
```

## 策略迁移与后续切换

本文件只承认 `schema_version: 4`、当前五个能力策略字段和发布后动作字段，不兼容、不推断任何更早 schema 或旧字段名，包括 `milestone_smoke`、`milestone_e2e`。不合规文件必须由用户重新走五项确认，一次性物化为 schema v4。

初始化后的永久策略变更必须由用户明确确认，同时更新 `confirmed_by`、`confirmed_at`，并在当日 ADR 记录前后值、原因、影响与恢复条件。同值请求按幂等 no-op 处理，不重写元数据或 ADR。切换 `user_owned_tasks` 只影响之后的结果边界，不迁移、中断、删除或改挂当前与既有 Task/Worktree。

## 日常开发、构建与记录

- 日常开发直接实施，只运行本次变化需要的非空单元/回归测试；本策略不自动触发 Work Plan、全仓检查、构建、冒烟、E2E 或验收。
- Windows GUI 普通本地试包使用 `$desktop-build-tauri-local-install`，不是发布候选，不要求 clean HEAD、发布上下文、E2E、性能、签名、提交或安装。
- 显式发布候选必须在测试或编译前为本候选解析 E2E；当前请求明确时复用，否则询问一次并可把 `e2e_hint` 作为建议。GUI 发布另由 `$desktop-prepare-release` 逐次解析性能与适用签名选择并写入发布上下文；不得跨发布复用。
- 候选构建/E2E/验收事实只进入忽略的 `release/` 原子证据与最终回复。真实渠道发布成功或独立回顾性审计等事件才创建相应 tracked 记录；普通维护不预建 Product Spec、Changelog、Product Status、Work Plan 或 Verification。
- 反馈渠道、发布渠道和产物格式仍为未知；任何外部地址、凭据、签名或渠道能力必须从后续明确事实取得。

## 执行优先级

1. 安全、已批准产品范围与渠道硬要求。
2. 当前请求明确约束和当前候选的选择。
3. 本文件持久策略；`e2e_hint` 只提供建议默认值。
4. 基于接口与真实产物的适用性判断。
5. 仍会改变范围且无法可靠判断时询问一次。
