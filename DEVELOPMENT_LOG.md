# TransLens 开发日志

> 本文档记录开发过程中的踩坑、修复、决策，每次迭代增量更新。
> 最后一次更新: 2026-05-22

---

## 2026-05-22

### 背景

修复 Translens CI 构建+功能迭代。项目使用 Tauri v2 + Svelte 5，目标是 Windows 桌面翻译工具。

### CI 修复（5轮迭代）

#### 问题：windows crate v0.60 API 不兼容

**现象：** `build-windows` job 编译失败
**根因：** `windows = "0.60"` crate 的 Win32 API 与原生 API 有诸多差异

**踩坑记录：**

| 轮次 | 尝试方案 | 结果 | 原因 |
|------|---------|------|------|
| 1 | 加 `Win32_System_DataExchange` feature | ❌ | 还缺 `System_Memory` |
| 2 | 加 `System_Memory` feature | ❌ | `GlobalAlloc` 返回 `Result<HGLOBAL>` 不是裸 `HGLOBAL` |
| 3 | `if let Ok(handle)` 处理 Result | ❌ | `HANDLE` ↔ `HGLOBAL` 类型不兼容 |
| 4 | 显式类型转换 `HGLOBAL(handle.0)` | ❌ | crate 函数签名还有更多坑 |
| 5 | 改用 `tauri-plugin-clipboard-manager` | ❌ | plugin 在 Windows runner 也有兼容问题 |
| **6** | **直接 Win32 FFI** `#[link(name = "user32")]` | ✅ | **结论：windows crate 对简单 Win32 调用太重了** |

**教训：** Tauri 项目里如果只是调几个 Win32 API（`OpenClipboard`、`keybd_event`），直接 FFI 比引入 `windows` crate 稳定得多。crate 的 Result 包装、类型系统、feature 依赖链都可能导致跨平台编译问题。

#### CI 调试技巧

添加 `cargo check` 步骤，输出重定向到文件，作为 artifact 上传：
```yaml
- name: Debug Rust check
  continue-on-error: true
  run: cargo check 2>&1 | Out-File -FilePath check.log
- uses: actions/upload-artifact@v4
  with:
    path: check.log
```

### 功能修复

#### 1. 翻译流 Empty chunk 错误

**现象：** 翻译时显示"翻译流错误：Empty chunk"
**根因：** SSE 解析器对心跳包/空行返回 `Err("Empty chunk")`，导致整个 stream 中断
**修复：** `deepseek.rs` 空 chunk 返回 `Ok(String::new())`，`commands.rs` 过滤 `if !content.is_empty()`

#### 2. Ctrl+Click 拖拽窗口（6次迭代！）

**现象：** 窗口无法拖动，持续修复多次才成功

| 尝试 | 方案 | 结果 | 原因 |
|------|------|------|------|
| 1 | `data-tauri-drag-region` 属性 | ❌ | 无边框透明窗口下不生效 |
| 2 | `setPosition(plainObject)` 每次 mousemove | ❌ | 窗口不动——IPC 序列化失败 |
| 3 | CSS `transform: translate()` | ❌ | WebView 裁剪超出原始矩形区域的内容 |
| 4 | `setPosition` + RAF 节流 | ❌ | 窗口不动——plain object 无 `type` 字段 |
| 5 | `new PhysicalPosition(x, y)` | ❌ | async mousedown 的 await 期间事件循环插入 mousemove，`isDragging` 已 true 但坐标未记录 → 窗口跳转 |
| **6** | **`window.startDragging()` 原生 API** | ✅ | **结论：** Tauri v2 框架无边框窗口拖拽，就该用原生 `startDragging()`，不要自己拼 `setPosition` |

**关键发现：** Tauri v2 的 `setPosition` 接收 `PhysicalPosition | LogicalPosition` 类实例。传纯对象 `{x, y}` 时，JS 端 `Position` 类 `this.position.type === undefined`，序列化输出 `{"undefined": {"x": ...}}`，Rust Serde 无法反序列化。

