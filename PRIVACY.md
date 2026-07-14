# 求职舱隐私说明

生效日期：2026-07-14；政策版本：`2026-07-14`。

求职舱是本地优先的桌面应用。岗位、简历、分析结果、设置和任务记录默认保存在本机 SQLite 数据库中。API Key 仅保存在 Windows Credential Manager 或 macOS Keychain，不写入数据库、诊断包或应用日志。

只有在用户主动配置并使用 AI 功能时，完成任务所需的岗位和简历上下文才会发送到用户选择的模型服务。模型服务的 Base URL、模型和隐私规则由用户选择的服务提供方负责。自定义 HTTP 地址会明文传输请求和密钥，应用会要求额外确认。

BOSS 功能仅在用户主动操作时启动独立的 Google Chrome Profile，并访问 BOSS 直聘。该 Profile 可能保存登录 Cookie，位置为用户主目录下的 `.boss-zhipin-scraper/chrome-profile`。设置页可单独删除该数据。

用户同意隐私说明后，应用默认每天最多访问一次公开 GitHub Release 地址检查更新；用户可在设置中关闭自动检查，并保留手动检查入口。该请求会向 GitHub 暴露常规网络信息，例如 IP 地址和 User-Agent。应用不会自动下载或安装更新，也不包含遥测、广告、后台行为分析或自动崩溃上传。

自动备份和用户导出的 `.aijobbackup` 包含简历及岗位数据，但不包含 API Key 或 BOSS Cookie。备份不加密，用户应保存在受信任的位置。

设置页支持删除模型密钥、BOSS 登录数据、旧版遗留数据或全部应用数据。普通卸载默认保留用户数据；如需彻底删除，应先在应用内执行“清除全部数据”。用户自行导出的文件不会被自动删除。

问题与数据删除支持请使用 [GitHub Issues](https://github.com/kyle-kw/ai-job-app/issues)，不要在公开 Issue 中上传简历、API Key、Cookie 或未检查的诊断包。
