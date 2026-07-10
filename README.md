# 求职舱

本地优先的 AI 求职桌面助手：抓取并去重 BOSS 岗位、生成全量岗位数据报告、导入和优化简历、评估岗位匹配度、审核专岗简历改写，并生成招呼语。

## 开发

要求：Node.js 22+、Rust stable、Python 3.12+、Chrome。

```powershell
npm install
python -m venv sidecar\.venv
$env:UV_CACHE_DIR = "$PWD\.uv-cache"
uv pip install --python sidecar\.venv\Scripts\python.exe -r sidecar\requirements.txt
npm run tauri:dev
```

只检查 Web UI 时运行 `npm run dev`。浏览器模式使用内置演示数据；Tauri 模式自动连接 Rust、SQLite、系统密钥库和 Python sidecar。

## 验证与打包

```powershell
npm run check
npm test
cd src-tauri
cargo test
cd ..
npm run sidecar:build
npm run tauri:bundle
```

`scripts/build_sidecar.py` 会为当前 Rust target 构建 PyInstaller 单文件程序。CI 在 Windows 和 macOS 分别生成对应 sidecar 与安装包，最终用户无需安装 Python 或 RenderCV。

## 数据与安全

- SQLite、导入文件、导出 PDF 和 UTF-8 岗位报告位于 Tauri 应用数据目录。
- API Key 保存于 Windows Credential Manager 或 macOS Keychain，不写入 SQLite 和日志。
- BOSS 抓取只由用户主动触发，使用独立 Chrome Profile，不绕过验证码。
- 专岗简历只允许引用主简历中已确认的事实，并在用户接受补丁后生成。
- 默认关闭遥测。