**`startDragging()` 需要权限：** `core:window:allow-start-dragging`

#### 3. 窗口内容不可选中/复制

**现象：** 原文和译文无法选中
**根因：** CSS 全局 `user-select: none`
**修复：** 正文区域改为 `.selectable { user-select: text; -webkit-user-select: text; }`

#### 4. 固定按钮无视觉反馈

**根因：** 只有 `.active` 颜色变化，不够明显
**修复：** 金色高亮 + 状态文字"已固定" + 工具栏指示器

#### 5. ESC 退出程序

**根因：** 前端调用 `window.close()` 销毁窗口 → Tauri 退出
**修复：** 改为 `window.hide()`

#### 6. 切换语言时译文追加

**根因：** 只改了 `targetLang` 就重新翻译，新内容追加到旧译文后面
**修复：** 切换时 `sourceText = translatedText`，清空 `translatedText`，重新翻译

#### 7. 滚动条白色不搭

**修复：** `::-webkit-scrollbar` 自定义深色轨道/滑块 + Firefox `scrollbar-color`

#### 8. 窗口不自动隐藏（失焦）

**根因：** 没有监听 `tauri://blur` 事件
**修复：** 200ms 防抖——resize 操作会短暂 blur，防抖后只有持续失焦才隐藏

#### 9. 固定后监听剪贴板

**实现：** 固定后 1.5s 轮询 `navigator.clipboard.readText()`，内容变化自动翻译。解除固定时停止。

#### 10. 窗口位置记忆（3次迭代）

| 尝试 | 方案 | 结果 | 原因 |
|------|------|------|------|
| 1 | Rust 侧 `AppConfig.window_x/y` + `save_window_position` 命令 | ❌ | `#[serde(default)]` 没加 → 已有 `config.json` 反序列化失败 → API Key 丢失 |
| 2 | 纯前端 `localStorage` + `tauri://move` 事件 | ❌ | `hideWindow()` 里的 `savePosition()` 被我意外删了，且原生拖拽期间 `tauri://move` 可能不触发 |
| **3** | **三层冗余保存：`tauri://move` + `startDragging()` resolve 后 + `hideWindow()` 前** | ✅ | |

**教训：** Rust 侧给 serde 结构体加字段必须加 `#[serde(default)]`，否则已存在的配置文件会爆掉。前端 `localStorage` 比 Rust IPC 更可靠（同步、无 IO 失败、无需序列化）。

### Rust 配置管理注意事项

```rust
// 错误做法：新字段无默认值，已有配置文件反序列化失败
pub window_x: i32,
pub window_y: i32,

// 正确做法：
#[serde(default = "default_window_pos")]
pub window_x: i32,
#[serde(default = "default_window_pos")]
pub window_y: i32,
```

---

## 2026-05-22（晚）

### 功能：UIA 选中检测 + 浮动翻译按钮

**目标：** 实现鼠标选中文本自动检测，在鼠标右上方显示浮动翻译图标，点击后弹出翻译窗口直接翻译，不经过剪贴板。

#### 整体设计

```
鼠标选中文本
    ↓  WinEvent Hook (EVENT_OBJECT_SELECTION)
UIA 钩子线程 → SetWinEventHook + 消息泵
    ↓  UIA TextPattern::GetSelection (原始 COM FFI)
读取选中文本 + 光标位置
    ↓  emit("selection-detected", {text, cx, cy, rect})
lib.rs 监听器
    ↓  overlay::show_button(cx+12, cy-20, text)
创建/定位 36×36 透明浮动按钮窗口 (OverlayButton.svelte)
    ↓  用户点击蓝色翻译图标
invoke("overlay_click")
    ↓  overlay::on_overlay_click()
隐藏按钮 → 打开翻译窗口 → 自动翻译
```

#### 新增文件

