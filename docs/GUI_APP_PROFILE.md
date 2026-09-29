# GUI 应用资料

## 项目事实

| 字段 | 值 |
|---|---|
| 中文名称 | 毕方内容生产助手 |
| English name | Bifang Content Production Assistant |
| 项目 ID | `bifang_content_production_assistant` |
| kebab 前缀 | `bifang-content-production-assistant` |
| Tauri identifier | `com.arden.bifang-content-production-assistant` |
| CLI 二进制 | `bifang_content_production_assistant_cli` |
| 负责人 | arden |
| 版本 | `0.2.17` |
| 状态 | MVP 功能与开发级 macOS Fixture E2E 已完成；未发布 |
| 产品规格 | `Approved` |
| 目标平台 | Windows、macOS |
| GUI 技术栈 | Tauri 2 + React + TypeScript + Mantine |

## 初始化配置

以下是本项目唯一的九字段 GUI 配置事实，顺序固定：

```gui-initialization-config
system_tray = enabled
system_notification = enabled
autostart = enabled
about_page = enabled
sponsor_page = enabled
single_instance = enabled
deep_link = enabled
global_shortcut = enabled
sidebar_mode = compact
```

全局快捷键能力采用 Rust-only 插件。当前 MVP 没有快捷键产品动作、默认 chord、OS 注册或快捷键界面；唯一契约为：

```gui-global-shortcut-contract
{
  "schemaVersion": 1,
  "actions": []
}
```

初始非空 binding 数为 0，实际注册数为 0；退出与失败路径只清理本模块拥有的注册。

## 固定 GUI 基线

- system-locale：固定启用，只由 Rust 侧探测系统语言。
- updater：固定启用官方插件基线；继续保持 `NotConfigured`，不配置 endpoint，不发起出站请求。另由 Rust 公共服务适配器按 `ADR-20260929-001` 调用 application-version API v1 做版本发现，不自动下载或安装更新。
- window-state：固定启用；只恢复仍与显示器相交的尺寸、位置和最大化状态，不恢复隐藏、最小化、全屏或装饰状态。
- dialog：固定启用；Builder 注册位于 window-state 后、notification 前，主窗口 capability 精确为 `dialog:default`，不授予通用文件系统权限。
- 系统通知和开机自启虽已安装能力，但设置开关初始均为关闭；不得因能力启用就产生通知或登录项。
- 单实例恢复既有主窗口。受限深链接固定为 `app-bifang-content-production-assistant://restore`，只执行主窗口恢复，不携带产品命令、参数或业务副作用。
- AI Provider 与连接器凭据服务分别固定为 `com.arden.bifang-content-production-assistant.ai-provider`、`com.arden.bifang-content-production-assistant.connector`。不读取或复制旧产品命名空间中的凭据。

## Logo 选择与证据

三个原始候选在生成前绑定标识，并始终按以下稳定顺序同时展示；用户选择前未对候选执行格式、尺寸、色彩、像素、摘要、质量检查或标准化：

| 顺序 | 候选 | 选择结果 | 后置处理 |
|---:|---|---|---|
| 1 | `candidate-1` | 已选择 | 仅此候选被安全解码；原始尺寸 1254×1254，随后标准化为 1024×1024、sRGB、RGBA 无损 PNG |
| 2 | `candidate-2` | 未选择 | 未执行后置检查或标准化 |
| 3 | `candidate-3` | 未选择 | 未执行后置检查或标准化 |

所选候选的固定输出位置：

| 资产 | 路径 | SHA-256 |
|---|---|---|
| 最终母版 | `bifang_content_production_assistant_gui/src-tauri/icons/app-icon-master.png` | `a6e895543fe009282217ba5bf7e75e53bfe4cc1ef8901a0d4d0fa911cda34e19` |
| 运行时 Logo | `bifang_content_production_assistant_gui/public/app-identity/logo.png` | `a6e895543fe009282217ba5bf7e75e53bfe4cc1ef8901a0d4d0fa911cda34e19` |
| 32×32 平台/托盘图标 | `bifang_content_production_assistant_gui/src-tauri/icons/32x32.png` | `3821262d5c16dfcf070c60e8a7cea2600fe95ed279c362db8ac50f29da274556` |
| macOS DMG 背景 | `bifang_content_production_assistant_gui/src-tauri/dmg/background.png` | `c4ef7ef3237d6c296f1b3b9fed066502cfa75afada65276e7adc60a8e86053b7` |

