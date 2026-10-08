# 独立产品范围与兼容约定

- 日期：2026-10-08（Asia/Shanghai）
- 用户要求：将已合并项目中的小红书独立出来，变成两个项目。
- change_id：`MAINT-INDEPENDENT-CREATOR-REPOSITORIES-20261008`
- required_version：`0.2.25`；行为保持的仓库与构建重组，不重置已有版本周期。
- 来源：`https://github.com/BFTools366/bifang-content-production-assistant`
- 基线：`fe0947f3ae08d5f5f6ffc27305d16c83ace55e58`，包含最新 Harness 更新。

## 本产品

本仓库只面向CSDN、百家号的文章。根 `product-edition.json` 固定产品身份；Vite 的前端常量和 Rust 的 `bifang_xhs` cfg 均从该清单生成，不能再通过独立环境变量或 Cargo feature 选择另一个产品。Tauri 基础配置本身就是本产品身份，开发模式不依赖覆盖配置。

## 保留

原 core、CLI、GUI 的业务实现、测试、Harness Skills、统计和版本服务、AI/飞书连接、提示词、配图与排版、草稿数据库格式以及版权和商业许可证。共享内部类型保留 CSDN、百家号、小红书枚举以兼容已有资产；本产品 UI 和采集入口仍按自己的平台边界过滤。

应用 identifier、深链接、AI 凭据服务、飞书凭据服务、统计 app code 和版本查询 code 沿用原同款发行版。小红书尚未登记自己的公共服务，保持未配置状态，不借用文章版的统计或版本服务身份。旧同款安装版升级应读取自己的原工作区，不自动迁移另一款产品数据，不复制 API Key。Windows 安装卸载和真实升级保留情况须在新安装包上验证。

## 独立维护

两仓库各自拥有 README、构建入口、版本状态和之后的 Release。根 Cargo 显式登记 GUI package.json 与 Tauri 配置为版本镜像，后续版本门禁同步更新这些安装版本。Git 历史保留共同来源；后续仅按具体改动选择性同步通用 core/Harness 修复，不整仓覆盖产品身份和版本周期，不强制改写原作者历史。新的运行目录与测试缓存不提交到 Git。

## 本次边界

这次只做源码和构建身份拆分，不新增平台、自动发布、采集方式或 AI 调用。GitHub 归属、远端创建和提交等待用户对目标账号的回答；不因默认选项或等待时间擅自创建公司或个人远端。新 Windows 安装包、安装验收和正式 Release 尚未完成。
