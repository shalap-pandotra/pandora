<script>
import { getCurrentWindow } from '@tauri-apps/api/window';

const appWindow = getCurrentWindow();

function minimize() { appWindow.minimize(); }
function toggleMaximize() { appWindow.toggleMaximize(); }
function close() { appWindow.close(); }
</script>

<div class="topbar">
  <div id="brand-name" class="brand" data-tauri-drag-region>Pandora</div>
  <div class="topbar-right" data-tauri-drag-region>
    <div role="group" aria-label="Window controls" class="winctrls">
      <button class="zen-btn" aria-label="Minimize window" onclick={minimize}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="5" y1="19" x2="19" y2="19" />
        </svg>
      </button>
      <button class="zen-btn" aria-label="Restore window" onclick={toggleMaximize}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="5" y="5" width="14" height="14" rx="2" />
        </svg>
      </button>
      <button class="zen-btn" aria-label="Close window" onclick={close}>
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
  font-size:22px;
  transform: scaleY(2.23);
}
.topbar {
  height: 64px; flex: none; display: flex; background: var(--zen-bg);
  cursor: default;
  user-select: none;
}
.brand {
  width: 248px; box-sizing: border-box; flex: none;
  display: flex; align-items: center; padding: 0 20px;
  font-family: var(--zen-font-serif); font-weight: 600; font-size: 20px;
  letter-spacing: -0.01em;
}
.topbar-right { flex: 1; display: flex; align-items: center; justify-content: flex-end; gap: 20px; padding: 0 20px; }
.winctrls { display: flex; background: var(--zen-panel); border: 1px solid var(--zen-border); border-radius: 2px; overflow: hidden; }

.winctrls button { position: relative; width: 38px; height: 38px; display: flex; align-items: center; justify-content: center; color: var(--zen-text-secondary); }

.winctrls button + button::before {
  content: "";
  position: absolute;
  left: 0;
  top: 20%;
  height: 60%;
  width: 0.5px;
  background: var(--zen-border);
}
</style>
