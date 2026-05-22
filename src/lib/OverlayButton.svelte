<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  let visible = $state(true);
  let fadingOut = $state(false);

  onMount(() => {
    // Start entry animation
    const el = document.querySelector(".overlay-btn");
    if (el) {
      requestAnimationFrame(() => {
        el.classList.add("show");
      });
    }

    // Auto-hide after timeout (handled by Rust side, but also listen for hide)
    // Click anywhere outside would hide via the Rust side
  });

  async function handleClick() {
    fadingOut = true;
    // Brief delay for fade-out animation
    await new Promise((r) => setTimeout(r, 120));
    // Tell Rust backend to handle the click (open translator)
    await invoke("overlay_click");
  }

  async function handleMouseEnter() {
    // Subtle scale-up on hover
  }
</script>

<div
  class="overlay-btn"
  class:show={visible}
  class:fade-out={fadingOut}
  onmousedown={handleClick}
  onmouseenter={handleMouseEnter}
  role="button"
  tabindex="-1"
  title="翻译"
>
  <svg width="24" height="24" viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg">
    <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z" fill="currentColor"/>
  </svg>
</div>

<style>
  .overlay-btn {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: rgba(30, 144, 255, 0.92);
    color: white;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25), 0 0 0 1px rgba(255, 255, 255, 0.15);
    transition: transform 0.12s ease, opacity 0.12s ease;
    opacity: 0;
    transform: scale(0.7);
    user-select: none;
    -webkit-user-select: none;
    -webkit-app-region: no-drag;
    position: absolute;
    top: 0;
    left: 0;
  }

  .overlay-btn.show {
    opacity: 1;
    transform: scale(1);
  }

  .overlay-btn:hover {
    transform: scale(1.12);
    background: rgba(30, 144, 255, 1);
    box-shadow: 0 3px 12px rgba(30, 144, 255, 0.4);
  }

  .overlay-btn:active {
    transform: scale(0.95);
  }

  .overlay-btn.fade-out {
    opacity: 0;
    transform: scale(0.6);
    pointer-events: none;
  }
</style>
