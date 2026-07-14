# Third-party notices

求职舱包含或打包了开源依赖。完整依赖版本以 `package-lock.json`、`src-tauri/Cargo.lock` 和 `sidecar/uv.lock` 为准，发布流水线同时生成 CycloneDX SBOM。

- `boss-zhipin-scraper` 衍生代码：MIT，版权声明见 `sidecar/vendor/BOSS_SCRAPER_LICENSE`。
- Tauri 及官方插件：Apache-2.0 / MIT。
- Svelte、SvelteKit、Vite 及前端依赖：适用各项目许可证。
- Rust crates：适用各 crate 的许可证。
- RenderCV、Typst、PyInstaller、pypdfium2、Pillow、python-docx 及 Python 依赖：适用各项目许可证。
- RenderCV 打包字体和图标资源：适用其随包许可证与字体许可证。

发布前必须依据锁文件和 SBOM 复核许可证，不得仅依赖本摘要。
