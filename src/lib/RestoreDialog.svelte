<script>
import { invoke } from '@tauri-apps/api/core';
import { onMount, tick } from 'svelte';
import { fade } from 'svelte/transition';

// Puts the library back the way it was before Pandora changed it, from one of the backups Pandora
// took. It asks twice, because a restore also undoes whatever changed since, wherever it changed.
let { onClose, onRestored } = $props();

/** @type {{ file: string, taken: number, size: number, before_restore: boolean, moves: number }[]} */
let backups = $state([]);
let loading = $state(true);
let loadError = $state('');
/** @type {any} */
let chosen = $state(null);
/** @type {'list' | 'confirm' | 'working' | 'done'} */
let step = $state('list');
let error = $state('');
/** @type {any} */
let report = $state(null);
/** @type {HTMLElement | undefined} */
let dialogEl = $state();

const when = (/** @type {number} */ secs) =>
  new Date(secs * 1000).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' });
const size = (/** @type {number} */ bytes) =>
  bytes >= 1048576 ? `${(bytes / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB`;
const plural = (/** @type {number} */ n, /** @type {string} */ one, /** @type {string} */ many) => `${n} ${n === 1 ? one : many}`;

// Moving to the next step removes the button that had the focus, so the dialog takes it back.
// Without that, the keys would go to the library behind it.
$effect(() => {
  void step;
  tick().then(() => dialogEl?.focus());
});

onMount(() => {
  dialogEl?.focus();
  invoke('list_backups')
    .then((list) => (backups = /** @type {any} */ (list)))
    .catch((e) => (loadError = String(e?.message ?? e)))
    .finally(() => (loading = false));
});

function close() {
  if (step === 'working') return;
  if (step === 'done') onRestored?.(report);
    else onClose?.();
}

async function restore() {
  step = 'working';
  error = '';
  try {
    report = await invoke('restore_backup', { file: chosen.file });
    step = 'done';
  } catch (e) {
    error = String(e?.message ?? e);
    step = 'confirm';
  }
}

function onKeydown(/** @type {KeyboardEvent} */ e) {
  if (e.key === 'Escape') {
    e.preventDefault();
    if (step === 'confirm') step = 'list';
      else close();
  } else if (step === 'list' && (e.key === 'ArrowDown' || e.key === 'ArrowUp') && backups.length) {
    e.preventDefault();
    const at = chosen ? backups.findIndex((b) => b.file === chosen.file) : -1;
    const next = Math.max(0, Math.min(backups.length - 1, at + (e.key === 'ArrowDown' ? 1 : -1)));
    chosen = backups[next];
    dialogEl?.querySelector(`[data-file="${CSS.escape(chosen.file)}"]`)?.scrollIntoView({ block: 'nearest' });
  }
  // Nothing typed here reaches the library behind.
  e.stopPropagation();
}
</script>

