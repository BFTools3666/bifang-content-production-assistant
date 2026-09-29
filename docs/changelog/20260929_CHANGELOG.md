# 2026-09-29 变更记录

## 变化

- `FEATURE-CLOUDBASE-PUBLIC-SERVICES`（所需版本 `0.2.17`）：GUI 接入已登记 CloudBase 环境的匿名每日活跃统计；统计默认关闭，可在设置页开启或撤回，开启后每个随机安装标识按北京时间每天最多尝试一次。
- 关于页接入 application-version API v1 版本发现，支持未配置、最新、可选更新、强制更新、暂停分发和失败状态；下载地址只由 Rust 可信层保存并交给系统浏览器。
- 强制更新会在应用根部阻断工作台，仅保留打开官方下载页和退出应用；官方 Tauri updater 仍保持空 endpoint、零出站且不自动安装更新。
- 当前仓库不包含运行时版本凭据；未注入应用专属 Key/Secret 的构建明确显示“未配置”并且不会发起版本请求。
- 版本发现出口改为 `https://gateway.kaiwhale.com/tools/version-control/v1`，App Key 固定为 `versionControl`，App Secret 继续由构建宿主安全注入；基址与 Windows x64 完整版本路径的在线探针均得到约定的 HTTP 400。
- `BUG-VERSION-WINDOWS-CODE-EMBEDDED-SECRET`（所需版本 `0.2.18`）：Windows 发布流 code 修正为 `bifang-content-production-assistant-windows`；按负责人明确要求将 App Secret 固定到 Rust 可信层，并取消构建环境变量依赖。

## 创作者工具整合

- `FEAT-BENCHMARK-STYLE-DUAL-EDITION-20260928`（整合所需版本 `0.2.18`）：新增公开作品关键词采集、作品链接与互动指标筛选、作者作品采集、可编辑二创提示词、风格库、多风格应用和草稿导出。SEO 与小红书发行版使用独立应用标识、数据目录和安装程序，首次启动及 Windows 安装向导默认简体中文。
- SEO 保留公司既有公共服务协议。小红书公共服务尚未登记，生产配置保持未配置，避免误用 SEO 版的统计身份或下载地址；具体决定见 `ADR-20260929-005`。
- `BUG-COLLECTION-KEEP-KNOWN-METRICS-20260929`（所需版本 `0.2.19`）：重复采集页面未提供指标时，保留此前观测值；页面明确提供的零值仍可覆盖旧值。
- `BUG-BATCH-CONFIG-ERROR-DETAILS-20260929`（所需版本 `0.2.20`）：批量预检、冻结配置和单阶段生成保留阶段与具体配置错误，缺少默认服务或凭据时提供阶段提示。
- 合并源码版本为 `0.2.20`。源码回归和 GitHub 合并不代替安装包验收；本版本尚未构建 Windows 安装包，也未完成 RPA002/RPA005 真实完整流程测试。


## 四步写作版本

- `FEAT-ARTICLE-WRITING-SOP-20260929`（所需版本 `0.2.20`）：主名称恢复“毕方内容生产助手”；按写作准备、生成文章、检查修改、保存导出组织原功能。业务导航为写文章、找对标、风格库、我的文章；原批量工作台、AI、飞书、提示词和记录保留。
- `BUG-AI-KEY-ONBOARDING-DISCOVERY-20260929`（所需版本 `0.2.21`）：首次使用明确接入 AI，预填 DeepSeek 地址和模型，保存 Key 不自动调用服务；配置读取失败可重试。写作输入和编辑表单跨页面保留，未保存不导出；正文可原样转入原批量流程继续简介、配图与飞书。
- 新版 Windows 安装验收进行中；主分支合并须在 RPA002/RPA005 验证正常后执行。
