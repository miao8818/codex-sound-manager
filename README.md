<div align="center">
  <img src="icons/app-icon.png" width="96" alt="Codex 提示音管理器图标" />
  <h1>Codex 提示音管理器</h1>
  <p>让 Codex 在每次任务完成时播放你喜欢的提示音。</p>
  <p>
    <strong>简体中文</strong> · <a href="README_EN.md">English</a>
  </p>
  <p>
    <img alt="Platform" src="https://img.shields.io/badge/platform-Windows-2563eb" />
    <img alt="Tauri" src="https://img.shields.io/badge/Tauri-2-24c8db" />
    <img alt="License" src="https://img.shields.io/badge/license-MIT-16a34a" />
  </p>
</div>

![应用界面](docs/images/app-screenshot.png)

## 功能亮点

| 功能 | 说明 |
|---|---|
| 自动发现 Codex | 扫描 `CODEX_HOME` 和当前用户的 `~/.codex` |
| 全局生效 | 一套设置覆盖当前 Windows 用户的所有 Codex 对话和项目 |
| 自定义声音 | 支持 WAV、MP3、FLAC、OGG、M4A、AAC，最大 50 MB |
| 播放次数 | 每次任务完成可连续播放 1–10 次 |
| 原创默认音频 | `sounds/default-notification.wav` 随项目公开分发 |
| 安全修改配置 | 使用 `toml_edit` 保留原 TOML 的注释、顺序和其他字段 |
| 保留已有回调 | 兼容 Codex Computer Use 的 `--previous-notify` 包装 |
| 可随时恢复 | 移除工具配置时恢复安装前的原通知回调 |
| 安静运行 | Release 和通知模式不显示黑色控制台窗口 |

## 下载与使用

### 使用安装包

1. 从 GitHub Releases 下载最新的 Windows 安装包。
2. 完成安装后打开 **Codex 提示音管理器**。
3. 确认页面顶部已发现 Codex。
4. 设置启用开关、播放次数和音频文件。
5. 点击 **应用到 Codex**。
6. 完整退出并重新打开 Codex。

### 使用便携版

从 GitHub Releases 下载便携 ZIP，完整解压后直接运行：

```text
codex-sound-manager.exe
```

也可以双击同目录的 `Run-Portable.vbs`。源码目录中的这个入口也会自动寻找 `target\release` 下的正式版程序。

## 从源码启动

### 一键启动开发版

双击：

```text
Start-Dev.cmd
```

或在 PowerShell 中执行：

```powershell
npm install
npm run tauri -- dev
```

开发模式会保留一个终端窗口，用于 Vite 热更新和 Rust 编译日志，这是正常现象。正式 Release EXE 和任务完成通知不会显示该窗口。

## 一键打包发布包

双击：

```text
Build-Release.cmd
```

或执行：

```powershell
.\scripts\Build-Release.ps1
```

脚本会依次生成图标和默认声音、安装前端依赖、执行 TypeScript 检查、Rust 单元测试、Clippy 检查并构建 Tauri Release。

构建产物：

| 产物 | 路径 |
|---|---|
| 便携 EXE | `target\release\codex-sound-manager.exe` |
| NSIS 安装包 | `target\release\bundle\nsis\` |
| 便携 ZIP | `target\release\bundle\portable\CodexSoundManager_1.0.0_x64-portable.zip` |

构建环境需要 Node.js、Rust、Microsoft C++ Build Tools 和 WebView2。

## 默认声音与自定义声音

项目内默认提示音：

```text
sounds\default-notification.wav
```

该音频由 `scripts/generate_default_sound.py` 原创生成，峰值约为 `-1.1 dBFS`，可随 MIT 许可公开分发。

用户选择自定义音频后，程序会复制一份到：

```text
%LOCALAPPDATA%\CodexSoundManager\sounds\
```

点击界面中的 **恢复默认** 即可重新使用项目内置声音。

## 配置与隐私

- Codex 设置：`%CODEX_HOME%\config.toml`
- 首次配置备份：`%CODEX_HOME%\config.toml.codex-sound-manager.bak`
- 工具设置：`%LOCALAPPDATA%\CodexSoundManager\settings.json`
- UTF-8 运行日志：`%LOCALAPPDATA%\CodexSoundManager\notifier.log`

程序不会上传 Codex 配置、自定义音频或日志，也不会读取对话内容。Codex 传入的任务结束参数只用于继续转发已有通知回调。

## 常见问题

### 为什么其他对话没有声音？

修改 `config.toml` 后需要完整重启 Codex。只关闭当前窗口但保留后台进程，其他已打开任务仍可能使用旧配置。

### 为什么开发版有黑色窗口？

开发模式需要终端承载热更新和编译日志。Release EXE、安装版和 `--notify` 模式均使用 Windows GUI 子系统，不显示黑色控制台窗口。

### 为什么听不到声音？

先在工具中点击 **试听**。若试听也没有声音，请检查 Windows 音量合成器、默认输出设备以及音频文件是否仍存在。

### 移动程序后为什么失效？

Codex 配置保存的是 EXE 的绝对路径。移动便携版后，重新打开工具并点击 **应用到 Codex** 即可更新路径。

## 技术栈

- Tauri 2 + Rust
- React 19 + TypeScript
- Tailwind CSS + Shadcn 风格组件
- Radix UI + Lucide 图标
- rodio + Symphonia 音频解码
- toml_edit 配置编辑

## 项目结构

```text
CodexSoundManager/
├─ frontend/             React 设置界面
├─ src/                  Rust/Tauri 核心
├─ sounds/               默认提示音
├─ assets/               Noto Sans CJK SC 字体
├─ docs/images/          截图与交流群二维码
├─ scripts/              图标、声音和发布脚本
├─ Start-Dev.cmd         一键开发启动
├─ Build-Release.cmd     一键发布构建
└─ Run-Portable.vbs      无控制台启动便携版
```

## 交流群

扫码加入交流群，反馈问题、交流 Codex 使用经验：

<p align="center">
  <img src="docs/images/community-qr.jpg" width="360" alt="Codex 提示音管理器交流群二维码" />
</p>

## 许可证

项目代码和原创默认提示音使用 [MIT License](LICENSE)。Noto Sans CJK SC 及其他第三方组件许可见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