最终母版与运行时 Logo 必须逐字节一致。平台图标由项目本地 Tauri `icon` 命令从最终母版生成；`32x32.png` 必须是普通非符号链接、32×32、8-bit RGBA、非交错且至少含一个非透明像素，并由 `tauri.conf.json` 的 `bundle.icon` 精确引用。DMG 背景为 660×400 PNG，配置使用同尺寸窗口、`appPosition: { x: 180, y: 220 }` 与 `applicationFolderPosition: { x: 480, y: 220 }`。

## UI 标准

赞助页复用的共享品牌媒体，其 `media-manifest.json` 中 `sourceApproval.approvedOn = 2026-08-20` 仅是上游资产来源授权证据，不表示本项目负责人在该日期批准过产品范围、发布或验收。

本项目精确命中以下稳定标准 ID：

- 通用：`tauri-gui-common-v1`
- 侧栏：`tauri-gui-sidebar-compact-80-v1`

compact 侧栏固定为 80px 宽、6px 内容内边距、36px Logo、22px 图标、11px/1.25 全宽居中名称、56px 菜单项、4px 图标与名称间距/垂直 padding、8px 区间距；不提供折叠按钮或固定 `em/ch` 名称盒。功能项从顶部增长，底部依次放置已启用赞助页、固定设置页和已启用关于页。主题与 detailed 侧栏折叠偏好分别使用 `bifang-content-production-assistant:application-color-scheme` 和 `bifang-content-production-assistant:app.sidebar.detailed.collapsed`；详细规则只读取 `docs/design_standards/README.md` 精确路由到的标准。

主窗口在无有效保存状态时使用 1440×900、最小 960×640、居中并防止溢出。动态标题遵循 `{applicationName} v{version} {contactChannel}:{contactValue}`，固定品牌资料提供 `contactChannel=QQ`、`contactValue=2222980`；项目产品反馈渠道仍未另行定义。设置页固定提供应用/版本、中英文和浅色/深色/跟随系统主题，并显示通知、自启和匿名统计开关；匿名统计默认关闭，说明发送字段、时机和撤回方式。

像素、密度或信息架构偏离必须先取得明确批准，并在本文件记录精确差异与关联 ADR。用户已批准 `ADR-20260916-002`：格式页由 Markdown/HTML 上下堆叠改为等宽并列选项卡，每种格式内提供“源码/渲染效果”模式，默认进入 Markdown 的渲染效果。用户同时批准 `ADR-20260916-003`：Grid 收敛为六列固定布局且不再横向滚动，并新增独立“连接器”一级页面和格式页显式同步入口。用户还批准 `ADR-20260916-004`：配图高级操作从详情 Drawer 移入独立大弹窗，Drawer 只保留当前配图预览和明确入口。

## MVP 界面事实

### 导航

- 顶部功能区依次为工作台 `/`、Prompt `/prompts`、AI 服务 `/providers`、连接器 `/connectors` 和运行记录 `/runs`。
- 底部支持区依次为赞助 `/sponsor`、设置 `/settings` 和关于 `/about`；设置始终存在，赞助与关于由当前 profile 启用。
- 导航动作和当前路由状态由各自语义化菜单项拥有，compact 模式不提供折叠控件。

### 工作台与 Grid

