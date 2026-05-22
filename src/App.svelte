<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import "./app.css";

  let sourceText = $state("");
  let translatedText = $state("");
  let isTranslating = $state(false);
  let isPinned = $state(false);
  let targetLang = $state("zh-CN");

  // --- Drag state (Ctrl+Click, Tauri setPosition per frame) ---
  let isDragging = $state(false);
  let dragStartScreenX = 0;
  let dragStartScreenY = 0;
  let dragStartWinX = 0;
  let dragStartWinY = 0;
  let dragScale = 1;
  let ctrlHeld = $state(false);
  // RAF throttling: store latest delta for the next frame
  let rafPending = false;
  let latestDx = 0;
  let latestDy = 0;

  // --- Clipboard monitor (when pinned) ---
  let clipMonitorInterval: ReturnType<typeof setInterval> | null = null;
  let lastClipText = "";

  // --- Tauri event listeners ---
  let unlisteners: Array<() => void> = [];

  // --- Tauri window handle (lazy loaded) ---
  let win: any = null;
  async function getWin() {
    if (!win) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      win = getCurrentWindow();
    }
    return win;
  }

  onMount(async () => {
    // Cache window handle
    await getWin();

    // Setup blur → auto-hide (unless pinned)
    await setupBlurHandler();

    // Receive text to translate
    unlisteners.push(
      await listen<string>("translate-text", (event) => {
        sourceText = event.payload;
        lastClipText = event.payload;
        translatedText = "";
        isTranslating = true;
        translate();
      })
    );

    // Receive translation chunks (streaming)
    unlisteners.push(
      await listen<string>("translation-chunk", (event) => {
        translatedText += event.payload;
      })
    );

    // Translation complete
    unlisteners.push(
      await listen<void>("translation-done", () => {
        isTranslating = false;
      })
    );
  });

  onDestroy(() => {
    unlisteners.forEach((u) => u());
    stopClipMonitor();
  });

  // --- Translation ---

  async function translate() {
    const { invoke } = await import("@tauri-apps/api/core");
    try {
      translatedText = "";
      await invoke("translate_text", {
        text: sourceText,
        sourceLang: "auto",
        targetLang: targetLang,
      });
    } catch (e) {
      console.error("Translation failed:", e);
      translatedText = `翻译失败: ${e}`;
      isTranslating = false;
    }
  }

  /** Switch languages: swap source ↔ translation, re-translate the swapped text */
  function switchLang() {
    const newLang = targetLang === "zh-CN" ? "en" : "zh-CN";
    targetLang = newLang;

    if (sourceText && translatedText && !translatedText.startsWith("翻译")) {
      // Swap: the current translation becomes the new source, re-translate
      const oldTranslation = translatedText;
      sourceText = oldTranslation;
      translatedText = "";
      isTranslating = true;
      translate();
    }
  }

  // --- Window controls ---

  async function hideWindow() {
    const w = await getWin();
    await w.hide();
  }

  function togglePin() {
    isPinned = !isPinned;
    if (isPinned) {
      startClipMonitor();
    } else {
      stopClipMonitor();
    }
  }

  // --- Clipboard monitor (when pinned) ---

  function startClipMonitor() {
    if (clipMonitorInterval) return;
    clipMonitorInterval = setInterval(async () => {
      try {
        let text: string;
        try {
          text = await navigator.clipboard.readText();
        } catch {
          const { readText } = await import(
            "@tauri-apps/plugin-clipboard-manager"
          );
          text = await readText();
        }
        if (text && text !== lastClipText && text !== sourceText) {
          lastClipText = text;
          sourceText = text;
          translatedText = "";
          isTranslating = true;
          translate();
        }
      } catch {
        // Silently ignore clipboard access errors
      }
    }, 1500);
  }

  function stopClipMonitor() {
    if (clipMonitorInterval) {
      clearInterval(clipMonitorInterval);
      clipMonitorInterval = null;
    }
  }

  // --- Auto-hide on blur (unless pinned) ---
  // Set up in onMount after window handle is cached
  let blurSetupDone = false;
  async function setupBlurHandler() {
    if (blurSetupDone) return;
    const w = await getWin();
    await w.listen("tauri://blur", () => {
      if (!isPinned) {
        hideWindow();
      }
    });
    blurSetupDone = true;
  }

  // --- Custom Ctrl+Click drag ---
  // Strategy: cache all state on mousedown, then call Tauri `setPosition()`
  // on each mousemove via requestAnimationFrame (no CSS transform clipping).

  async function handleMouseDown(e: MouseEvent) {
    if (!ctrlHeld) return;
    const target = e.target as HTMLElement;
    if (target.closest("button") || target.closest("input") || target.closest("textarea")) return;

    isDragging = true;
    const w = await getWin();
    const pos = await w.position();
    dragStartScreenX = e.screenX;
    dragStartScreenY = e.screenY;
    dragStartWinX = pos.x;
    dragStartWinY = pos.y;
    dragScale = await w.scaleFactor() || 1;
  }

  async function doSetPosition(dxPhys: number, dyPhys: number) {
    try {
      const w = await getWin();
      await w.setPosition({
        x: Math.round(dragStartWinX + dxPhys / dragScale),
        y: Math.round(dragStartWinY + dyPhys / dragScale),
      });
    } catch (err) {
      console.error("setPosition:", err);
    }
  }

  function handleMouseMove(e: MouseEvent) {
    if (!isDragging) return;
    // Keep updating latest delta on every mousemove
    latestDx = e.screenX - dragStartScreenX;
    latestDy = e.screenY - dragStartScreenY;
    // Schedule at most one RAF per frame — uses freshest delta
    if (!rafPending) {
      rafPending = true;
      requestAnimationFrame(async () => {
        rafPending = false;
        await doSetPosition(latestDx, latestDy);
      });
    }
  }

  /** Mouse up / leave: finalize position. */
  async function handleMouseUp(_e: MouseEvent) {
    if (!isDragging) return;
    isDragging = false;
    rafPending = false;
  }

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape" && !isPinned) {
      hideWindow();
    }
    if (e.key === "Control") {
      ctrlHeld = true;
    }
  }

  function handleKeyUp(e: KeyboardEvent) {
    if (e.key === "Control") {
      ctrlHeld = false;
      isDragging = false;
      rafPending = false;
    }
  }

  // --- Copy helpers ---

  async function copyText(text: string) {
    try {
      await navigator.clipboard.writeText(text);
    } catch {
      const { writeText } = await import(
        "@tauri-apps/plugin-clipboard-manager"
      );
      await writeText(text);
    }
  }

  async function speakText(text: string) {
    if ("speechSynthesis" in window) {
      const utterance = new SpeechSynthesisUtterance(text);
      utterance.lang = targetLang === "zh-CN" ? "zh-CN" : "en-US";
      speechSynthesis.speak(utterance);
    }
  }
