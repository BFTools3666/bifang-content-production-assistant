# 2026-10-09

- `FEAT-BATCH-AI-TITLES-20261009`，required_version `0.2.25`：支持从主题生成标题候选，勾选后加入原文章工作台；复用用户文本 AI，没有密钥时提供接入入口，失败不自动重试。
- `UX-ARTICLE-PRODUCTION-OPERATOR-20261009`，required_version `0.2.26`：首页与批量文章工作台对应；文章主题不限于技术领域；重写主要操作、设置和错误提示，设置四个业务入口改为两列。原单篇、对标和风格流程保留。
- `BUG-UPDATE-CHECK-FEEDBACK-20261009`，required_version `0.2.27`：更新查询显示检查过程并阻止重复点击；后台未登记、网络和协议失败有固定可读原因，失败不宣称最新版。
- `BUG-RELEASE-NOTES-PACKAGING-20261009`，required_version `0.2.28`：双层更新日志校验统一最近十版，正式安装包构建前检查日志、版本和资源映射。

上述为源码变更记录。Windows 安装与真实更新渠道尚未验收，不是正式发布记录。
