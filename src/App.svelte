<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import "./app.css";

  let sourceText = $state("");
  let translatedText = $state("");
  let isTranslating = $state(false);
  let isPinned = $state(false);
  let showOriginal = $state(false);
  let targetLang = $state("zh-CN");
  let isDark = $state(false);

  // --- Clipboard monitor (when visible) ---
  let clipMonitorInterval: ReturnType<typeof setInterval> | null = null;
  let lastClipText = $state("");

  // --- Translation cache ---
  const CACHE_KEY = "translens_cache";
  const MAX_CACHE = 10;

  interface CacheEntry {
    key: string;
    translated: string;
    ts: number;
  }

  function cacheKey(text: string, lang: string): string {
    return text + "|" + lang;
  }

  function getCache(text: string, lang: string): string | null {
    try {
      const raw = localStorage.getItem(CACHE_KEY);
      if (!raw) return null;
      const entries: CacheEntry[] = JSON.parse(raw);
      const key = cacheKey(text, lang);
      const idx = entries.findIndex((e) => e.key === key);
      if (idx === -1) return null;
      // Move to front (LRU)
      const [entry] = entries.splice(idx, 1);
      entries.unshift(entry);
      localStorage.setItem(CACHE_KEY, JSON.stringify(entries));
      return entry.translated;
    } catch {
      return null;
    }
  }

  function setCache(text: string, translated: string, lang: string) {
    try {
      const raw = localStorage.getItem(CACHE_KEY);
      let entries: CacheEntry[] = raw ? JSON.parse(raw) : [];
      const key = cacheKey(text, lang);
      // Remove existing entry for this key
      entries = entries.filter((e) => e.key !== key);
      // Add to front
      entries.unshift({ key, translated, ts: Date.now() });
      // Trim to max
      if (entries.length > MAX_CACHE) {
        entries = entries.slice(0, MAX_CACHE);
      }
      localStorage.setItem(CACHE_KEY, JSON.stringify(entries));
    } catch {
      // Silently fail — cache is non-critical
    }
  }

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
    // Restore theme preference
    const savedTheme = localStorage.getItem("translens_theme");
    if (savedTheme === "dark") {
      isDark = true;
      document.documentElement.classList.add("theme-dark");
      document.documentElement.classList.remove("theme-light");
    } else {
      isDark = false;
      document.documentElement.classList.add("theme-light");
      document.documentElement.classList.remove("theme-dark");
    }

    await getWin();

    // Setup blur → auto-hide (unless pinned)
    await setupBlurHandler();

    // Clipboard monitor lifecycle
    const w = await getWin();
    w.listen("tauri://focus", () => startClipMonitor());
    w.listen("tauri://blur", () => { if (!isPinned) stopClipMonitor(); });

    // Receive text to translate (from hotkey or tray)
    unlisteners.push(
      await listen<string>("translate-text", (event) => {
        sourceText = event.payload;
        lastClipText = event.payload;
        showOriginal = false;

        // Check cache first
        const cached = getCache(event.payload, targetLang);
        if (cached !== null) {
          translatedText = cached;
          isTranslating = false;
          return;
        }

        // Not cached — start translation
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
        // Cache result
        setCache(sourceText, translatedText, targetLang);
      })
    );
  });

  onDestroy(() => {
    unlisteners.forEach((u) => u());
    stopClipMonitor();
  });

  // --- Theme toggle ---

  function toggleTheme() {
    isDark = !isDark;
    const root = document.documentElement;
    if (isDark) {
      root.classList.remove("theme-light");
      root.classList.add("theme-dark");
      localStorage.setItem("translens_theme", "dark");
    } else {
      root.classList.remove("theme-dark");
      root.classList.add("theme-light");
      localStorage.setItem("translens_theme", "light");
    }
  }

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
      const oldTranslation = translatedText;
      sourceText = oldTranslation;
      translatedText = "";
      isTranslating = true;
      translate();
    }
  }

  // --- Window controls ---

  async function hideWindow() {
    stopClipMonitor();
    const w = await getWin();
    await w.hide();
  }

  function togglePin() {
    isPinned = !isPinned;
    if (isPinned) {
      startClipMonitor();
    } else {
      (async () => {
        const w = await getWin();
        const focused = await w.isFocused();
        if (!focused) stopClipMonitor();
      })();
    }
  }

  // --- Clipboard monitor (when window visible or pinned) ---

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

          // Check cache for clipboard auto-translate
          const cached = getCache(text, targetLang);
          if (cached !== null) {
            sourceText = text;
            translatedText = cached;
            showOriginal = false;
            isTranslating = false;
            return;
          }

          sourceText = text;
          translatedText = "";
          showOriginal = false;
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
  let blurSetupDone = false;
  let blurTimer: ReturnType<typeof setTimeout> | null = null;
  async function setupBlurHandler() {
    if (blurSetupDone) return;
    const w = await getWin();
    await w.listen("tauri://blur", () => {
      if (isPinned) return;
      if (blurTimer) clearTimeout(blurTimer);
      blurTimer = setTimeout(async () => {
        const focused = await w.isFocused();
        if (!focused && !isPinned) {
          await hideWindow();
        }
        blurTimer = null;
      }, 200);
    });
    await w.listen("tauri://focus", () => {
      if (blurTimer) {
        clearTimeout(blurTimer);
        blurTimer = null;
      }
    });
    blurSetupDone = true;
  }

  // --- Keyboard ---

  function handleKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape" && !isPinned) {
      hideWindow();
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
/>