<div class="backdrop" role="presentation" transition:fade|global={{ duration: 70 }} onclick={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="restore-title" tabindex="-1" bind:this={dialogEl} onkeydown={onKeydown}>
    <h2 id="restore-title">Restore from a backup</h2>

    {#if step === 'list'}
      <p class="lead">Pandora keeps a backup of the library's database before it first changes anything. Choose one to put the library back the way it was then.</p>
      <div class="list" role="radiogroup" aria-label="Backups">
        {#if loading}
          <div class="note">Looking for backups…</div>
        {:else if loadError}
          <div class="note error">{loadError}</div>
        {:else if !backups.length}
          <div class="note">There are no backups of this library yet. Pandora takes one before it first changes the library.</div>
        {:else}
          {#each backups as b (b.file)}
            <button
              type="button"
              class="zen-btn row"
              class:selected={chosen?.file === b.file}
              role="radio"
              aria-checked={chosen?.file === b.file}
              data-file={b.file}
              onclick={() => (chosen = b)}
              ondblclick={() => ((chosen = b), (step = 'confirm'))}
            >
              <span class="when">{when(b.taken)}</span>
              <span class="what">
                {b.before_restore ? 'Before a restore' : 'Before a save'}
                · {size(b.size)}
                {#if b.moves}· {plural(b.moves, 'rename', 'renames')} to undo{/if}
              </span>
            </button>
          {/each}
        {/if}
      </div>
      <div class="actions">
        <button type="button" class="zen-btn primary" disabled={!chosen} onclick={() => (step = 'confirm')}>Continue</button>
        <button type="button" class="zen-btn secondary" onclick={close}>Cancel</button>
      </div>
    {:else if step === 'confirm' || step === 'working'}
      <p class="lead strong">Put the library back to {when(chosen.taken)}?</p>
      <ul class="facts">
        <li>Everything changed since then is undone, whether it was changed in Pandora or in calibre.</li>
        <li>
          {#if chosen.moves}{plural(chosen.moves, 'folder or file rename is', 'folder and file renames are')} put back.{:else}No folders or files need to move.{/if}
        </li>
        <li>Cover images you replaced are not brought back.</li>
        <li>Close calibre first. If it has this library open, the restore stops and nothing is changed.</li>
        <li>A copy of the library as it is now is kept, so you can undo this restore afterwards.</li>
      </ul>
      {#if error}
        <div class="note error">{error}{#if !error.startsWith('The library was restored')}<br />Your library was not changed.{/if}</div>
      {/if}
      <div class="actions">
        <button type="button" class="zen-btn primary" disabled={step === 'working'} onclick={restore}>{step === 'working' ? 'Restoring…' : 'Restore'}</button>
        <button type="button" class="zen-btn secondary" disabled={step === 'working'} onclick={() => (step = 'list')}>Back</button>
      </div>
    {:else}
      <p class="lead strong">The library is back to how it was on {when(chosen.taken)}.</p>
      <ul class="facts">
        {#if report.moves_undone}<li>{plural(report.moves_undone, 'rename was', 'renames were')} undone.</li>{/if}
        <li>If calibre is open, restart it to see the restored library.</li>
        <li>To undo this restore, restore the backup marked "Before a restore" at {when(Number(report.safety_copy.match(/(\d+)\.db$/)?.[1] ?? 0))}.</li>
      </ul>
      {#if report.missing_folders}
        <div class="note warn">
          {plural(report.missing_folders, 'book in this backup has', 'books in this backup have')} no folder on disk, for example
          {report.missing_examples.join(', ')}. They were probably added or removed outside Pandora after the backup.
        </div>
      {/if}
      <div class="actions">
        <button type="button" class="zen-btn primary" onclick={close}>Done</button>
      </div>
    {/if}
  </div>
</div>

<style>
.backdrop {
  position: fixed; inset: 0; z-index: 10;
  display: flex; justify-content: center; align-items: flex-start;
  padding-top: 10vh;
  background: rgba(28, 27, 24, 0.18);
}
.dialog {
  width: min(560px, calc(100% - 32px));
  max-height: 76vh;
  display: flex; flex-direction: column;
  background: var(--zen-panel);
  border: 1px solid var(--zen-border); border-radius: 2px;
  box-shadow: 0 14px 34px rgba(27, 28, 30, 0.2);
  outline: none;
}
h2 {
  margin: 0; padding: 18px 20px 6px;
  font-family: var(--zen-font-serif); font-size: 22px; font-weight: 400;
  color: var(--zen-ink);
}
.lead { margin: 0; padding: 4px 20px 12px; font-size: 13px; line-height: 1.55; color: var(--zen-text-secondary); }
.lead.strong { font-size: 14px; color: var(--zen-ink); }
.list { overflow-y: auto; min-height: 0; padding: 4px 8px; border-top: 1px solid var(--zen-border); border-bottom: 1px solid var(--zen-border); }
.row {
  display: flex; flex-direction: column; align-items: flex-start; gap: 2px;
  width: 100%; box-sizing: border-box; padding: 8px 12px; border-radius: 2px; text-align: left;
}
.row:hover, .row:focus, .row.selected { outline: none; background: #F7C7CE; }
.when { font-size: 14px; color: var(--zen-ink); }
.what { font-size: 12px; color: var(--zen-text-muted); }
.facts { margin: 0; padding: 0 20px 12px 38px; font-size: 13px; line-height: 1.55; color: var(--zen-text-secondary); display: flex; flex-direction: column; gap: 6px; }
.note { padding: 12px 20px; font-size: 13px; line-height: 1.5; color: var(--zen-text-faint); }
.note.error { color: var(--zen-red); }
.note.warn { color: var(--zen-text-secondary); background: var(--zen-pill-bg); margin: 0 20px 12px; padding: 10px 12px; border-radius: 2px; }
.actions { display: flex; gap: 10px; padding: 14px 20px 18px; }
.primary, .secondary { height: 38px; padding: 0 18px; border-radius: 2px; font-size: 13px; font-weight: 500; }
.primary { background: var(--zen-red); color: #FFFFFF; border: 1px solid transparent; }
.secondary { border: 1px solid var(--zen-ink); color: var(--zen-ink); }
.primary:disabled, .secondary:disabled { opacity: 0.45; cursor: default; }
</style>
