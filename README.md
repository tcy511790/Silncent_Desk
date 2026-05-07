# 静桌

Windows 桌面透明覆盖层原型。它不替代 Explorer 或任务栏，只在原生桌面壁纸上方显示时间、日期、天气、专注状态、音乐条、命令面板、AI 面板和本地设置。

## 当前启动链路

```text
npm run tauri:dev
-> tauri dev
-> npm run dev
-> Vite http://localhost:1420
-> src/main.tsx
-> src/app/App.tsx
```

Tauri 配置位于 `src-tauri/tauri.conf.json`，前端构建产物位于 `dist/`。

## 当前功能

- 全屏无边框透明 Tauri 窗口，默认鼠标穿透。
- 时间每秒刷新，日期、天气、专注状态显示在信息区。
- `Ctrl+Space` 打开命令面板。
- `Ctrl+T` 打开 AI 面板。
- `Esc` 关闭当前面板并恢复鼠标穿透。
- `Ctrl+Shift+Q` 退出应用。
- 窗口有焦点时，长按 `Space` 打开功能轮盘。
- 命令面板支持搜索、方向键选择、回车执行。
- AI 面板通过 Tauri 后端调用 DeepSeek API，API Key 不写入前端代码。
- 音乐条优先使用 WASAPI loopback 采集系统输出；如果 loopback 失败，会降级为系统音量表驱动的频谱，不再永久静默。
- 壁纸功能支持下一张和切换默认/备用文件夹；启动时不会自动修改系统壁纸。
- 本地配置保存到 `%APPDATA%\Jingzhuo\config.json`。

## 运行

安装依赖：

```powershell
npm install
```

启动桌面应用：

```powershell
npm run tauri:dev
```

只预览前端：

```powershell
npm run dev
```

构建前端：

```powershell
npm run build
```

构建 Windows 可执行文件：

```powershell
npm run tauri:build
```

## 目录说明

- 根目录是当前主项目。
- `new-app/` 是迁移前备份，不作为默认启动入口。
- `legacy/` 和 `legacy_archive/` 只作为历史参考，不参与当前构建。
- `dist/` 是前端构建产物，可删除后由 `npm run build` 重建。
- `src-tauri/target/` 是 Rust/Tauri 构建产物，不应作为源码依据。

## 已知限制

- 全局裸 `Space` 没有启用，避免和系统输入冲突；长按 `Space` 只在窗口有焦点时生效。
- 中国天气 SmartWeatherAPI 正式接口需要申请并按文档签名；当前可用的是城市代码原型模式。
- 音乐条依赖 Windows 默认输出设备；设备不可用或没有系统声音时会进入静默线。
