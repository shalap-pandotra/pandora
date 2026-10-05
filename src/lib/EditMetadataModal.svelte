<script>
import { invoke } from '@tauri-apps/api/core';

let { book, onClose, onSaved } = $props();

let comments = $state('');
let originalComments = '';
let stars = $state(0);
let originalStars = $derived(Math.floor((book.rating ?? 0) / 2));
let tagsText = $state('');
let originalTags = [];
let seriesText = $state('');
let originalSeries = '';
let seriesIndex = $state(1);
let originalIndex = null;
let seriesNames = $state([]);
let loaded = $state(false);
let saving = $state(false);
let error = $state('');
let commentsEl = $state();

async function seriesChosen() {
  const name = seriesText.trim();
  if (name === '') return;
  if (name.toLowerCase() === originalSeries.toLowerCase()) {
    seriesIndex = originalIndex ?? 1;
    return;
  }
  seriesIndex = await invoke('get_next_series_index', { series: name });
}

function parseTags(text) {
  const out = [];
  for (const raw of text.split(',')) {
    const t = raw.trim();
    if (t && !out.some((o) => o.toLowerCase() === t.toLowerCase())) out.push(t);
  }
  return out;
}

// Order alone doesn't count as a change. Capitalization does, because calibre
// renames the shared tag everywhere when the spelling differs.
const keyOf = (list) => list.slice().sort().join('\u0001');

$effect(() => {
  invoke('get_comments', { bookId: book.id }).then((r) => {
    comments = r ?? '';
    originalComments = comments;
    stars = originalStars;
    tagsText = book.tags.join(', ');
    originalTags = parseTags(tagsText);
    seriesText = book.series ?? '';
    originalSeries = seriesText;
    originalIndex = book.series ? book.series_index : null;
    seriesIndex = originalIndex ?? 1;
    invoke('get_series').then((r) => (seriesNames = r.map((s) => s.name)));
    loaded = true;
  });
});

$effect(() => {
  if (loaded) commentsEl?.focus();
});

async function save() {
  const edit = {};
  if (comments !== originalComments) edit.comments = comments;
  if (stars !== originalStars) edit.rating = stars;
  if (keyOf(parseTags(tagsText)) !== keyOf(originalTags)) edit.tags = parseTags(tagsText);

  // Capitalization counts as a change, because calibre renames the shared series everywhere.
  const seriesName = seriesText.trim();
  const seriesChanged = seriesName !== originalSeries;
  if (seriesChanged) edit.series = seriesName;
  if (seriesName !== '' && (seriesChanged || seriesIndex !== originalIndex)) {
    if (typeof seriesIndex !== 'number' || !Number.isFinite(seriesIndex)) {
      error = 'Series number must be a number';
      return;
    }
    edit.series_index = seriesIndex;
  }
  if (Object.keys(edit).length === 0) {
    onClose();
    return;
  }
  saving = true;
  error = '';
  try {
    await invoke('save_metadata', { bookId: book.id, edit });
    onSaved(book.id);
  } catch (e) {
    error = String(e);
    saving = false;
  }
}

function handleKey(e) {
  if (e.key === 'Escape') onClose();
}
</script>

<svelte:window onkeydown={handleKey} />

