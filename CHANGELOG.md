# Changelog

## [0.2.0] - 2026-07-14

### Added

- Tauri 签名更新包、每日更新检查和用户确认安装流程。
- SQLite 自动/手动备份、恢复、分项数据清除和旧 identifier 数据迁移。
- 首次启动隐私确认、关于与诊断信息、脱敏诊断包。
- Windows 10/11 x64 与 macOS 12+ 的公开 Beta 发布配置。

### Changed

- 应用 identifier 更新为 `io.github.kylekw.aijobapp`。
- Windows 公开安装器统一为 current-user NSIS。
- Python sidecar 使用锁定的 uv 依赖和固定工具链构建。

### Security

- 发布产物增加 updater 签名、SHA-256 校验和、SBOM 和全历史密钥扫描门禁。
