# RideX

基于 Tauri 2、Rust 和 Vue 3 的APP。

## 开发

```bash
npm install
npm run dev          # 仅启动 Vite 前端
npm run build        # 构建前端 dist
npm run tauri dev    # 启动完整桌面应用
```

检查 Rust：

```bash
cargo check --manifest-path src-tauri/Cargo.toml
``