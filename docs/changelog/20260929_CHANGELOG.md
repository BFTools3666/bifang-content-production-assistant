# 2026-09-29 变更记录

## 变化

- `FEATURE-CLOUDBASE-PUBLIC-SERVICES`（所需版本 `0.2.17`）：GUI 接入已登记 CloudBase 环境的匿名每日活跃统计；统计默认关闭，可在设置页开启或撤回，开启后每个随机安装标识按北京时间每天最多尝试一次。
- 关于页接入 application-version API v1 版本发现，支持未配置、最新、可选更新、强制更新、暂停分发和失败状态；下载地址只由 Rust 可信层保存并交给系统浏览器。
- 强制更新会在应用根部阻断工作台，仅保留打开官方下载页和退出应用；官方 Tauri updater 仍保持空 endpoint、零出站且不自动安装更新。
- 当前仓库不包含运行时版本凭据；未注入应用专属 Key/Secret 的构建明确显示“未配置”并且不会发起版本请求。
- 版本发现出口改为 `https://gateway.kaiwhale.com/tools/version-control/v1`，App Key 固定为 `versionControl`，App Secret 继续由构建宿主安全注入；基址与 Windows x64 完整版本路径的在线探针均得到约定的 HTTP 400。
