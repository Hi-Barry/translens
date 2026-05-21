<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import "../app.css";

  let apiKey = $state("");
  let baseUrl = $state("https://api.deepseek.com/v1");
  let model = $state("deepseek-chat");
  let temperature = $state(0.1);
  let targetLang = $state("zh-CN");
  let showOverlay = $state(true);
  let closeOnEsc = $state(true);
  let theme = $state("system");
  let font_size = $state(14);
  let windowOpacity = $state(0.95);

  let saving = $state(false);
  let saved = $state(false);
  let error = $state("");

  onMount(async () => {
    try {
      const config: any = await invoke("get_config");
      apiKey = config.deepseek_api_key || "";
      baseUrl = config.deepseek_base_url || "https://api.deepseek.com/v1";
      model = config.deepseek_model || "deepseek-chat";
      temperature = config.deepseek_temperature ?? 0.1;
      targetLang = config.target_language || "zh-CN";
      showOverlay = config.show_overlay_button ?? true;
      closeOnEsc = config.close_on_esc ?? true;
      theme = config.theme || "system";
      font_size = config.font_size || 14;
      windowOpacity = config.window_opacity ?? 0.95;
    } catch (e) {
      error = `加载配置失败: ${e}`;
    }
  });

  async function save() {
    saving = true;
    saved = false;
    error = "";
    try {
      await invoke("save_config", {
        config: {
          target_language: targetLang,
          auto_detect_source: true,
          show_overlay_button: showOverlay,
          overlay_timeout_ms: 5000,
          close_on_esc: closeOnEsc,
          close_on_lose_focus: false,
          start_with_windows: false,
          deepseek_api_key: apiKey,
          deepseek_base_url: baseUrl,
          deepseek_model: model,
          deepseek_temperature: temperature,
          theme: theme,
          window_opacity: windowOpacity,
          font_size: font_size,
        },
      });
      saved = true;
      setTimeout(() => (saved = false), 2000);
    } catch (e) {
      error = `保存失败: ${e}`;
    } finally {
      saving = false;
    }
  }

  async function closeWindow() {
    await getCurrentWindow().close();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") closeWindow();
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="settings-window">
  <div class="header">
    <h1>⚙️ TransLens 设置</h1>
    <button class="close-btn" onclick={closeWindow}>✕</button>
  </div>

  {#if error}
    <div class="msg error">{error}</div>
  {/if}

  {#if saved}
    <div class="msg success">✅ 已保存</div>
  {/if}

  <div class="section">
    <h2>DeepSeek API 配置</h2>

    <label class="field">
      <span>API Key</span>
      <input
        type="password"
        bind:value={apiKey}
        placeholder="sk-xxxxxxxxxxxxxxxx"
      />
    </label>

    <label class="field">
      <span>API Base URL</span>
      <input type="url" bind:value={baseUrl} />
    </label>

    <label class="field">
      <span>模型</span>
      <input type="text" bind:value={model} />
    </label>

    <label class="field">
      <span>Temperature</span>
      <div class="range-row">
        <input type="range" min="0" max="2" step="0.05" bind:value={temperature} />
        <span class="range-val">{temperature}</span>
      </div>
    </label>
  </div>

  <div class="section">
    <h2>翻译设置</h2>

    <label class="field">
      <span>目标语言</span>
      <select bind:value={targetLang}>
        <option value="zh-CN">简体中文</option>
        <option value="en">English</option>
        <option value="ja">日本語</option>
        <option value="ko">한국어</option>
        <option value="fr">Français</option>
        <option value="de">Deutsch</option>
      </select>
    </label>
  </div>

  <div class="section">
    <h2>外观</h2>

    <label class="field">
      <span>主题</span>
      <select bind:value={theme}>
        <option value="system">跟随系统</option>
        <option value="dark">深色</option>
        <option value="light">浅色</option>
      </select>
    </label>

    <label class="field">
      <span>字体大小</span>
      <div class="range-row">
        <input type="range" min="10" max="24" step="1" bind:value={font_size} />
        <span class="range-val">{font_size}px</span>
      </div>
    </label>

    <label class="field">
      <span>窗口透明度</span>
      <div class="range-row">
        <input type="range" min="0.5" max="1" step="0.05" bind:value={windowOpacity} />
        <span class="range-val">{windowOpacity}</span>
      </div>
    </label>
  </div>

  <div class="section">
    <h2>行为</h2>

    <label class="field checkbox">
      <input type="checkbox" bind:checked={showOverlay} />
      <span>选中文本后显示浮动按钮</span>
    </label>

    <label class="field checkbox">
      <input type="checkbox" bind:checked={closeOnEsc} />
      <span>ESC 键关闭翻译窗口</span>
    </label>
  </div>

  <div class="footer">
    <button class="btn-primary" onclick={save} disabled={saving}>
      {saving ? "保存中..." : "💾 保存设置"}
    </button>
    <button class="btn-secondary" onclick={closeWindow}>关闭</button>
  </div>
</div>

<style>
  .settings-window {
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--text);
    overflow-y: auto;
    font-size: 14px;
    padding: 0;
  }

  .header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .header h1 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }
  .close-btn {
    background: none;
    border: none;
    color: var(--text-secondary);
    cursor: pointer;
    font-size: 18px;
    padding: 2px 8px;
    border-radius: 4px;
  }
  .close-btn:hover {
    background: rgba(255,255,255,0.1);
    color: var(--text);
  }

  .msg {
    margin: 8px 16px;
    padding: 8px 12px;
    border-radius: 6px;
    font-size: 13px;
  }
  .msg.error {
    background: rgba(255, 80, 80, 0.15);
    color: #ff6b6b;
  }
  .msg.success {
    background: rgba(80, 255, 80, 0.1);
    color: #51cf66;
  }

  .section {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }
  .section h2 {
    margin: 0 0 12px 0;
    font-size: 13px;
    color: var(--accent);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 12px;
  }
  .field span {
    font-size: 13px;
    color: var(--text-secondary);
  }
  .field input,
  .field select {
    padding: 8px 10px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: rgba(255,255,255,0.05);
    color: var(--text);
    font-size: 13px;
    outline: none;
    transition: border-color 0.15s;
  }
  .field input:focus,
  .field select:focus {
    border-color: var(--accent);
  }
  .field input[type="password"] {
    letter-spacing: 2px;
    font-family: monospace;
  }

  .field.checkbox {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }
  .field.checkbox input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: var(--accent);
  }

  .range-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .range-row input[type="range"] {
    flex: 1;
    accent-color: var(--accent);
  }
  .range-val {
    min-width: 40px;
    text-align: right;
    font-family: monospace;
    font-size: 13px;
    color: var(--accent);
  }

  .footer {
    display: flex;
    gap: 8px;
    padding: 12px 16px;
    background: var(--surface);
    flex-shrink: 0;
    margin-top: auto;
  }
  .btn-primary,
  .btn-secondary {
    padding: 8px 20px;
    border-radius: 6px;
    border: none;
    font-size: 14px;
    cursor: pointer;
    transition: all 0.15s;
  }
  .btn-primary {
    background: var(--accent);
    color: white;
  }
  .btn-primary:hover {
    filter: brightness(1.15);
  }
  .btn-primary:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .btn-secondary {
    background: rgba(255,255,255,0.08);
    color: var(--text);
  }
  .btn-secondary:hover {
    background: rgba(255,255,255,0.15);
  }
</style>
