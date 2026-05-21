# TransLens 🛠️

**Windows AI 翻译工具** — 选中文本、一键翻译、即刻呈现。

![](https://img.shields.io/badge/platform-Windows%2010%2B-blue)
![](https://img.shields.io/badge/Rust-1.84%2B-orange)
![](https://img.shields.io/badge/license-MIT-green)

---

## 功能演示

```
                  ┌──────────────────────────────────┐
  选中文本后       │ TransLens                        │
  点击小图标  ──▶  ├──────────────────────────────────┤
                  │ 原文: Hello World                 │
  或              ├──────────────────────────────────┤
  托盘菜单        │ 你好，世界                        │
  → 翻译选中文本  ├──────────────────────────────────┤
                  │ 🔊  📋  🔄               [中文] │
                  └──────────────────────────────────┘
```

- ✅ 系统托盘常驻，右键菜单操作
- ✅ 翻译窗口透明、无边框、置顶
- ✅ DeepSeek 流式翻译（逐字显示）
- ✅ 固定 / 复制 / 朗读 / 切换语言
- ✅ 全功能设置面板

---

## 下载

[🔽 最新 Release](https://github.com/Hi-Barry/translens/releases/latest)

- `TransLens_x64-setup.exe` — NSIS 安装器
- `translens.exe` — 便携版（直接运行）

---

## 快速开始

### 1. 安装

双击安装器，按提示完成安装。启动后系统托盘会出现 TransLens 图标。

### 2. 配置 API Key

右键托盘图标 → **设置** → 填入 DeepSeek API Key：

```
API Key:       sk-xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
Base URL:      https://api.deepseek.com/v1
模型:           deepseek-chat
Temperature:   0.1
```

> 获取 API Key：[platform.deepseek.com](https://platform.deepseek.com)

### 3. 使用

| 方式 | 操作 |
|------|------|
| **托盘菜单** | 右键托盘图标 → "翻译选中文本" |
| **双击托盘** | 双击托盘图标 |
| **自动检测** *(开发中)* | 选中文本后自动弹出浮动按钮 |

流程：

1. 在任何应用中选中要翻译的文本
2. 右键托盘图标 → **翻译选中文本**
3. 翻译窗口弹出，显示原文和流式译文

---

## 配置说明

### 配置文件位置

```
Windows:  %APPDATA%/translens/config.json
Linux:    ~/.config/translens/config.json
```

### config.json 完整格式

```jsonc
{
  // ─── 翻译设置 ─────────────────────────────────
  "target_language": "zh-CN",        // 目标语言: zh-CN | en | ja | ko | fr | de
  "auto_detect_source": true,        // 自动检测源语言

  // ─── 界面行为 ─────────────────────────────────
  "show_overlay_button": true,       // 选中文本后显示浮动按钮
  "overlay_timeout_ms": 5000,        // 浮动按钮超时自动消失(毫秒)
  "close_on_esc": true,              // ESC 键关闭翻译窗口
  "close_on_lose_focus": false,      // 失焦自动关闭
  "start_with_windows": false,       // 开机自启

  // ─── DeepSeek API ─────────────────────────────
  "deepseek_api_key": "sk-...",      // DeepSeek API Key
  "deepseek_base_url": "https://api.deepseek.com/v1",  // API 地址
  "deepseek_model": "deepseek-chat", // 模型名 (deepseek-chat = v4-flash)
  "deepseek_temperature": 0.1,       // 生成温度 0~2 (翻译建议 ≤0.3)

  // ─── 外观 ─────────────────────────────────────
  "theme": "system",                 // system | dark | light
  "window_opacity": 0.95,            // 窗口透明度 0.5~1.0
  "font_size": 14                    // 翻译内容字号
}
```

> 也可以通过设置面板修改，无需手写 JSON。

---

## 技术架构

```
┌───────────────── Frontend (Svelte 5) ─────────────────┐
│                                                        │
│  TranslationPopup         SettingsPage                  │
│  (index.html)             (settings.html)               │
│                                                        │
└────────────┬───────────────────────────────┬────────────┘
             │ Tauri IPC (invoke / events)   │
┌────────────▼───────────────────────────────▼────────────┐
│                    Rust Backend                          │
│                                                        │
│  ┌──────────┐  ┌────────────┐  ┌────────────────────┐ │
│  │ Detection│  │ Translator │  │ Config (JSON file)  │ │
│  │  ─ Windows│  │  ─ DeepSeek│  │                    │ │
│  │  clipboard│  │  SSE stream│  │ serde + Mutex      │ │
│  │  capture  │  │  async     │  │                    │ │
│  └──────────┘  └────────────┘  └────────────────────┘ │
│                                                        │
│  System Tray (tray-icon)                                │
│  Window Management (translator + settings)              │
└────────────────────────────────────────────────────────┘
```

### 技术栈

| 层 | 技术 |
|---|---|
| 桌面框架 | [Tauri v2](https://v2.tauri.app) |
| 后端语言 | Rust 1.84+ |
| 前端框架 | [Svelte 5](https://svelte.dev) |
| 构建工具 | Vite 6 |
| 翻译 API | DeepSeek OpenAI-compatible API (SSE 流式) |
| Windows API | `windows` crate (clipboard, keyboard simulation) |
| 打包 | NSIS 安装器 |

### 项目结构

```
translens/
├── src/                           # 前端
│   ├── App.svelte                 # 翻译弹窗
│   ├── lib/
│   │   └── SettingsPage.svelte    # 设置页面
│   ├── main.ts                    # 翻译窗口入口
│   └── settings-main.ts           # 设置窗口入口
│
├── src-tauri/                     # Rust 后端
│   ├── src/
│   │   ├── main.rs                # 入口
│   │   ├── lib.rs                 # Tauri setup + 系统托盘
│   │   ├── commands.rs            # IPC 命令
│   │   ├── detection/             # 文本选择检测
│   │   │   ├── win.rs             # Windows: 剪贴板捕获 + Ctrl+C
│   │   │   └── stub.rs            # Linux: NOP
│   │   ├── translator/
│   │   │   └── deepseek.rs        # DeepSeek 流式翻译
│   │   └── config/
│   │       └── store.rs           # JSON 配置
│   ├── tauri.conf.json
│   └── Cargo.toml
│
├── .github/workflows/build.yml    # CI 自动构建
├── README.md
└── package.json
```

---

## 开发指南

### 环境要求

| 工具 | 版本 |
|---|---|
| Rust | 1.84+ |
| Node.js | 22+ |
| Tauri CLI | 2.x |

### 在 Ubuntu 上开发 Windows 应用

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装 Tauri 系统依赖
sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev

# 克隆项目
git clone https://github.com/Hi-Barry/translens.git
cd translens

# 安装前端依赖
npm install

# 开发模式（Linux 上运行，仅测试非 Windows 专有功能）
npm run tauri dev

# 最终 Windows 构建交给 CI
git push origin main
```

### 跨平台注意事项

- **Windows 专有代码**标记 `#[cfg(target_os = "windows")]`
- **Linux/macOS 测试桩**在 `stub.rs` 中
- 核心逻辑（翻译、配置）跨平台通用
- CI 自动在 `windows-latest` 上构建

### 构建

```bash
# 前端构建
npm run build

# Rust 检查
cargo check --manifest-path src-tauri/Cargo.toml

# Rust 测试
cargo test --manifest-path src-tauri/Cargo.toml

# 完整 Tauri 构建
npm run tauri build
```

---

## 发布新版本

```bash
# 打标签
git tag v0.2.0
git push origin v0.2.0

# CI 自动完成:
#   1. Linux 检查 (check)
#   2. Windows 构建 (build-windows)
#   3. 创建 GitHub Release (release)
#   4. 上传 NSIS 安装器 + 便携版 exe
```

---

## 路线图

- [x] 系统托盘 + 右键菜单
- [x] DeepSeek 流式翻译
- [x] 翻译弹窗（固定、复制、朗读）
- [x] 设置面板
- [x] 选中文本捕获 (Ctrl+C 模拟)
- [ ] 自动检测文本选中（UIA / 鼠标钩子）
- [ ] 浮动翻译按钮
- [ ] 全局快捷键 (Alt+Shift+T)
- [ ] 截图翻译 (OCR)
- [ ] 多引擎支持 (DeepL / 百度 / 本地)
- [ ] 翻译历史记录

---

## License

MIT