<div class="window" class:theme-dark={isDark} class:theme-light={!isDark}>
  <!-- Title bar — draggable via native Tauri region -->
  <div class="titlebar" data-tauri-drag-region>
    <div class="titlebar-left">
      <button
        class="icon-btn {isPinned ? 'active' : ''}"
        onclick={togglePin}
        title={isPinned ? '取消固定' : '固定窗口'}
      >
        {isPinned ? '📌' : '📍'}
      </button>
    </div>
    <div class="titlebar-center">TransLens</div>
    <div class="titlebar-right">
      <button class="icon-btn" onclick={hideWindow} title="隐藏 (ESC)">✕</button>
    </div>
  </div>

  <!-- Source text (hidden by default) -->
  {#if showOriginal}
    <div class="section source-section">
      <div class="section-label">
        原文
        <button class="link-btn" onclick={() => showOriginal = false}>隐藏原文 ✕</button>
        <span class="flex-1"></span>
        <button class="copy-btn" onclick={() => copyText(sourceText)} title="复制原文">📋</button>
      </div>
      <div class="text-content selectable">{sourceText}</div>
    </div>
    <div class="divider"></div>
  {/if}

  <!-- Translation result (always visible) -->
  <div class="section result-section" class:is-full={!showOriginal}>
    <div class="section-label">
      {showOriginal ? '翻译' : '译文'} ({targetLang === "zh-CN" ? "中文" : "English"})
      {#if !showOriginal && sourceText}
        <button class="link-btn" onclick={() => showOriginal = true}>显示原文</button>
      {/if}
      <span class="flex-1"></span>
      <button class="copy-btn" onclick={() => copyText(translatedText)} title="复制译文">📋</button>
      {#if !isTranslating && translatedText && getCache(sourceText, targetLang) !== null}
        <span class="cached-badge">💾</span>
      {/if}
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
      {:else}
        <span class="empty-hint">复制文本后按 Alt+Shift+T 翻译</span>
      {/if}
    </div>
  </div>

  <!-- Bottom toolbar -->
  <div class="toolbar">
    <button class="tool-btn" onclick={() => speakText(translatedText)} title="朗读">🔊</button>
    <button class="tool-btn" onclick={() => copyText(translatedText)} title="复制译文">📋</button>
    <button class="tool-btn" onclick={switchLang} title="切换语言">🔄</button>

    {#if !showOriginal && sourceText}
      <button class="tool-btn text-btn" onclick={() => showOriginal = true}>原文</button>
    {:else if showOriginal}
      <button class="tool-btn text-btn active" onclick={() => showOriginal = false}>原文 ✓</button>
    {/if}

    <span class="flex-1"></span>

    {#if isPinned}
      <span class="pin-badge pinned">📌 已固定</span>
    {:else}
      <span class="pin-badge auto-hide">失焦隐藏</span>
    {/if}

    <button class="tool-btn theme-btn" onclick={toggleTheme} title={isDark ? '切换亮色' : '切换暗色'}>
      {isDark ? '☀️' : '🌙'}
    </button>

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
    box-shadow: 0 8px 32px var(--shadow);
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
    -webkit-app-region: drag;
  }

  .titlebar-right,
  .titlebar-left {
    display: flex;
    align-items: center;
    gap: 2px;
    -webkit-app-region: no-drag;
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
    background: rgba(128,128,128,0.15);
    color: var(--text);
  }
  .icon-btn.active {
    color: #ffd700;
    text-shadow: 0 0 6px rgba(255,215,0,0.5);
  }

  .section {
    padding: 10px 14px;
    overflow-y: auto;
    min-height: 0;
  }

  .source-section {
    flex: 0 0 auto;
    max-height: 35vh;
  }

  .result-section {
    flex: 1;
  }

  .result-section.is-full {
    flex: 1;
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

  .link-btn {
    background: none;
    border: none;
    color: var(--accent);
    cursor: pointer;
    font-size: 11px;
    padding: 0;
    opacity: 0.6;
    transition: opacity 0.15s;
  }
  .link-btn:hover {
    opacity: 1;
    text-decoration: underline;
  }

  .copy-btn {
    background: none;
    border: none;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 11px;
    padding: 0;
    opacity: 0.5;
    transition: opacity 0.15s;
  }
  .copy-btn:hover {
    opacity: 1;
    color: var(--accent);
  }

  .cached-badge {
    font-size: 10px;
    opacity: 0.4;
    margin-left: auto;
  }

  .flex-1 { flex: 1; }

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
    color: var(--loading-color);
    animation: pulse 1.5s ease infinite;
  }

  .empty-hint {
    color: var(--empty-hint);
    font-style: italic;
    font-size: 13px;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .toolbar {
    display: flex;
    align-items: center;
    padding: 6px 10px;
    background: var(--toolbar-bg);
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
    background: rgba(128,128,128,0.12);
    color: var(--text);
  }

  .tool-btn.text-btn {
    font-size: 11px;
    padding: 2px 8px;
    border: 1px solid var(--border);
  }
  .tool-btn.text-btn.active {
    border-color: var(--accent);
    color: var(--accent);
  }

  .theme-btn {
    font-size: 14px;
    padding: 2px 6px;
  }

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
    color: #b8960f;
  }
  .theme-light .pin-badge.pinned {
    background: rgba(255,215,0,0.2);
  }
  .pin-badge.auto-hide {
    background: rgba(128,128,128,0.08);
    color: var(--text-secondary);
  }
</style>
