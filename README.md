# 求职舱

求职舱是一款本地优先的 AI 求职桌面助手，用于整理 BOSS 直聘岗位、维护可信主简历、分析岗位匹配度，并生成岗位报告、招呼语和面试准备建议。

岗位、简历和统计数据默认保存在本机。只有在用户主动使用 AI 功能时，完成当前请求所需的内容才会发送到用户配置的模型服务。

## 核心能力

- 使用独立 Chrome Profile 登录 BOSS 直聘，抓取岗位列表与详情并去重保存。
- 通过关键词、城市、薪资、公司规模和匹配度筛选本地岗位库。
- 导入 PDF、DOCX、YAML 或 YML 简历，维护结构化主简历和事实清单。
- 基于简历事实分析岗位匹配度，生成招呼语、专岗建议和面试准备内容。
- 汇总岗位数量、薪资、经验、技能、地区和公司分布，生成可导出的数据报告。
- 在本地保留任务记录、简历版本和自动备份，支持诊断与数据清理。

## 快速使用

1. 从 [GitHub Releases](https://github.com/kyle-kw/ai-job-app/releases) 下载适合当前系统的安装包并安装。
2. 安装 Google Chrome。BOSS 功能使用 Chrome，但不会读取日常 Chrome Profile。
3. 首次启动时阅读并确认隐私与使用说明。
4. 在初始化向导中配置一个 OpenAI 兼容模型：填写 Base URL、模型名称和自己的 API Key，然后完成连接验证。
5. 打开 BOSS 专用 Chrome，在 5 分钟内完成登录。验证成功后专用窗口会自动关闭。
6. 导入现有简历，或从空白模板建立主简历。
7. 在首页或岗位库开始搜索。每次抓取前都会重新检查登录状态；如果出现登录界面，完成登录后任务会自动继续。

初始化完成后，首页会自动切换为求职工作台，展示岗位总数、新增岗位、优先机会和最近市场观察。BOSS 与模型配置仍可在“设置”中维护。

## 支持范围

- Windows 10/11 x64：NSIS 安装包，按当前用户安装。
- macOS 12+ Intel：DMG。
- macOS 12+ Apple Silicon：DMG。
- BOSS 功能要求用户自行安装 Google Chrome；应用不会下载浏览器。

当前公开 Beta 不包含 Windows/macOS 平台代码签名和 Apple 公证，因此 SmartScreen 或 Gatekeeper 可能显示来源警告。Tauri updater 更新包仍使用独立签名验证，签名失败的包不会安装。

## 隐私与本地数据

- 首次启动必须先确认离线可读的隐私与使用说明；确认前不检查更新、不访问 BOSS、不测试模型。
- 设置中可关闭自动检查更新；关闭后启动时不会访问 GitHub Releases，仍可手动检查。应用不会自动下载或安装更新。
- SQLite 数据库、简历、岗位和应用配置默认保存在应用数据目录。
- API Key 保存在 Windows Credential Manager 或 macOS Keychain，不写入 SQLite 和日志。
- BOSS 登录状态保存在独立 Chrome Profile 中，不读取普通 Chrome Profile，也不绕过验证码。
- 应用不启用遥测，也不自动上传崩溃信息。
- `.aijobbackup` 是未加密的 SQLite 备份，包含简历和岗位数据，但不包含 API Key 或 BOSS Cookie。
- 普通卸载默认保留用户数据。彻底卸载前，请在“设置 → 数据生命周期”执行“清除全部数据”。

生产构建只提供自定义 OpenAI 兼容模型配置，不使用内置共享 Key。开发模式保留额外的模型预设用于本地调试。

详见 [PRIVACY.md](PRIVACY.md)、[TERMS.md](TERMS.md)、[SECURITY.md](SECURITY.md) 与 [SUPPORT.md](SUPPORT.md)。

## 开发环境

项目固定使用以下工具链：

- Node.js `22.23.1`
- Rust `1.96.0`
- Python `3.13.6`
- uv `0.11.24`

克隆仓库后安装依赖：

```powershell
npm ci
$env:UV_CACHE_DIR = "$PWD\.uv-cache"
uv sync --project sidecar --locked --no-install-project
```

## 本地运行

启动完整 Tauri 桌面应用：

```powershell
npm run tauri:dev
```

只运行浏览器演示：

```powershell
npm run dev
```

浏览器演示使用内置数据。更新安装、文件备份/恢复和诊断导出会标记为仅桌面版可用，不会修改本地系统数据。

## 测试与检查

```powershell
npm run release:verify
npm run check
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
$env:UV_CACHE_DIR = "$PWD\.uv-cache"
uv run --project sidecar --locked python -m unittest discover -s sidecar\tests -v
```

`package-lock.json`、`src-tauri/Cargo.lock` 和 `sidecar/uv.lock` 必须与各自依赖声明保持一致。

## 本地构建安装包

```powershell
npm run tauri:bundle
```

该命令会先根据 `sidecar/pyproject.toml` 和锁文件构建 Python sidecar，再执行 Tauri release 构建并生成当前平台的安装包。需要单独调试 sidecar 打包时，可运行：

```powershell
npm run sidecar:build
```

生产前端和 Tauri release 后端都会限制为自定义 OpenAI 兼容模型配置；`tauri:dev` 保留开发模式预设。

## 发布

推送 `v*` tag 会执行质量检查、三平台构建、updater 签名、校验和与 SBOM 验证，随后创建 Draft Release、上传安装包和 `latest.json`，最后公开 Release。发布物不可覆盖，已存在的 tag 或 Release 会导致流程失败。

版本更新使用：

```powershell
npm run version:set -- 0.2.1
```

该命令会同步 `package.json`、lockfile、Cargo、Tauri 和 sidecar 版本。更完整的发布操作见 [docs/RELEASE.md](docs/RELEASE.md)。

## 致谢

求职舱建立在许多优秀的开源项目之上，特别感谢：

- [boss-zhipin-scraper](https://github.com/eatmoreduck/boss-zhipin-scraper)：BOSS 直聘抓取与 Chrome CDP 实现参考。
- [RenderCV](https://github.com/rendercv/rendercv)：结构化简历和 PDF 渲染能力。
- [Tauri](https://github.com/tauri-apps/tauri)：跨平台桌面应用框架。
- [Svelte](https://github.com/sveltejs/svelte) 与 [SvelteKit](https://github.com/sveltejs/kit)：前端组件和应用框架。
- [Vite](https://github.com/vitejs/vite)：前端开发与构建工具。
- [Tailwind CSS](https://github.com/tailwindlabs/tailwindcss)：界面样式系统。
- [Lucide](https://github.com/lucide-icons/lucide)：界面图标。
- [Typst](https://github.com/typst/typst)：RenderCV 使用的排版与 PDF 生成基础。

完整依赖、许可证说明和发布前复核要求见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

## 许可证

本项目使用 [MIT License](LICENSE)。
