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

### 常用命令

```bash
# 本地检查
cargo check --manifest-path src-tauri/Cargo.toml
npm run build

# CI 调试（在 build-windows job 中）
cargo check 2>&1 | tee check.log
```
