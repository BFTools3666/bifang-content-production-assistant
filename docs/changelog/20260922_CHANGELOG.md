# 2026-09-22 变更记录

## 变化

- `FEATURE-LARK-CLI-PUBLISHING-TARGET`（所需版本 `0.2.11`）：飞书连接器改为复用官方 `lark-cli` 用户 profile。应用只保存 profile 名称、目标文件夹和发布选项，不再要求或保存飞书 App ID/App Secret；连接测试只读验证 CLI 版本、用户身份和文件夹访问，确认发布前再次验证并分别创建原生 Markdown 文件或上传自包含 HTML。
- 旧直连目标升级后保留配置历史与同步记录，并以新修订停用，等待用户选择 CLI profile 后重新启用；遗留 App Secret 只进入可重试安全清理队列。应用不会自动安装、登录、切换或删除共享 CLI profile。
- `BUGFIX-LARK-CLI-ONBOARDING-GUIDE`（所需版本 `0.2.12`）：连接器页增加五步飞书 CLI 配置向导，可直接检测 CLI、选择已有 Profile、弹出官方二维码创建 Profile/授权用户、粘贴文件夹完整链接并回填发布目标；二维码弹窗提供官方验证链接、等待状态和可终止的取消操作。迁移目标会原位修复，不再误建重复目标；拒绝、过期、超时、取消和配置状态均显示可恢复提示。
