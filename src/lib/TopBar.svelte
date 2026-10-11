<script>
import { getCurrentWindow } from '@tauri-apps/api/window';

// Without a title this is the library's bar. A reader window passes the book's title,
// shown in place of the name, with the same slim bar and window controls.
let { title = null } = $props();

const appWindow = getCurrentWindow();

function minimize() { appWindow.minimize(); }
function toggleMaximize() { appWindow.toggleMaximize(); }
function close() { appWindow.close(); }
</script>

<div class="topbar" class:reader={title != null}>
  {#if title != null}
    <div class="book-title" data-tauri-drag-region><span data-tauri-drag-region>{title}</span></div>
  {:else}
    <div id="brand-name" class="brand" data-tauri-drag-region>Pandora</div>
  {/if}
  <div class="topbar-right" data-tauri-drag-region>
    <div role="group" aria-label="Window controls" class="winctrls">
      <button class="zen-btn" aria-label="Minimize window" onclick={minimize}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="5" y1="19" x2="19" y2="19" />
        </svg>
      </button>
      <span class="sep" aria-hidden="true"></span>
      <button class="zen-btn" aria-label="Restore window" onclick={toggleMaximize}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="5" y="5" width="14" height="14" rx="2" />
        </svg>
      </button>
      <span class="sep" aria-hidden="true"></span>
      <button class="zen-btn close" aria-label="Close window" onclick={close}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="5" y1="5" x2="19" y2="19" /><line x1="19" y1="5" x2="5" y2="19" />
        </svg>
      </button>
    </div>
  </div>
</div>

<style>
#brand-name{
  font-weight:400;
  font-size:11px;
  transform: scaleY(2.23);
}
.topbar {
  height: 32px; flex: none; display: flex; background: var(--zen-bg);
  cursor: default;
  user-select: none;
}
.brand {
  width: 248px; box-sizing: border-box; flex: none;
  display: flex; align-items: center; padding: 0 20px;
  font-family: var(--zen-font-serif); font-weight: 600; font-size: 20px;
  letter-spacing: -0.01em;
}
.book-title {
  flex: 1; min-width: 0;
  display: flex; align-items: center; padding: 0 12px;
  font-family: var(--zen-font-serif); font-weight: 400; font-size: 14px;
}
.book-title span {
  overflow: hidden; white-space: nowrap; text-overflow: ellipsis;
  transform: scaleY(1.15);
}
.topbar-right { flex: 1; display: flex; align-items: center; justify-content: flex-end; gap: 20px; padding: 0 20px; }
.reader .topbar-right { flex: none; padding: 0 6px; }
.winctrls { display: flex; background: var(--zen-panel); border: 1px solid var(--zen-border); border-radius: 2px; overflow: hidden; }

.winctrls button { position: relative; width: 28px; height: 22px; display: flex; align-items: center; justify-content: center; color: var(--zen-text-secondary); }
.winctrls svg { width: 13px; height: 13px; }
.reader .winctrls button { color: var(--zen-red); }
.winctrls button:hover { background: var(--zen-pill-bg); }
.winctrls button.close:hover { background: var(--zen-red); color: #FFFFFF; }
/* The dividers are elements of their own between the buttons, so a hovered button's
   colour never reaches them. */
.sep {
  flex: none; width: 1px; align-self: stretch;
  display: flex; align-items: center;
}
.sep::before {
  content: "";
  width: 0.5px; height: 60%;
  background: var(--zen-border);
}
</style>