- 工作台提供批量粘贴标题、行选择、全选当前结果、标题搜索、总体状态筛选，以及按创建时间、更新时间、标题或状态排序，并在预检后“运行所选”。每次新进程默认按不可变创建时间倒序，同一创建时间按稳定文章 ID 倒序；用户在本进程内可切换其它排序。空行忽略，重复标题保留为独立任务。
- Grid 沿用工作台完整快照并执行客户端分页，派生顺序固定为标题搜索与总体状态筛选、排序、分页；默认每页 `20` 条，只提供 `20`、`50`、`100` 三种每页数量。全选当前结果始终作用于全部筛选结果，不受当前页边界限制，Grid 行只展示当前页切片。
- Grid 固定为“选择、文章信息、正文、简介、配图、格式”六列；文章信息格聚合标题、文章包版本、总体状态、创建/更新时间、错误摘要和明确的“查看详情”动作。正文与简介有产物时在各自单元格显示最多三行缩略内容，点击直接打开对应的可编辑详情页签；配图有产物时显示自适应缩略图，点击打开完整配图页签。四个阶段格在状态徽标下分别提供“生成/重新生成”按钮，只运行所点阶段且不自动继续后续步骤。复选框、缩略预览、状态、单步生成和详情分别由独立语义动作拥有，预览点击不得调用 AI；旧的过期产物可继续预览，但阶段状态必须同时可见。表格使用固定布局与可收缩单元格，Grid 不设置横向滚动容器或固定最小宽度。
- 四个阶段格仅在真实 `queued` 或 `running` 状态显示生成转圈；`running` 依据当前尝试开始时间每秒更新已用秒数，终态固定显示最后一次尝试的实际开始/结束耗时。AI 尝试紧凑显示 Provider 实际返回的总 token，完全缺失时显示“Token 未提供”且不估算；本地格式转换固定显示“0 Token · 本地处理”。读取缩略图、打开详情和后台刷新只显示静态读取状态，不得启动生成动效或计时。
- Grid 配图缩略图只在进入可视区域附近时加载，离开后释放。原生层仅从受管素材目录读取并输出固定 `192×128` 边界内的 PNG 缩略图，同时限制源尺寸、解码内存、并发和返回体；Grid 不接收本地文件路径、远端临时 URL 或完整 2K 图片，完整图仍只在详情 Drawer 中按既有边界加载。
- 单步按钮按固定 SOP 依赖和当前步骤 AI 配置禁用并在按钮文字中说明最早缺失项：正文只需标题，简介依赖正文，配图依赖正文与简介，本地格式需前三阶段完成；目标动作独占 loading，其它生成与重试动作同步锁定。配图单步在调用前确认弹窗展示当前服务修订、模型、尺寸和水印，取消零出站、一次确认最多一次请求，失败后关闭弹窗才能重新确认。
- 活动批次显示状态、进度、完成/失败/总数以及暂停、继续和取消按钮；单行失败不阻断同批其他行，所选失败行可精确重试。
- 总体状态显示草稿、已排队、运行中、待审核、已批准、部分失败、已取消和已中断；阶段状态显示未开始、排队中、生成中、已完成、失败、已过期、已取消和已中断。

### 详情与审核

- 文章详情使用右侧 Drawer，页签依次为输入、正文、简介、配图、格式和历史。
- Drawer 默认宽度为 `620 CSS px`，用户可拖动左侧手柄在 `480 CSS px` 到当前 WebView 宽度之间调整；手柄支持 `ArrowLeft`/`ArrowRight` 微调及 `Home`/`End` 到边界。用户确认的宽度以设备级布局偏好写入 `localStorage` 键 `bifang-content-production-assistant:article-details-drawer-width`；缺失、非法或不可读时回退默认值，窗口临时缩窄只约束可见宽度而不覆盖原偏好。
- Drawer 使用 body Portal 覆盖完整 WebView，避免受 `AppShell.Main` 起点影响而向右裁切；标题、动作组、六个页签、表单、图片和格式内容随当前宽度回流，长文本允许断行，Drawer 内不产生非预期横向溢出。拖拽手柄固定在左边缘，不随正文滚动离开视口，且不得小于产品最小宽度。
- 输入、正文和简介可编辑；正文编辑会使简介、配图和格式过期，简介编辑会使配图过期。详情页保留重新生成整篇、仅重试失败阶段和退回修改；正文、简介、配图与格式页签各自提供与 Grid 相同的“生成/重新生成当前阶段”动作，只执行当前步骤且不自动继续。按钮下方持续说明单步边界，依赖、当前步骤 AI 或运行锁未就绪时显示最早缺失项；配图继续复用同一服务修订与付费确认。只在完整待审核状态批准当前文章包。
- 详情展示 Markdown、HTML、配图、模板/Provider/模型/尝试血缘、阶段尝试、关联批次、被替换文章包和不可变的已批准历史；每条尝试展示实际耗时及输入、输出、总 token 的逐字段明细，Provider 未返回的字段保持“未提供”，本地格式转换标记为 0 Token/本地处理。格式页使用等宽并列的 Markdown/HTML 选项卡，每个格式内切换源码或隔离渲染效果；进入格式页后通过文章 ID 惰性读取当前受管配图，在内存中组装带完整 Data URL 的自包含 Markdown/HTML 复制投影，不写回规范正文或本地路径。源码区折叠图片负载但复制保留完整数据；有图而读取失败时禁止复制缺图降级内容。
- 配图页签在 Drawer 内只展示当前已保存配图预览和明确的“高级编辑配图”入口；点击预览或入口才打开独立高级编辑 Modal。`25%–400%` 缩放、适应窗口、`100%`、拖动平移、全屏、重置、自由裁剪、`1:1`/`4:3`/`16:9`、左右 `90°` 旋转、水平/垂直翻转、撤销/重做和恢复 AI 原图全部只存在于该 Modal，不直接占用 Drawer。Modal 最大宽度为 `1240 CSS px`，保留 `16–32 CSS px` viewport gutter，受 viewport 高度约束并让 body 独立滚动；窄窗继续回流。所有查看动作零写入，未保存关闭高级编辑时先确认，保存未完成时禁止关闭；只有“保存新配图版本”调用写 IPC。
- 过期配图以及目标文章排队/运行时仍可查看，但保存显示稳定禁用原因。保存携带行版本、包版本、AI 图片 attempt 和编辑修订；成功后 Grid 以人工编辑修订刷新缩略图，失败或冲突保留编辑草稿。WebView 只接收 Rust 授权的 data URL、像素、规范和锁，不接收本地路径。
- Rust 从不可变 AI 原图按“裁剪→旋转→翻转”重算，每次保存为受管目录中的新 UUID PNG 和新派生文章包；AI 生成血缘、原图、当前包快照与批准历史均保留。来源限 `25 MiB`、PNG/JPEG/WebP 魔数、完整结构、最大 `4096×4096` 和 `128 MiB` 解码分配；工作区 schema v8 为当前与每份历史图片分别补齐原图引用，schema v9 为文章补齐不可变创建时间并允许阶段尝试保存逐字段 token 用量。

