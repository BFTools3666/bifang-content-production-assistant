# 产品状态索引

本目录只在重要阻断、跨会话交接、用户明确要求或真实渠道发布后的受管记录事件中保存当前阶段、已完成事项、待确认项与下一步。普通开发、候选构建、E2E、完整验收和就绪复核不逐次写 Product Status。

## 当前状态

- 项目：毕方内容生产助手（Bifang Content Production Assistant）
- 负责人：arden
- 日期：2026-09-13
- 版本：`0.2.0`
- 状态：MVP 功能与开发级 macOS Fixture E2E 已完成；未进入渠道发布
- 产品规格：`Approved`
- 目标平台：Windows、macOS
- 接口：CLI、GUI
- 日期状态文件：[2026-09-13 产品状态](20260913_product_status.md)

当前版本与接口事实分别以根 `Cargo.toml`、`README.md`、`docs/AGENT_POLICY.md` 和 `docs/GUI_APP_PROFILE.md` 为准；产品目标与范围以已批准的 [Product Spec](../product_spec/20260923_product_spec.md) 为准。MVP 功能与开发级验证结论汇总于 [MVP 工作计划](../work_plan/20260913_work_plan.md)；发布候选、签名、公证、Windows 验收和渠道发布均未执行。

## 写入规则

- 事件触发后使用 `YYYYMMDD_product_status.md`，日期取状态更新时区的自然日；同一天只维护一份完整有效快照。
- 新自然日写入时读取上一份状态，把仍有效事实与当日变化综合重写；只保留日期最新文件，历史由 Git 版本控制承担。
- 阻断解除或发布完成并不自动证明产品可用；每个声明仍须引用真实事实源或候选证据。
