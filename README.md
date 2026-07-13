# 求职舱

本地优先的 AI 求职桌面助手：抓取并去重 BOSS 岗位、用模型批量提取岗位与公司信息、生成岗位数据报告、导入和维护可信主简历、评估岗位匹配度、审核专岗简历修改，并生成招呼语与面试准备建议。

## 开发环境

要求：Node.js 22+、Rust stable、Python 3.12+、Chrome。

```powershell
npm install
python -m venv sidecar\.venv
$env:UV_CACHE_DIR = "$PWD\.uv-cache"
uv pip install --python sidecar\.venv\Scripts\python.exe -r sidecar\requirements.txt
npm run tauri:dev
```

只检查 Web UI 时运行 `npm run dev`。浏览器模式使用内置演示数据；Tauri 模式连接 Rust、SQLite、系统钥匙串和 Python sidecar。

## 验证

```powershell
npm run check
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets -- -D warnings
sidecar\.venv\Scripts\python.exe -m unittest discover -s sidecar\tests -v
```

## 构建安装包

```powershell
npm run sidecar:build
npm run tauri:bundle
```

`scripts/build_sidecar.py` 会为当前 Rust target 构建 PyInstaller 单文件程序。CI 在 Windows 和 macOS 上分别生成 sidecar 与安装包，最终用户无需单独安装 Python 或 RenderCV。

当前公开构建产物会在文件名中标记 `unsigned`。Windows SmartScreen 或 macOS Gatekeeper 可能因此显示警告；面向正式分发的构建必须由发布者另外配置 Windows 代码签名证书以及 Apple Developer 签名与公证凭据。

## 数据与安全

- SQLite、导入文件、导出 PDF 和 UTF-8 岗位报告位于 Tauri 应用数据目录。
- API Key 保存到 Windows Credential Manager 或 macOS Keychain，不写入 SQLite 和日志。
- 设置页“测试连接”是只读操作；只有“验证并保存”会在连接成功后更新配置和钥匙串。
- 简历导入支持 PDF、DOCX、YAML、YML，单文件上限为 25 MiB。
- BOSS 抓取只由用户主动触发，使用独立 Chrome Profile，不绕过验证码；任务结束后自动关闭专用 Chrome。
- 专岗简历只允许引用主简历中已确认的事实，并在用户审核、接受修改后生成新版本。
- 遥测默认且始终关闭。
- 自定义模型允许 HTTP，但必须逐个提供商明确确认；HTTP 会以明文传输 API Key 和请求内容，优先使用 HTTPS。
- WebView CSP 因现有动态样式保留 `style-src 'unsafe-inline'`；模型网络请求由 Rust 发出，不依赖 WebView 的远程 `connect-src`。