</script>

<svelte:window
  onkeydown={handleKeyDown}
  onkeyup={handleKeyUp}
  onmousedown={handleMouseDown}
  onmousemove={handleMouseMove}
  onmouseup={handleMouseUp}
  onmouseleave={handleMouseUp}
/>

<div class="window" class:dragging={isDragging}>
  <!-- Title bar (drag is handled by Ctrl+Click, data-tauri-drag-region is not used) -->
  <div class="titlebar">
    <div class="titlebar-left">
      <button
        class="icon-btn {isPinned ? 'active' : ''}"
        onclick={togglePin}
        title={isPinned ? '取消固定' : '固定窗口'}
      >
        {isPinned ? '📌' : '📍'}
      </button>
      <span class="ctrl-hint">Ctrl+拖拽</span>
    </div>
    <div class="titlebar-center">TransLens</div>
    <div class="titlebar-right">
      <button class="icon-btn" onclick={hideWindow} title="隐藏 (ESC)">✕</button>
    </div>
  </div>

  <!-- Source text -->
  <div class="section source-section">
    <div class="section-label">
      原文
      <button class="copy-btn" onclick={() => copyText(sourceText)} title="复制原文">📋</button>
    </div>
    <div class="text-content selectable">{sourceText}</div>
  </div>

  <div class="divider"></div>

  <!-- Translation result -->
  <div class="section result-section">
    <div class="section-label">
      翻译 ({targetLang === "zh-CN" ? "中文" : "English"})
      <button class="copy-btn" onclick={() => copyText(translatedText)} title="复制译文">📋</button>
    </div>
    <div class="text-content selectable">
      {#if isTranslating}
        {#if translatedText}
          {translatedText}
        {:else}
          <span class="loading">翻译中...</span>
        {/if}
      {:else if translatedText}
        {translatedText}
      {/if}
    </div>
  </div>

  <!-- Bottom toolbar -->
  <div class="toolbar">
    <button class="tool-btn" onclick={() => speakText(translatedText)} title="朗读">🔊</button>
    <button class="tool-btn" onclick={() => copyText(translatedText)} title="复制译文">📋</button>
    <button class="tool-btn" onclick={switchLang} title="切换语言">🔄</button>

    {#if isPinned}
      <!-- When pinned, show monitoring indicator -->
      <span class="pin-badge pinned">📌 已固定 · 监听中</span>
    {:else}
      <!-- When not pinned, show blur auto-hide hint -->
      <span class="pin-badge auto-hide">失焦隐藏</span>
    {/if}

    <span class="flex-1"></span>
    <span class="lang-badge">{targetLang === "zh-CN" ? "中" : "EN"}</span>
  </div>
</div>

<style>
  .window {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 10px;
    overflow: hidden;
    cursor: default;
  }
  .window.dragging {
    cursor: grabbing;
  }

  .titlebar {
    display: flex;
    align-items: center;
    padding: 6px 8px;
    background: var(--surface);
    gap: 4px;
    flex-shrink: 0;
  }

  .titlebar-center {
    flex: 1;
    text-align: center;
    font-size: 12px;
    color: var(--text-secondary);
    letter-spacing: 1px;
    text-transform: uppercase;
  }

  .titlebar-right,
  .titlebar-left {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .ctrl-hint {
    font-size: 10px;
    color: var(--text-secondary);
    opacity: 0.5;
    margin-left: 4px;
  }

  .icon-btn {
    background: none;
    border: none;
    color: var(--text-secondary);
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 13px;
    transition: all 0.15s;
  }
  .icon-btn:hover {
    background: rgba(255,255,255,0.1);
    color: var(--text);
  }
  .icon-btn.active {
    color: #ffd700;
    text-shadow: 0 0 6px rgba(255,215,0,0.5);
  }

  .section {
    padding: 10px 14px;
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }

  .section-label {
    font-size: 11px;
    color: var(--text-secondary);
    margin-bottom: 6px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .copy-btn {
    background: none;
    border: none;
    color: var(--accent);
    cursor: pointer;
    font-size: 11px;
    padding: 0;
    opacity: 0.5;
    transition: opacity 0.15s;
  }
  .copy-btn:hover {
    opacity: 1;
  }

  .text-content {
    font-size: 14px;
    line-height: 1.6;
    word-break: break-word;
    white-space: pre-wrap;
  }

  .selectable {
    user-select: text;
    -webkit-user-select: text;
  }

  .divider {
    height: 1px;
    background: var(--border);
    margin: 0 14px;
    flex-shrink: 0;
  }

  .loading {
    color: var(--text-secondary);
    animation: pulse 1.5s ease infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .toolbar {
    display: flex;
    align-items: center;
    padding: 6px 10px;
    background: var(--surface);
    gap: 4px;
    flex-shrink: 0;
  }

  .tool-btn {
    background: none;
    border: none;
    color: var(--text-secondary);
    cursor: pointer;
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 14px;
    transition: all 0.15s;
  }
  .tool-btn:hover {
    background: rgba(255,255,255,0.1);
    color: var(--text);
  }

  .flex-1 { flex: 1; }

  .lang-badge {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 10px;
    background: var(--accent);
    color: white;
  }

  .pin-badge {
    font-size: 10px;
    padding: 2px 6px;
    border-radius: 8px;
    transition: all 0.2s;
  }
  .pin-badge.pinned {
    background: rgba(255,215,0,0.15);
    color: #ffd700;
  }
  .pin-badge.auto-hide {
    background: rgba(255,255,255,0.05);
    color: var(--text-secondary);
  }
</style>