### 云文档连接器

- 独立连接器页面首版展示飞书 CLI 发布目标及新建/编辑区。字段为名称、已有 CLI profile 名称、目标文件夹 Token、默认格式、超时、启停和默认连接器；不收集 App ID、App Secret 或 CLI token。CLI 的安装、profile 配置和用户登录由用户在应用外自行完成；页面只读检测版本、已有 profile 与所选用户身份，不创建、修改或切换共享 profile，也不发起扫码登录。
- 配置保存、启停与删除使用明确反馈；删除必须确认并保留非秘密历史。连接测试按钮只读列取目标文件夹，不创建文件；停用、profile 缺失或未登录时禁用或返回安全状态。
- 格式页在 Markdown/HTML 选项卡上方选择启用的同步目标，每个格式分别提供“同步当前格式”动作。点击先打开确认弹窗，明确文章、格式、连接器、目标文件夹和外部写入；确认后最多上传一次自包含 `.md`/`.html` 文件，失败不自动重试，成功或幂等复用均给出可见反馈。

### Prompt、AI 服务与运行记录

- Prompt 页面分别管理正文、简介和配图模板，展示必需/可选变量，拒绝未知或缺失变量；本地预览只替换示例变量、不调用外部 AI，保存会创建新版本。
- AI 服务页面以正文 AI、简介 AI 和图片 AI 三个 SOP 步骤为一级入口；当前步骤只展示和编辑本步骤的模型、参数、默认服务与适用连接测试。底层服务的名称、API 根地址、密钥状态、启停和删除集中在独立服务连接管理区；同一 OpenAI-compatible 服务可被多个步骤复用，且编辑一个步骤不得覆盖其它步骤配置。图片页另提供仅图片可用的火山方舟 Ark v3 服务类型，显示推理接入点 ID、`2K`、水印、超时和固定 URL 返回协议，正文与简介页不会展示该类型。
- 非秘密配置保存在本地，API 密钥只进入系统凭据库且不回填；保存不联网，正文/简介连接测试发送最小文本请求。图片步骤提供“生成测试图片”：每次先确认可能计费，再以弹窗所示的已保存修订和固定无文章数据 Prompt 真实生成一张，复用正式协议完成下载、大小和格式校验，只在当前弹窗显示内存预览，关闭即清除且不写入文章、素材目录或运行记录；失败不自动重试，结果不确定时明确提示可能已计费。未分配给当前步骤的连接可直接删除；历史执行绑定保留非秘密配置和血缘，但会解除目标 Provider 的凭据引用并安全清理，旧阶段需选择当前 AI 后重新生成。
- 自定义 OpenAI-compatible Provider 地址按用户明确录入的受信任 HTTPS 目标处理，可指向自托管服务；界面明确提示测试或运行会向该地址发送密钥和内容。火山方舟预设保存 `https://ark.cn-beijing.volces.com/api/v3` 根地址并展示实际图片生成端点，返回的临时签名 URL 只由原生层立即下载，不进入配置、持久数据或 WebView；POST 结果未知、生成已成功但下载失败或运行中退出时，禁止普通重试并提示用户手动重新生成，避免重复计费。v6 工作区中仅承担图片阶段、且地址精确为方舟完整生成端点的旧 OpenAI-compatible 当前配置会原地升级为方舟类型；升级保留服务 ID 与凭据引用，不读取或复制密钥，也不改写历史配置或冻结执行绑定。
- 用量解析只读取响应真实字段：Chat Completions 的 `prompt_tokens`、`completion_tokens`、`total_tokens` 映射为输入、输出、总量；OpenAI-compatible 图片读取 `input_tokens`、`output_tokens`、`total_tokens`；火山方舟图片读取其实际返回的 `output_tokens`、`total_tokens`，不把图片计数或像素换算为 token。任何缺失字段保持缺失，不能由其余字段、Prompt 长度、图片尺寸或模型上限推算。Provider 已返回 usage 后即使产物校验、本地保存或方舟临时图片下载失败，原值仍随对应失败尝试保存。
- 运行记录页展示最近一百次批次的状态、完成/失败/总数、开始和结束时间；单步运行另以目标阶段标识并按该阶段收口，活动批次每 500ms 刷新，夹具批次带“测试夹具 · 非真实 AI”标识。

