<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import "./app.css";

  let sourceText = $state("");
  let translatedText = $state("");
  let isTranslating = $state(false);
  let isPinned = $state(false);
  let targetLang = $state("zh-CN");

  // Listen for Tauri events from Rust backend
  let unlisteners: Array<() => void> = [];

  onMount(async () => {
    // Receive text to translate
    unlisteners.push(
      await listen<string>("translate-text", (event) => {
        sourceText = event.payload;
        translatedText = "";
        isTranslating = true;
        targetLang = "zh-CN"; // or read from config
        // Call the Rust backend command
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

    // Show settings window
    unlisteners.push(
      await listen<void>("show-settings", () => {
        // Could navigate or open settings
        console.log("show settings");
      })
    );
  });

  onDestroy(() => {
    unlisteners.forEach((u) => u());
  });

  async function translate() {
    const { invoke } = await import("@tauri-apps/api/core");
    try {
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

  async function closeWindow() {
    if (!isPinned) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().close();
    }
  }

  function togglePin() {
    isPinned = !isPinned;
  }

  async function copyResult() {
    try {
      await navigator.clipboard.writeText(translatedText);
    } catch {
      const { writeText } = await import("@tauri-apps/plugin-clipboard-manager");
      await writeText(translatedText);
    }
  }

  async function speakText(text: string) {
    if ("speechSynthesis" in window) {
      const utterance = new SpeechSynthesisUtterance(text);
      utterance.lang = targetLang === "zh-CN" ? "zh-CN" : "en-US";
      speechSynthesis.speak(utterance);
    }
  }

  // Listen for keyboard events
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && !isPinned) {
      closeWindow();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="window">
  <!-- Title bar (drag region for frameless window) -->
  <div class="titlebar" data-tauri-drag-region>
    <div class="titlebar-left">
      <button
        class="icon-btn {isPinned ? 'active' : ''}"
        onclick={togglePin}
        title={isPinned ? '取消固定' : '固定窗口'}
      >
        📌
      </button>
    </div>
    <div class="titlebar-center" data-tauri-drag-region>
      TransLens
    </div>
    <div class="titlebar-right">
      <button class="icon-btn" onclick={closeWindow} title="关闭 (ESC)">✕</button>
    </div>
  </div>

  <!-- Source text -->
  <div class="section source-section">
    <div class="section-label">原文</div>
    <div class="text-content">{sourceText}</div>
  </div>

  <div class="divider"></div>

  <!-- Translation result -->
  <div class="section result-section">
    <div class="section-label">翻译 ({targetLang})</div>
    <div class="text-content">
      {#if isTranslating && !translatedText}
        <span class="loading">翻译中...</span>
      {:else if translatedText}
        {translatedText}
      {/if}
    </div>
  </div>

  <!-- Bottom toolbar -->
  <div class="toolbar">
    <button class="tool-btn" onclick={() => speakText(translatedText)} title="朗读">
      🔊
    </button>
    <button class="tool-btn" onclick={copyResult} title="复制">
      📋
    </button>
    <button
      class="tool-btn"
      onclick={() => { targetLang = targetLang === 'zh-CN' ? 'en' : 'zh-CN'; translate(); }}
      title="切换语言"
    >
      🔄
    </button>
    <span class="flex-1"></span>
    <span class="lang-badge">{targetLang === 'zh-CN' ? '中' : 'EN'}</span>
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
    gap: 2px;
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
    color: var(--accent);
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
  }

  .text-content {
    font-size: 14px;
    line-height: 1.6;
    word-break: break-word;
    white-space: pre-wrap;
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

  .flex-1 {
    flex: 1;
  }

  .lang-badge {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 10px;
    background: var(--accent);
    color: white;
  }
</style>
