# 区域变化监测器（Tauri 2）

轻量 Windows 桌面原型：记录一个屏幕矩形的基准画面，后台按间隔采样，检测到一定比例像素持续变化超过设定时间后发送 Windows 通知。

## 参数
- X/Y/宽度/高度：检测区域屏幕坐标
- 轮询间隔：最低 100 ms，越大越省资源
- 颜色通道变化阈值：单个 R/G/B 通道的差异阈值（0–254），超过阈值的通道字节占比大于 3% 时开始累计变化时间
- 持续变化时间：变化持续多久才通知，避免瞬时误报

## Windows 便携版通知

正式版以 Windows 图形应用运行，启动时不会额外打开终端窗口。开发运行保留控制台日志。

点击“测试通知”可单独检查通知功能，无需开始屏幕监测。首次发送时，应用会为当前用户注册 `com.local.regionwatch` 通知身份，并把通知图标保存到 `%LOCALAPPDATA%\com.local.regionwatch\notification.ico`，无需管理员权限。移动 exe 后仍使用同一通知身份。此注册和图标会保留，删除便携版 exe 不会自动移除它们。

应用直接调用 Windows 通知 API，报告系统禁用状态、同步发送错误和异步失败。“已提交给 Windows”仅表示系统接受请求；通知横幅仍可能受勿扰模式、专注助手及系统通知设置影响。

## 验证和构建

```powershell
node tests/stop-button.cjs
cargo test --manifest-path src-tauri/Cargo.toml --lib
# 会注册当前用户的通知身份并发送一条真实通知
cargo test --manifest-path src-tauri/Cargo.toml --lib windows_notification_smoke -- --ignored --nocapture
npm run tauri build -- --no-bundle
```

## 开发运行
联网后在项目根目录执行：

```powershell
npm install
npm run tauri dev
```