<div class="overlay">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="edit-title">
    <h2 id="edit-title">Edit metadata</h2>
    <p class="hint">Greyed out fields can't be edited yet. They arrive in later updates.</p>

    <div class="grid">
      <label for="em-title">Title</label>
      <input id="em-title" value={book.title} disabled />

      <label for="em-authors">Author(s)</label>
      <input id="em-authors" value={book.authors} disabled />

      <label for="em-series">Series</label>
      <div class="row">
        <input id="em-series" list="em-series-list" bind:value={seriesText} onchange={seriesChosen} disabled={!loaded} placeholder="No series" />
        <datalist id="em-series-list">
          {#each seriesNames as name}<option value={name}></option>{/each}
        </datalist>
        <input class="num" type="number" step="any" aria-label="Series number" bind:value={seriesIndex} disabled={!loaded || seriesText.trim() === ''} />
      </div>
      <span class="field-label">Rating</span>
      <div class="rating-row">
        <div class="rating-stars" role="group" aria-label="Rating">
          {#each [1, 2, 3, 4, 5] as n}
            <button
              type="button"
              class="star-btn"
              aria-label="Set rating to {n} {n === 1 ? 'star' : 'stars'}"
              aria-pressed={stars === n}
              disabled={!loaded}
              onclick={() => (stars = n)}
            >
              <svg width="20" height="20" viewBox="0 0 24 24" fill={n <= stars ? 'var(--zen-red)' : 'none'} stroke="var(--zen-red)" stroke-width="1.5">
                <path d="M12 2l2.9 6.6 7.1.7-5.4 4.8 1.6 7-6.2-3.7-6.2 3.7 1.6-7-5.4-4.8 7.1-.7z" />
              </svg>
            </button>
          {/each}
        </div>
        <button type="button" class="zen-btn clear-btn" onclick={() => (stars = 0)} disabled={!loaded || stars === 0}>Clear</button>
      </div>

      <label for="em-tags">Tags</label>
      <input id="em-tags" bind:value={tagsText} disabled={!loaded} placeholder="Separate tags with commas" />
    </div>

    <label for="em-comments">Comments (HTML)</label>
    <textarea id="em-comments" bind:this={commentsEl} bind:value={comments} rows="12" disabled={!loaded}></textarea>

    {#if error}<div class="error">{error}</div>{/if}

    <div class="actions">
      <!-- disabled until comments have loaded, so we can never save an empty draft over real text -->
      <button class="zen-btn btn-primary" onclick={save} disabled={!loaded || saving}>OK</button>
      <button class="zen-btn btn-secondary" onclick={onClose}>Cancel</button>
    </div>
  </div>
</div>

<style>
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(28, 27, 24, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.dialog {
  width: min(720px, 92vw);
  max-height: 90vh;
  overflow-y: auto;
  background: var(--zen-panel);
  border: 1px solid var(--zen-border);
  border-radius: 2px;
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}
h2 { margin: 0; font-family: var(--zen-font-serif); font-size: 22px; }
.hint { margin: 0; font-size: 12px; color: var(--zen-text-faint); }
.grid { display: grid; grid-template-columns: 90px 1fr; gap: 10px 12px; align-items: center; }
label, .field-label { font-size: 12px; font-weight: 600; color: var(--zen-text-secondary); }
input, textarea {
  font: inherit;
  font-size: 13px;
  padding: 8px 10px;
  border: 1px solid var(--zen-border);
  border-radius: 2px;
  background: var(--zen-panel);
  color: var(--zen-ink);
  box-sizing: border-box;
  width: 100%;
}
input:disabled, textarea:disabled { background: var(--zen-bg); color: var(--zen-text-faint); }
textarea { font-family: monospace; resize: vertical; }
.row { display: flex; gap: 8px; }
.num { width: 80px; flex: none; }
.error { color: var(--zen-red); font-size: 12px; }
.rating-row { display: flex; align-items: center; gap: 10px; }
.rating-stars { display: flex; gap: 2px; }
.star-btn { background: none; border: none; padding: 2px; cursor: pointer; display: flex; }
.star-btn:disabled { cursor: default; }
.clear-btn { font-size: 12px; color: var(--zen-text-faint); }
.clear-btn:disabled { opacity: 0.4; cursor: default; }
.actions { display: flex; justify-content: flex-end; gap: 10px; }
.btn-primary, .btn-secondary {
  height: 36px;
  padding: 0 18px;
  border-radius: 2px;
  font-size: 13px;
  font-weight: 500;
}
.btn-primary { background: var(--zen-red); color: white; }
.btn-secondary { background: var(--zen-panel); border: 1px solid var(--zen-border); color: var(--zen-ink); }
.btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
