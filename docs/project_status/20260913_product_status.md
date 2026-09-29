# 产品状态：2026-09-13

- 项目：毕方内容生产助手（Bifang Content Production Assistant）
- 负责人：arden
- 当前版本：`0.2.0`
- 目标版本：`0.2.0`
- 状态：MVP 功能与开发级 macOS Fixture E2E 已完成；未进入渠道发布
- 产品规格：已批准
- 活动变更：`FEATURE-BATCH-AI-CONTENT-WORKBENCH-MVP`

## 已完成

- 用户已批准以本地飞书式 Grid 为中心的批量 AI 内容工作台设计。
- 已明确首版包含固定文章 SOP、Prompt 模板、可插拔真实 AI、持久化、阶段级重试和人工审核。
- 已明确飞书同步、内容平台自动发布和通用工作流不进入首版。
- MVP-01 至 MVP-03 已完成：shared core 承载任务、文章包版本、固定 SOP、阶段尝试/来源、重试、重新生成、编辑失效和批准保护；Tauri 适配器提供 SQLite 持久化/恢复、系统凭据库引用和 OpenAI-compatible 文本/图片调用路径；GUI 提供 Grid 批量工作台、详情编辑/审核、Prompt 模板、AI 服务配置和运行记录。
- 产品化 CLI 最小契约已完成：只读 `sop describe` 直接输出 shared core 的四阶段顺序、执行前置依赖、产物语义和审核门禁；Draft 阶段的中性状态入口不再作为命令或兼容别名，其 core API 与 GUI 启动元数据链路也已移除。
- 版本门禁已把当前版本事实更新为 `0.2.0`。
- 产品代码基线的 Rust workspace 188/188（CLI 7、core 44、GUI 137）通过，后续前端收口的 Vitest 12 个文件 77/77、typecheck、lint 与 production frontend build 通过。产品化 CLI 的独立 Worktree 产品代码/文档提交 `3ce780988d6a48ee35636c7af1f25ec2883b6901`、`9eb2c7d7e4c684b6abefed3948c3158c7ea0e77a` 已分别映射为主功能分支提交 `4eeb7836627654766beb3ef0cb84fc311313d173`、`941e5e3c86a6ed5c3854a1e5fb80cef18ac59478`；残留清理/证据提交 `291c2a1ac6ef6deb9cdf92ccb3533d6ba7ba6d45`、`8fb55935e0104cdf10cddd351567cd6e20140fbf` 已分别映射为 `f9abf4224186ec47fffa5f268e4840abbec7c45c`、`bd50dbc37b6553c698e19c353c052f8ac89fcb4e`。主控在该合入 HEAD 上复跑 Rust workspace 190/190（CLI 9、core 44、GUI 137）、前端 12 个文件 77/77、typecheck 与 lint，全部通过。
- 从产品代码提交 `9dd96eb47fd85d5ed4da58722470e021ec0882a6` 全新构建的 macOS arm64 debug Tauri `.app`，在隔离 Fixture 数据库中完成 16/16 个必需真实窗口场景；最终 35 张独立 JPEG 的文件名、字节、尺寸和 SHA-256 与清单逐项一致。
- 同一 80 条补充批次的静态证据明确显示暂停 `11/80` 后恢复运行并推进到 `73/80`；原有不同批次帧不再承担该证明。

## 当前工作

- 已按 [MVP 工作计划](../work_plan/20260913_work_plan.md) 完成功能、产品化 CLI 最小契约、自动化回归、macOS 开发级 Fixture E2E 和文档/Git 收口。主控在集成验证提交 `2a658adb4f0187001ad88f4b9262a5ec145cd569` 上取得代码与文档双重独立复核的无 P1/P2 结论，Task 1 已关闭；本状态不启动真实 Provider、候选构建或发布工作。

## 当前阻断

- 当前 MVP 范围没有阻断。
- 真实 AI 联网与系统凭据库故障尚未宿主验证；需要用户另行授权并提供测试 Provider/凭据。缺少这些事实不阻断开发级 Fixture E2E，但不能宣称真实供应商调用已验证。
- Windows 尚未构建或运行，macOS 系统剪贴板拒绝分支未强制制造；两者均为公开的剩余风险。
- 当前没有发布候选；签名、公证、推送、tag、上传和渠道发布均未执行。

## 下一步

- 用户可另行授权真实 Provider 冒烟，并提供隔离测试账号、服务地址与凭据；执行方需保持密钥不进入数据库、日志、截图或回复。
- 用户可另行确定发布渠道/产物并授权发布准备、跨平台候选构建与验收；在相应门禁完成前不得把本次开发级结果称为发布就绪。