### 页面状态与窄窗行为

- 工作台搜索、状态筛选、排序、分页页码与每页数量、已选行、当前详情及页签、格式页当前 Markdown/HTML 选项卡与源码/渲染模式、Prompt 当前类型，以及 AI 服务当前步骤由应用根 Jotai store 在本进程内跨路由保留；这些纯交互状态不镜像 TanStack Query 数据，也不写入数据库、URL 或浏览器存储。搜索、状态范围、排序或每页数量改变时页码回到 `1`；成功载入后若大于第 `1` 页的当前页为空，只原子回退一次，第 `1` 页空结果、首次加载、后台刷新或错误不得触发回退循环。应用真正退出后排序恢复“创建时间倒序”，工作台分页恢复第 `1` 页、每页 `20` 条，其它值回到各自默认。唯一明确批准的设备级布局偏好是文章详情 Drawer 宽度，按“详情与审核”中的独立键保存，不包含文章、Prompt、Provider、凭据或查询状态。
- 主窗口最小尺寸为 960×640。工具栏与动作组允许换行，指标区在窄窗使用两列，AI 服务与连接器双栏回流为单栏；整个页面和 Grid 均不产生横向滚动，只有运行记录表可以在自身局部容器内横向滚动。
- 工作台明确区分首次加载、保留旧内容的后台刷新、工作区没有任务、筛选无匹配、可重试的首次读取错误，以及保留旧内容的后台刷新错误；后台刷新只在 Grid 局部显示进度或错误，不得用整页 loading、empty 或 error 覆盖既有内容。成功反馈和禁用防重分别可见，所有状态使用文字和语义属性，不只依赖颜色。
- 批次运行与恢复由 Tauri/Rust 适配器驱动。测试夹具只允许在显式开启的 debug E2E 模式和独立临时数据目录中出现，不能作为真实 Provider、正式数据或内容质量验证。

上述既有工作台实现已由从 `9dd96eb47fd85d5ed4da58722470e021ec0882a6` 全新构建的 macOS arm64 debug Tauri `.app` 在隔离 Fixture 数据库中完成 16/16 个必需真实窗口场景；最终 35 张独立 JPEG 的文件名、字节、尺寸和 SHA-256 与清单逐项一致，其中同一 80 条批次明确显示暂停 `11/80` 后恢复运行并推进到 `73/80`。配图基础编辑器在此提交中只执行 core、Rust 适配器和 React 自动化，不继承该早期真实窗口结论。现有结论仅限开发级 macOS Fixture E2E，不代表发布候选、Windows 验证、真实 AI 联网、签名、公证或渠道发布已经完成；强制制造系统剪贴板拒绝为非阻断 `NOT RUN`。
