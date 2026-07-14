# 公开 Beta 发布手册

## 一次性准备

1. 将 `.secrets/updater.key` 与 `.secrets/updater-password.txt` 分别复制到两个受控的离线位置。不要把任一文件提交到 Git。
2. 在 GitHub Actions Secrets 设置：
   - `TAURI_SIGNING_PRIVATE_KEY`：`.secrets/updater.key` 的完整内容；
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：`.secrets/updater-password.txt` 的完整内容。
3. 确认 `src-tauri/tauri.conf.json` 中的 updater 公钥与 `.secrets/updater.key.pub` 完全一致。
4. 仓库公开前确认全历史 gitleaks 门禁通过，再启用 GitHub Secret Scanning、Dependabot 和 Private Vulnerability Reporting。

丢失 updater 私钥或密码后，已经安装的版本无法再验证新更新。不要轮换 updater 密钥；如需增加平台代码签名，保持 identifier、updater 公钥、更新 URL 和数据格式不变。

## 发布版本

```powershell
npm run version:set -- 0.2.1
$env:REQUIRE_UPDATER_KEY = '1'
npm run release:verify
$env:UV_CACHE_DIR = "$PWD\.uv-cache"
uv lock --project sidecar --check
git tag v0.2.1
git push origin v0.2.1
```

流水线缺少 updater 私钥或密码会直接失败。它只会创建新 Draft Release，不允许覆盖已有 tag/Release；全部安装包、签名、SBOM、校验和及 `latest.json` 上传并核对数量后才公开 Release。

## 首次真实更新验收

- `v0.2.0` 只通过手动安装分发，并在说明中注明未进行平台代码签名/公证。
- 用真实 Windows 10 x64、macOS 12 Intel、macOS 12 Apple Silicon 安装 `v0.2.0`。
- 发布 `v0.2.1` 后，分别验证提示、下载进度、签名校验、安装和重启。
- 三个平台全部升级成功后，才将 `v0.2.1` Release 从 Draft 公开。

真实设备还需覆盖中文路径、Chrome 缺失、离线启动、备份恢复、分项清除与“清除全部数据”。
