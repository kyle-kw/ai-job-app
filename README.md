# 求职舱

本地优先的 AI 求职桌面助手：整理 BOSS 岗位、维护可信主简历、分析岗位匹配、生成报告、招呼语和面试准备建议。

## 公开 Beta 支持范围

- Windows 10/11 x64：发布 NSIS 安装包，按当前用户安装。
- macOS 12+ Intel：发布 DMG。
- macOS 12+ Apple Silicon：发布 DMG。
- BOSS 功能需要用户自行安装 Google Chrome；应用不会下载浏览器。

`v0.2.0` 是首个公开 Beta 和首个包含 updater 的版本。`v0.1.7` 用户需要手动安装一次 `v0.2.0`；从 `v0.2.1` 起验证应用内更新。

当前公开 Beta 不包含 Windows/macOS 平台代码签名和 Apple 公证，因此 SmartScreen 或 Gatekeeper 可能显示来源警告。更新包仍使用 Tauri updater 独立签名验证，签名失败的包不会安装。

## 隐私与数据

- 首次启动必须先确认离线可读的隐私与使用说明；确认前不检查更新、不访问 BOSS、不测试模型。
- SQLite、简历、岗位和配置默认保存在应用数据目录。
- API Key 保存在 Windows Credential Manager 或 macOS Keychain，不写入 SQLite 和日志。
- BOSS 使用独立 Chrome Profile，不读取普通 Chrome Profile，也不绕过验证码。
- 无遥测、无自动崩溃上传。
- `.aijobbackup` 是不加密的 SQLite 备份，包含简历和岗位数据，但不包含 API Key 或 BOSS Cookie。
- 普通卸载默认保留用户数据。彻底卸载前，请在“设置 → 数据生命周期”执行“清除全部数据”。

详见 [PRIVACY.md](PRIVACY.md)、[TERMS.md](TERMS.md)、[SECURITY.md](SECURITY.md) 与 [SUPPORT.md](SUPPORT.md)。

## 开发环境

构建工具链固定为：

- Node.js `22.23.1`
- Rust `1.96.0`
- Python `3.13.6`
- uv `0.11.24`

```powershell
npm ci
$env:UV_CACHE_DIR = "$PWD\.uv-cache"
uv sync --project sidecar --locked --no-install-project
npm run tauri:dev
```

浏览器演示可运行 `npm run dev`。演示模式使用内置数据，更新安装、文件备份/恢复和诊断导出明确标记为仅桌面版可用，且不会修改本地系统数据。

## 验证

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

## 构建安装包

```powershell
npm run sidecar:build
npm run tauri:bundle
```

`sidecar/pyproject.toml` 是 Python 依赖的唯一声明来源，`sidecar/uv.lock` 必须与其一致。npm、Cargo 和 uv 构建均使用已提交的 lockfile。

## 发布

推送 `v*` tag 会执行质量检查、三平台构建、updater 签名、校验和与 SBOM 验证，然后先创建 Draft Release，上传 NSIS、DMG、macOS updater 包和 `latest.json`，最后公开 Release。已存在的 tag 或 Release 会直接失败，发布物不可覆盖。

版本更新使用：

```powershell
npm run version:set -- 0.2.1
```

该命令同步 `package.json`、lockfile、Cargo、Tauri 和 sidecar 版本；`npm run release:verify` 校验 tag、版本和 `CHANGELOG.md` 标题一致。