| 文件 | 行数 | 说明 |
|------|------|------|
| `src-tauri/src/detection/uia_hook.rs` | ~470 | UIA 选中检测核心。原始 COM FFI（不依赖 windows crate），SetWinEventHook + 消息泵 |
| `src/lib/OverlayButton.svelte` | ~100 | 36×36 蓝色翻译图标，带缩放淡入动画 |
| `src/overlay-main.ts` | 10 | 浮动按钮窗口入口 |
| `overlay.html` | 16 | 浮动按钮 HTML 模板 |

#### 关键技术决策

**1. 为什么不直接用 windows crate 的 UIA 绑定？**

现有 `win.rs` 使用的是原始 FFI（`#[link(name = "user32")]`），原因是项目在 Linux 上交叉编译，`windows` crate 的 `#![cfg(windows)]` 属性使其在 Linux host 上完全为空。为了保持一致和可靠，UIA 也用了原始 COM FFI。

但实践中，UIA COM 接口的 vtbl 定义极其冗长（每个接口几十个方法）。尝试了两种路径：

| 方案 | 结果 | 原因 |
|------|------|------|
| windows crate `UI_Accessibility` feature | ❌ 开发机 Linux 无法编译 | 跨平台开发环境限制 |
| **原始 COM FFI + vtable 索引** | ✅ | 遵循现有代码模式，只声明需要用到的 vtable entry 索引号即可 |

**2. UIA TextPattern vs EM_GETSELTEXT**

UIA TextPattern 跨应用覆盖最广（浏览器、VS Code、记事本等），但需要处理 SAFEARRAY。`EM_GETSELTEXT` 只支持标准 Edit/RichEdit 控件。选了 UIA。

**3. SAFEARRAY 处理**

`IUIAutomationTextPattern::GetSelection` 返回 SAFEARRAY of `IUIAutomationTextRange*`。直接 `SafeArrayAccessData` + 指针数组读取，配套 `SafeArrayGetDim` / `SafeArrayGetUBound` / `SafeArrayDestroy`，全部原始 oleaut32 FFI。

**4. 浮动按钮实现方式**

| 方案 | 结果 | 原因 |
|------|------|------|
| 原始 WS_EX_LAYERED 窗口 | 未采用 | 需要额外处理绘制/点击命中 |
| **Tauri WebviewWindow** | ✅ | 复用 Vite 构建流程，Svelte 组件开箱即用 |

透明的 36×36 Tauri 窗口，`skip_taskbar: true`, `alwaysOnTop: true`。Svelte 组件实现缩放淡入动画，点击后通过 IPC 通知 Rust 后端。

#### 踩坑记录

**Bug 1：UIA 钩子线程从未启动**

**现象：** 选中文本无任何反应，调试日志无 UIA 相关输出
**根因：** `detection::start_detection` 里调用顺序为：
```rust
pub fn start_detection(handle: AppHandle) {
    platform::start_impl(handle.clone(), running);   // ← 死循环，从不返回
    uia_hook::start_uia_hook(handle);                // ← 永远执行不到
}
```
`platform::start_impl` 内部有一个 `while(running) { sleep(500ms) }` 永真循环。
**修复：** 将 `start_impl` 放入 `thread::spawn`。

**Bug 2（潜在）：COM 初始化顺序**

`CoInitializeEx` 必须在 `SetWinEventHook` 之前调用。当前 `start_uia_hook` 内先 `CoInitializeEx` 再注册 hook，顺序正确。

**Bug 3（潜在）：消息泵阻塞**

`GetMessageW` 是阻塞调用，没有消息时会挂起线程。这是 WinEventHook 的标准模式——Windows 在消息泵活跃时才能分发事件回调。使用 `WINEVENT_OUTOFCONTEXT` 标志，回调会由 `GetMessageW` 在内部调用。

#### 配置变更

`config/store.rs` 新增字段：
```rust
pub text_selection_detection: bool,  // 默认 true
```

