# 毕方 SEO 文章助手与小红书笔记助手

变更：`FEAT-BENCHMARK-STYLE-DUAL-EDITION-20260928`；原候选最低版本：`0.2.17`；与公司公共服务代码整合时所需版本：`0.2.18`。

## 两个独立工具

| 工具 | 目标平台 | 应用身份 |
| --- | --- | --- |
| 毕方 SEO 文章助手 | CSDN、百家号 | 保留原应用身份，升级保留原文章工作区 |
| 毕方小红书笔记助手 | 小红书 | 独立应用身份、数据目录和系统凭据命名空间 |

两个工具共享原有 Rust core、React/Mantine 和 Tauri 宿主实现。原文章工作台、AI 服务、提示词、配图和排版功能继续保留。小红书新工作区使用笔记提示词，独立保存作品、风格、草稿和用户的 API 设置。

Windows 安装向导及两个工具首次启动均默认简体中文。设置页中的语言选择会保存在本机，重启后继续生效。

## 使用流程

1. 在“AI 服务”填写自己的兼容 Chat Completions 的 API 基础地址、文本模型和 API Key，启用服务。正文模型供二创和风格分析使用。
2. 在“对标采集”输入关键词，或粘贴当前平台的公开作品、作者主页链接。
3. 工具打开可见采集窗口。需要登录时在该窗口完成登录，等作品加载后点击“采集当前页”。返回工具查看作品链接、阅读或播放、点赞、收藏、评论、分享与采集时间。
4. 本次未读到的指标保留以前公开页面已采集的数值；新观测到的数值（包括 0）会更新。未曾采集到的指标标为“未提供”。设置最低指标会排除缺失项。可按作者主页筛选作品。
5. 勾选 1 至 10 件作品，点击“批量采集所选正文”。工具依次打开详情补充平台公开可见正文和指标；遇到登录或验证时停在该页，完成后再次采集。
6. 二创：选择作品进入“二创写作”，填写关键词、自己的材料及要求，修改内置提示词，生成多个标题和完整正文。可编辑、保存、复制、导出 Markdown 或文本。
7. 风格库：选同一作者主页的 3 至 10 件已采集正文的作品，点击“分析所选作品风格”，命名并分析保存。可保存多个对标、编辑应用指令和一键应用；写作页也可切换风格。
8. “原创草稿”保存实际生成的内容、来源、风格、提示词、模型和接口返回的用量。重启后可以继续编辑和导出。

平台未公开的数据不推算。采集窗口读取公开页面的内容；登录和验证码不自动绕过。搜索列表只采集已加载作品，主页可滚动加载更多作品再采集。

## API 与数据

API Key 保存到系统凭据库，不进入 SQLite、源码、生成文稿或交付包。火山方舟模型服务使用方舟 API Key；IAM Access Key 不是此处的模型调用凭据。兼容基础地址示例：`https://ark.cn-beijing.volces.com/api/v3`。模型名称须使用自己账号已开通的 Model ID 或接入点 ID。

生成和风格分析每次只发起一次请求，不自动重复失败的可能计费调用。网页资料作为参考数据，提示词要求原创组织并禁止虚构个人经历、数据或引用。使用前请自行核对事实。

## 从源码构建 Windows 安装包

环境：Windows x64、Rust MSVC 工具链与 Visual Studio C++ Build Tools、Node.js 24.15 或支持版本、pnpm 11.24 或更高。运行：

```powershell
cd bifang_content_production_assistant_gui
pnpm install --frozen-lockfile
cd ..
cargo test --workspace --all-targets --all-features --locked
cd bifang_content_production_assistant_gui
pnpm test -- --maxWorkers=2
cd ..
node tools/build-edition.mjs seo
node tools/build-edition.mjs xhs
```

安装包由 Tauri NSIS 生成，默认在 Cargo target 的 `x86_64-pc-windows-msvc/release/bundle/nsis` 目录。每款构建后立即保存对应安装包再构建下一款。源代码不包含用户的 AI API Key。此脚本生成未签名安装包；代码签名与正式发布渠道可由仓库维护者另行配置。

macOS 的 Windows 交叉构建额外需要 cargo-xwin、LLVM/LLD 和 NSIS，执行 `node tools/build-edition.mjs seo --windows-cross` 或 `xhs --windows-cross`。交叉构建成功不代表实际 Windows 安装测试成功。

## 本次交付的边界

用户明确要求在现有工具基础上制作两个独立安装包，并在 UU Remote 的 RPA002 和 RPA005 安装实测；这构成本次双发行版与本地安装包交付的授权。源码准备供用户自行上传 GitHub；本任务不自动发布远端仓库、标签或公共 Release。

两款工具的安装、真实采集、真实 AI 生成、多个风格应用、编辑导出和退出重启验证都完成后，才能记录交付验收通过。单元测试、编译成功或启动窗口各自只证明对应环节。
