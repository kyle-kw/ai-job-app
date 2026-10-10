# Third-party notices

求职舱包含或打包了开源依赖。完整依赖版本以 `package-lock.json`、`src-tauri/Cargo.lock` 和 `sidecar/uv.lock` 为准，发布流水线同时生成 CycloneDX SBOM。

- `boss-zhipin-scraper` 衍生代码：MIT，版权声明见 `sidecar/vendor/BOSS_SCRAPER_LICENSE`。抓取与登录链路参考本地上游 v2.3.0（提交 `46e2965`），采用 Network 被动捕获与滚动加载；保留本应用的流式入库、5 页上限、离线城市校验、详情容错和专用浏览器安全清理，并隔离前台人工登录与后台登录探测。新增岗位由应用数据库判定。
- Tauri 及官方插件：Apache-2.0 / MIT。
- Svelte、SvelteKit、Vite 及前端依赖：适用各项目许可证。
- Rust crates：适用各 crate 的许可证。
- RenderCV、Typst、PyInstaller、pypdfium2、Pillow、python-docx 及 Python 依赖：适用各项目许可证。
- RenderCV 打包字体和图标资源：适用其随包许可证与字体许可证。

发布前必须依据锁文件和 SBOM 复核许可证，不得仅依赖本摘要。