`tauri.conf.json` 新增窗口：
```json
{
  "label": "overlay-button",
  "url": "overlay.html",
  "width": 36, "height": 36,
  "decorations": false, "transparent": true,
  "alwaysOnTop": true, "visible": false,
  "skip_taskbar": true
}
```

vite.config.ts 新增构建入口：
```ts
rollupOptions.input.overlay = resolve(__dirname, "overlay.html")
```

### 调试技巧（Windows 专用）

```rust
// 在 uia_hook.rs 中加日志
fn process_selection() {
    log::info!("selection-detected event received");
    match unsafe { query_uia_selection() } {
        Some(sel) => log::info!("selected text: {}...", &sel.text[..sel.text.len().min(50)]),
        None => log::info!("no selection or UIA unavailable"),
    }
}
```

```bash
# Windows 上查看日志（需要 env_logger）
$env:RUST_LOG="info"
translens.exe 2> debug.log
```

### 常用命令

```bash
# 本地检查
cargo check --manifest-path src-tauri/Cargo.toml
npm run build

# CI 调试（在 build-windows job 中）
cargo check 2>&1 | tee check.log
```

---

## 2026-05-25 — 重构：移除 Windows 钩子，改用全局快捷键

### 背景
- Windows 钩子（UIA + WinEventHook）过于复杂，反复实现未能稳定工作
- 改为「快捷键触发」方案，用户手动复制文本后按热键调出翻译

### 改动清单

**删除了：**
- `src-tauri/src/detection/` — 整个模块（win.rs, uia_hook.rs, stub.rs, mod.rs）
- `src-tauri/src/overlay/` — 整个模块（win.rs, stub.rs, mod.rs）
- `src/lib/OverlayButton.svelte` — 浮动翻译按钮
- `src/overlay-main.ts` — overlay 入口
- `overlay.html` — overlay 页面
- `Cargo.toml` 中的 `windows = "0.60"` crate（不再需要）
- 配置项：`show_overlay_button`, `text_selection_detection`, `overlay_timeout_ms`

**新增了：**
- `tauri-plugin-global-shortcut = "2"` — 全局快捷键插件
- `Alt+Shift+T` 全局快捷键注册
  - Rust 侧处理：热键按下 → 读取剪贴板文本 → 判断是否为文本 → 弹出翻译窗口
  - 非文本内容（图片、文件等）自动忽略，不弹窗
- 原文默认隐藏，点击「显示原文」按钮切换
- 剪贴板变化自动监听（窗口可见时 1.5s 轮询）
  - 窗口 focus → 开始监听，blur → 停止监听
  - Pin 住时保持监听不受 blur 影响

**修改了：**
- `src/App.svelte` — 原文区折叠、剪贴板监听生命周期调整、空状态提示
- `src/lib/SettingsPage.svelte` — 移除过时选项，添加快捷键说明
- `vite.config.ts` — 移除 overlay.html 入口
- `capabilities/default.json` — 添加 global-shortcut 权限
- `config/store.rs` — 简化配置结构

### 用户流程

```
1. 选中文字 → Ctrl+C 复制到剪贴板
2. 按 Alt+Shift+T → 弹出翻译窗口，显示译文（原文默认隐藏）
3. 想看原文 → 点「显示原文」按钮
4. 窗口保持可见时，剪贴板变化自动重新翻译
5. ESC → 隐藏窗口（Pin 住时 ESC 不生效）
```

### 注意事项
- `clipboard-manager` 的 `.read_text().ok()` 返回 `Option<String>`，不需要 `.flatten()`（不用 `windows` crate 的旧写法）
- `tauri-plugin-global-shortcut` 的 API：`on_shortcut(shortcut, handler)` 两个参数，handler 签名为 `Fn(&AppHandle<R>, &Shortcut, ShortcutEvent)`
- `ShortcutState::Pressed` 用于判断按键按下，不是 `ShortcutEvent::Pressed`
