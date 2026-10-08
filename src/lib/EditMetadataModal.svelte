<script>
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { tick } from 'svelte';
import { open } from '@tauri-apps/plugin-dialog';

let { book, draft, ui, startError, position, stagedElsewhere, onNavigate, onSave, onClose, quitRequest, onQuit } = $props();

let comments = $state('');
let originalComments = '';
let stars = $state(0);
let originalStars = $derived(Math.floor((book.rating ?? 0) / 2));
let tagsText = $state('');
let originalTags = [];
let customCols = $state([]);
let idsText = $state('');
let originalIds = '';
let originalIdKeys = [];
let publisherText = $state('');
let originalPublisher = '';
let publisherNames = $state([]);
let dateText = $state('');
let originalDate = '';
let publishedText = $state('');
let originalPublished = '';
let langOptions = $state([]);
let langCodes = $state([]);
let originalLangs = [];
let langInput = $state('');
// svelte-ignore state_referenced_locally
let customOpen = $state(ui?.customOpen ?? false);
// svelte-ignore state_referenced_locally
let coverOpen = $state(ui?.coverOpen ?? false);
let confirmOpen = $state(false);
let quitting = $state(false);
// svelte-ignore state_referenced_locally
let lastQuit = quitRequest;
let keepBtn = $state();
let coverEdit = $state(null);
let coverError = $state('');
let coverBusy = $state(false);
let titleText = $state('');
let originalTitle = $state('');
let authorsText = $state('');
let originalAuthors = $state([]);
let seriesText = $state('');
let originalSeries = '';
let seriesIndex = $state(1);
let originalIndex = null;
let seriesNames = $state([]);
let loaded = $state(false);
let saving = $state(false);
// svelte-ignore state_referenced_locally
let error = $state(startError ?? '');
let commentsEl = $state();

// Calibre writes dates as UTC text, and "0101-01-01" means undefined. Dates picked in the
// dialog are stored as local midnight converted to UTC, which is what calibre does.
function storedToInput(s) {
  if (!s || s.startsWith('0101-01-01')) return '';
  const d = new Date(s.replace(' ', 'T').replace(/\.\d+/, ''));
  if (Number.isNaN(d.getTime())) return '';
  const p = (n) => String(n).padStart(2, '0');
  return `${String(d.getFullYear()).padStart(4, '0')}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}
function inputToStored(v) {
  if (!v) return '0101-01-01 00:00:00+00:00';
  const [y, m, d] = v.split('-').map(Number);
  // setFullYear avoids JavaScript treating years 0 to 99 as 1900 to 1999.
  const local = new Date(2000, 0, 1);
  local.setFullYear(y, m - 1, d);
  local.setHours(0, 0, 0, 0);
  const iso = local.toISOString();
  return `${iso.slice(0, 10)} ${iso.slice(11, 19)}+00:00`;
}
function langName(code) {
  return langOptions.find((o) => o.code === code)?.name ?? code;
}
// Accepts a language name or a three letter code. Returns false if it can't be understood.
function addLanguage(strict) {
  const t = langInput.trim().toLowerCase();
  if (!t) return true;
  const hit = langOptions.find((o) => o.name.toLowerCase() === t || o.code === t);
  const code = hit ? hit.code : /^[a-z]{3}$/.test(t) ? t : null;
  if (!code) {
    if (strict) error = `"${langInput.trim()}" isn't a language Pandora knows. Pick one from the list or type its three letter code.`;
    return false;
  }
  if (!langCodes.includes(code)) langCodes = [...langCodes, code];
  langInput = '';
  error = '';
  return true;
}
function removeLanguage(code) {
  langCodes = langCodes.filter((c) => c !== code);
}
function langKey(e) {
  if (e.key === 'Enter') {
    e.preventDefault();
    addLanguage(true);
  }
}
// "type:value" pairs separated by commas. Only the first colon splits, so a value can have colons.
function parseIds(text) {
  const items = [];
  const seen = [];
  for (const raw of text.split(',')) {
    const part = raw.trim();
    if (!part) continue;
    const i = part.indexOf(':');
    if (i < 1 || i === part.length - 1) return { error: `"${part}" needs the form type:value` };
    const type = part.slice(0, i).trim().toLowerCase();
    if (seen.includes(type)) return { error: `Only one "${type}" identifier is allowed` };
    seen.push(type);
    items.push({ type, val: part.slice(i + 1).trim() });
  }
  return { items };
}

function parseList(text, isNames) {
  const out = [];
  for (const raw of text.split(isNames ? '&' : ',')) {
    const v = raw.trim();
    if (v && !out.some((o) => o.toLowerCase() === v.toLowerCase())) out.push(v);
  }
  return out;
}
// Existing values that match what is being typed after the last separator.
function suggestionsFor(col) {
  const fragment = col.text.split(col.is_names ? '&' : ',').pop().trim().toLowerCase();
  const chosen = parseList(col.text, col.is_names).map((v) => v.toLowerCase());
  return col.suggestions
    .filter((s) => !chosen.includes(s.toLowerCase()) && s.toLowerCase().includes(fragment))
    .slice(0, 50);
}
function pickSuggestion(col, value) {
  const sep = col.is_names ? ' & ' : ', ';
  const head = col.text
  .split(col.is_names ? '&' : ',')
  .slice(0, -1)
  .map((p) => p.trim())
  .filter((p) => p !== '');
  col.text = [...head, value].join(sep) + sep;
}

function optionId(col, i) {
  return `cc-opt-${col.id}-${i}`;
}
async function moveHighlight(col, delta) {
  const count = suggestionsFor(col).length;
  if (!count) return;
  col.open = true;
  const current = Math.min(col.hi, count - 1);
  col.hi = Math.max(-1, Math.min(current + delta, count - 1));
  if (col.hi < 0) return;
  await tick();
  document.getElementById(optionId(col, col.hi))?.scrollIntoView({ block: 'nearest' });
}
function customKey(e, col) {
  if (e.key === 'ArrowDown') {
    e.preventDefault();
    if (!col.open) {
      col.open = true;
      col.hi = 0;
    } else {
      moveHighlight(col, 1);
    }
  } else if (e.key === 'ArrowUp') {
    e.preventDefault();
    moveHighlight(col, -1);
  } else if (e.key === 'Enter') {
    // Only picks something you actively highlighted, so a typed value is never replaced.
    const list = suggestionsFor(col);
    if (col.open && col.hi >= 0 && list[col.hi]) {
      e.preventDefault();
      pickSuggestion(col, list[col.hi]);
      col.hi = -1;
    }
  } else if (e.key === 'Escape' && col.open) {
    // Closes the list first. The dialog only closes on a second Escape.
    e.stopPropagation();
    col.open = false;
  }
}

// Only "&" separates authors. A comma inside a name belongs to that name.
function parseAuthors(text) {
  return text.split('&').map((a) => a.trim()).filter((a) => a !== '');
}
let renamesFiles = $derived(
  titleText.trim() !== originalTitle ||
    parseAuthors(authorsText).join('\u0001') !== originalAuthors.join('\u0001')
);

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

// Everything is read first and set together, so a saved draft can then be laid on top.
async function loadForm() {
  try {
    const [c, series, x, cols] = await Promise.all([
      invoke('get_comments', { bookId: book.id }),
      invoke('get_series'),
      invoke('get_edit_extras', { bookId: book.id }),
      invoke('get_custom_editor', { bookId: book.id })
    ]);
    comments = c ?? '';
    originalComments = comments;
    stars = originalStars;
    tagsText = book.tags.join(', ');
    originalTags = parseTags(tagsText);
    originalTitle = book.title;
    titleText = book.title;
    originalAuthors = parseAuthors(book.authors);
    authorsText = book.authors;
    seriesText = book.series ?? '';
    originalSeries = seriesText;
    originalIndex = book.series ? book.series_index : null;
    seriesIndex = originalIndex ?? 1;
    seriesNames = series.map((s) => s.name);
    idsText = x.identifiers.map(([t, v]) => `${t}:${v}`).join(', ');
    originalIds = idsText;
    originalIdKeys = x.identifiers.map(([t, v]) => `${t.toLowerCase()}:${v}`);
    publisherText = x.publisher ?? '';
    originalPublisher = publisherText;
    publisherNames = x.publishers;
    dateText = storedToInput(x.timestamp);
    originalDate = dateText;
    publishedText = storedToInput(x.pubdate);
    originalPublished = publishedText;
    langOptions = x.language_options;
    langCodes = x.languages;
    originalLangs = [...x.languages];
    customCols = cols.map((col) => {
      const text = col.values.join(col.is_names ? ' & ' : ', ');
      return { ...col, text, original: text, open: false, hi: -1 };
    });
    if (draft) restoreDraft(draft);
    loaded = true;
  } catch (e) {
    error = e?.message ?? String(e);
  }
}

$effect(() => {
  loadForm();
});

$effect(() => {
  if (loaded) commentsEl?.focus();
});

$effect(() => {
  if (confirmOpen) keepBtn?.focus();
});

// Works out what changed on this book. Returns null when an entry is invalid, and shows the
// problem unless quiet, so Next, Previous and OK never move on from a bad entry.
function buildEdit(quiet = false) {
  const fail = (message) => {
    if (!quiet) error = message;
    return null;
  };
  const edit = {};
  if (comments !== originalComments) edit.comments = comments;
  if (stars !== originalStars) edit.rating = stars;

  if (keyOf(parseTags(tagsText)) !== keyOf(originalTags)) edit.tags = parseTags(tagsText);

  if (publisherText.trim() !== originalPublisher) edit.publisher = publisherText.trim();
  if (dateText !== originalDate) edit.date_added = inputToStored(dateText);
  if (publishedText !== originalPublished) edit.published = inputToStored(publishedText);
  if (coverEdit) edit.cover = coverEdit.action === 'set' ? { action: 'set', path: coverEdit.path } : { action: 'remove' };
  if (langInput.trim()) {
    if (quiet || !addLanguage(true)) return null;
  }
  if (langCodes.join(',') !== originalLangs.join(',')) edit.languages = [...langCodes];
  if (idsText !== originalIds) {
    const parsed = parseIds(idsText);
    if (parsed.error) return fail(parsed.error);
    const now = parsed.items.map((i) => `${i.type}:${i.val}`);
    if (keyOf(now) !== keyOf(originalIdKeys)) edit.identifiers = parsed.items;
  }

  // Capitalization counts as a change, because calibre renames the shared value everywhere.
  const customEdits = [];
  for (const col of customCols) {
    const now = parseList(col.text, col.is_names);
    if (keyOf(now) !== keyOf(parseList(col.original, col.is_names))) {
      customEdits.push({ column_id: col.id, values: now });
    }
  }
  if (customEdits.length) edit.custom = customEdits;

  // Capitalization counts as a change here too, since it renames the folder and files.
  const newTitle = titleText.trim();
  if (newTitle !== originalTitle) edit.title = newTitle;
  const newAuthors = parseAuthors(authorsText);
  if (newAuthors.join('\u0001') !== originalAuthors.join('\u0001')) edit.authors = newAuthors;
  // Capitalization counts as a change, because calibre renames the shared series everywhere.
  const seriesName = seriesText.trim();
  const seriesChanged = seriesName !== originalSeries;
  if (seriesChanged) edit.series = seriesName;
  if (seriesName !== '' && (seriesChanged || seriesIndex !== originalIndex)) {
    if (typeof seriesIndex !== 'number' || !Number.isFinite(seriesIndex)) {
      return fail('Series number must be a number');
    }
    edit.series_index = seriesIndex;
  }
  return edit;
}

// Everything typed into the form, so it can be put back when you return to this book.
function snapshot() {
  return {
    comments, stars, tagsText, idsText, publisherText, dateText, publishedText,
    langCodes: [...langCodes], titleText, authorsText, seriesText, seriesIndex,
    custom: customCols.map((c) => ({ id: c.id, text: c.text })),
    coverEdit: coverEdit ? { ...coverEdit } : null
  };
}

function restoreDraft(d) {
  comments = d.comments;
  stars = d.stars;
  tagsText = d.tagsText;
  idsText = d.idsText;
  publisherText = d.publisherText;
  dateText = d.dateText;
  publishedText = d.publishedText;
  langCodes = [...d.langCodes];
  titleText = d.titleText;
  authorsText = d.authorsText;
  seriesText = d.seriesText;
  seriesIndex = d.seriesIndex;
  for (const x of d.custom) {
    const col = customCols.find((c) => c.id === x.id);
    if (col) col.text = x.text;
  }
  coverEdit = d.coverEdit ? { ...d.coverEdit } : null;
}

// Checks this book and hands its changes to the page, which keeps them until OK.
function stage() {
  const edit = buildEdit();
  if (!edit) return null;
  error = '';
  return { edit, draft: snapshot() };
}

function go(delta) {
  const entry = stage();
  if (entry) onNavigate(delta, entry, { customOpen, coverOpen });
}

async function save() {
  const entry = stage();
  if (!entry) return;
  saving = true;
  try {
    await onSave(entry);
  } catch (e) {
    error = e?.message ?? String(e);
    saving = false;
  }
}

function isDirty() {
  if (!loaded) return false;
  const edit = buildEdit(true);
  return edit === null || Object.keys(edit).length > 0;
}

function pendingCount() {
  return stagedElsewhere + (isDirty() ? 1 : 0);
}

function requestClose() {
  if (pendingCount() === 0) onClose();
  else confirmOpen = true;
}

// The window itself is being closed. Never stop in the middle of a save.
function handleQuit() {
  if (saving) return;
  if (pendingCount() === 0) {
    onQuit();
    return;
  }
  quitting = true;
  confirmOpen = true;
}

$effect(() => {
  if (quitRequest !== lastQuit) {
    lastQuit = quitRequest;
    handleQuit();
  }
});

async function browseCover() {
  coverError = '';
  const picked = await open({
    multiple: false,
    directory: false,
    filters: [{ name: 'Images', extensions: ['jpg', 'jpeg', 'png', 'gif', 'webp', 'bmp'] }]
  });
  if (!picked) return;
  coverBusy = true;
  try {
    const preview = await invoke('preview_cover', { path: picked });
    coverEdit = { action: 'set', path: picked, preview };
  } catch (e) {
    coverError = String(e);
  } finally {
    coverBusy = false;
  }
}

function handleKey(e) {
  if (e.key === 'Escape') {
    // Esc goes up one level: the prompt, then an open panel, then the dialog itself.
    if (confirmOpen) {
      confirmOpen = false;
      quitting = false;
    } else if (customOpen || coverOpen) {
      customOpen = false;
      coverOpen = false;
    } else {
      requestClose();
    }
  }
}
</script>

<svelte:window onkeydown={handleKey} />

<div class="overlay">
  {#if confirmOpen}
    <div class="confirm-scrim">
      <div class="confirm" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title">
        <p id="confirm-title">Discard your changes to {pendingCount()} {pendingCount() === 1 ? 'book' : 'books'}{quitting ? ' and quit' : ''}?</p>
        <div class="actions">
          <button type="button" class="zen-btn btn-primary" bind:this={keepBtn} onclick={() => { confirmOpen = false; quitting = false; }}>Keep editing</button>
          <button type="button" class="zen-btn btn-secondary" onclick={quitting ? onQuit : onClose}>{quitting ? 'Discard and quit' : 'Discard'}</button>
        </div>
      </div>
    </div>
  {/if}
  <div class="shell" inert={confirmOpen}>
    <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="edit-title">
      <div class="dialog-body">
        <h2 id="edit-title">Edit metadata</h2>
        <div class="grid">
          <label for="em-title">Title</label>
          <input id="em-title" bind:value={titleText} disabled={!loaded} />
          <label for="em-authors">Author(s)</label>
          <input id="em-authors" bind:value={authorsText} disabled={!loaded} placeholder="Separate authors with &amp;" />
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

          <label for="em-ids">Ids</label>
          <input id="em-ids" bind:value={idsText} disabled={!loaded} placeholder="isbn:9781101489147, doi:10.1000/182" />

          <label for="em-date">Date</label>
          <div class="row">
            <input id="em-date" type="date" bind:value={dateText} disabled={!loaded} />
            <button type="button" class="zen-btn clear-btn" onclick={() => (dateText = '')} disabled={!loaded || dateText === ''}>Clear</button>
          </div>

          <label for="em-published">Published</label>
          <div class="row">
            <input id="em-published" type="date" bind:value={publishedText} disabled={!loaded} />
            <button type="button" class="zen-btn clear-btn" onclick={() => (publishedText = '')} disabled={!loaded || publishedText === ''}>Clear</button>
          </div>

          <label for="em-publisher">Publisher</label>
          <input id="em-publisher" list="em-publisher-list" bind:value={publisherText} disabled={!loaded} placeholder="No publisher" />
          <datalist id="em-publisher-list">
            {#each publisherNames as name}<option value={name}></option>{/each}
          </datalist>

          <label for="em-lang">Languages</label>
          <div class="lang-box">
            {#each langCodes as code (code)}
              <span class="lang-chip">{langName(code)}<button type="button" class="zen-btn lang-x" aria-label="Remove {langName(code)}" onclick={() => removeLanguage(code)}>×</button></span>
            {/each}
            <input id="em-lang" list="em-lang-list" bind:value={langInput} disabled={!loaded} placeholder="Add a language" onchange={() => addLanguage(false)} onkeydown={langKey} />
            <datalist id="em-lang-list">
            {#each langOptions as o}<option value={o.name}></option>{/each}
          </datalist>
          </div>
        </div>

        <div class="comments-wrap">
          <label for="em-comments">Comments (HTML)</label>
          <div class="ta-box">
            <textarea id="em-comments" bind:this={commentsEl} bind:value={comments} disabled={!loaded}></textarea>
          </div>
        </div>
      </div>

      <div class="dialog-foot">

        {#if renamesFiles}
          <div class="hint">Saving will rename this book's folder and files on disk.</div>
        {/if}
        {#if error}<div class="error">{error}</div>{/if}

        <div class="foot-row">
          <div class="nav">
            <button type="button" class="zen-btn btn-secondary nav-btn" onclick={() => go(-1)} disabled={!loaded || saving || coverBusy || position.n <= 1}>‹ Previous</button>
            <button type="button" class="zen-btn btn-secondary nav-btn" onclick={() => go(1)} disabled={!loaded || saving || coverBusy || position.n >= position.total}>Next ›</button>
            <span class="hint">Book {position.n} of {position.total}{pendingCount() ? ` · ${pendingCount()} edited` : ''}</span>
          </div>
          <div class="actions">
            <!-- disabled until the form has loaded, so we can never save an empty draft over real text -->
            <button class="zen-btn btn-primary" onclick={save} disabled={!loaded || saving || coverBusy}>OK</button>
            <button class="zen-btn btn-secondary" onclick={requestClose}>Cancel</button>
          </div>
        </div>
      </div>
    </div>

    {#if customCols.length}
      <div id="cc-panel" class="cc-panel" class:open={customOpen} inert={!customOpen}>
        <div class="cc-inner">
          <h3>Custom metadata</h3>
          {#each customCols as col (col.id)}
            {@const list = suggestionsFor(col)}
            <div class="cc-item">
              <label for="em-cc-{col.id}">{col.name}</label>
              <div class="cc-box">
                <input
                  id="em-cc-{col.id}"
                  role="combobox"
                  aria-expanded={col.open && list.length > 0}
                  aria-controls="cc-list-{col.id}"
                  aria-autocomplete="list"
                  autocomplete="off"
                  bind:value={col.text}
                  disabled={!loaded}
                  placeholder={col.is_names ? 'Separate with &' : 'Separate with commas'}
                  onfocus={() => { col.open = true; col.hi = -1; }}
                  oninput={() => { col.open = true; col.hi = -1; }}
                  onblur={() => (col.open = false)}
                  onkeydown={(e) => customKey(e, col)}
                />
                {#if col.open && list.length}
                  <div class="cc-list" id="cc-list-{col.id}" role="listbox" tabindex="-1" onmousedown={(e) => e.preventDefault()}>
                    {#each list as s, i}
                      <div
                        id={optionId(col, i)}
                        class="cc-opt"
                        class:hi={i === col.hi}
                        role="option"
                        aria-selected={i === col.hi}
                        tabindex="-1"
                        onmousedown={(e) => { e.preventDefault(); pickSuggestion(col, s); col.hi = -1; }}
                        onmouseenter={() => (col.hi = i)}
                      >{s}</div>
                    {/each}
                  </div>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/if}

    <div id="cv-panel" class="cv-panel" class:open={coverOpen} inert={!coverOpen}>
      <div class="cv-inner">
        <h3>Cover</h3>
        <div class="cv-preview">
          {#if coverEdit?.action === 'set'}
            <img src={coverEdit.preview.data_url} alt="Preview of the new cover" />
          {:else if coverEdit?.action === 'remove'}
            <span class="hint">The cover will be removed when you press OK.</span>
          {:else if book.cover_path}
            <img src={convertFileSrc(book.cover_path) + '?v=' + book.cover_version} alt="Current cover of {book.title}" />
          {:else}
            <span class="hint">No cover</span>
          {/if}
        </div>
        {#if coverEdit?.action === 'set'}
          <p class="hint">{coverEdit.preview.width} x {coverEdit.preview.height}. {coverEdit.preview.note}. It is saved when you press OK.</p>
        {/if}
        {#if coverBusy}<p class="hint">Preparing the cover…</p>{/if}
        {#if coverError}<p class="error">{coverError}</p>{/if}
        <div class="cv-actions">
          <button type="button" class="zen-btn cv-btn" onclick={browseCover} disabled={!loaded || coverBusy}>Browse…</button>
          <button type="button" class="zen-btn cv-btn" onclick={() => (coverEdit = { action: 'remove' })} disabled={!loaded || !book.cover_path || coverEdit?.action === 'remove'}>Remove cover</button>
          {#if coverEdit}
            <button type="button" class="zen-btn cv-btn" onclick={() => { coverEdit = null; coverError = ''; }}>Undo change</button>
          {/if}
        </div>
      </div>
    </div>

    <div class="tabs">
      {#if customCols.length}
        <button type="button" class="side-tab" aria-expanded={customOpen} aria-controls="cc-panel" onclick={() => { customOpen = !customOpen; if (customOpen) coverOpen = false; }}>Custom</button>
      {/if}
      <button type="button" class="side-tab" aria-expanded={coverOpen} aria-controls="cv-panel" onclick={() => { coverOpen = !coverOpen; if (coverOpen) customOpen = false; }}>Cover</button>
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

.shell {
  position: relative;
  display: flex;
  max-width: calc(100vw - 80px);
  background: var(--zen-panel);
  border: 1px solid var(--zen-border);
  border-radius: 2px;
}

.dialog {
  width: min(780px, 92vw);
  height: min(94vh, 860px);
  flex: 1 1 auto;
  min-width: 0;
  container-type: inline-size;
  display: flex;
  flex-direction: column;
}
.dialog-body {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  padding: 20px 24px 8px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.dialog-foot {
  flex: none;
  padding: 8px 24px 16px;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.comments-wrap {
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.ta-box {
  position: relative;
  flex: 1 1 auto;
  min-height: 100px;
}
.ta-box textarea {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  max-height: 100%;
}

h2 { margin: 0; font-family: var(--zen-font-serif); font-size: 22px; font-weight: 400; transform: scaleY(1.15); }
.hint { margin: 0; font-size: 12px; color: var(--zen-text-faint); }
.grid {
  display: grid;
  grid-template-columns: 90px minmax(0, 1fr) auto minmax(0, 1fr);
  gap: 8px 12px;
  align-items: center;
}
.grid > input:not(#em-publisher) { grid-column: 2 / -1; }
@container (max-width: 560px) {
  .grid { grid-template-columns: 90px minmax(0, 1fr); }
  .grid > input:not(#em-publisher) { grid-column: auto; }
}
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
textarea { font-family: monospace; resize: vertical; min-height: 100px; }
.row { display: flex; gap: 8px; }
.num { width: 80px; flex: none; }
.cc-panel {
  flex: none;
  display: flex;
  width: 0;
  overflow: hidden;
  transition: width 90ms ease;
}
.cc-panel.open { width: 340px; }
.cv-panel {
  order: -1;
  flex: none;
  display: flex;
  width: 0;
  overflow: hidden;
  transition: width 90ms ease;
}
.cv-panel.open { width: 300px; }
.cv-inner {
  flex: none;
  width: 300px;
  box-sizing: border-box;
  overflow-y: auto;
  padding: 24px;
  border-right: 1px solid var(--zen-border);
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.cv-inner h3 {
  margin: 0 0 4px;
  font-family: var(--zen-font-serif);
  font-size: 17px;
  font-weight: 400;
  transform: scaleY(1.13);
  transform-origin: left top;
}
.cv-preview {
  display: flex;
  align-items: center;
  justify-content: center;
  min-height: 200px;
  padding: 12px;
  text-align: center;
  background: var(--zen-bg);
  border: 1px solid var(--zen-border);
  border-radius: 2px;
}
.cv-preview img { max-width: 100%; max-height: 360px; display: block; }
.cv-actions { display: flex; flex-wrap: wrap; gap: 8px; }
.cv-btn {
  font-size: 12px;
  padding: 6px 12px;
  border: 1px solid var(--zen-border);
  border-radius: 2px;
  background: var(--zen-panel);
  color: var(--zen-ink);
}
.cv-btn:disabled { opacity: 0.4; cursor: default; }
.cc-inner {
  flex: none;
  width: 340px;
  box-sizing: border-box;
  overflow-y: auto;
  padding: 24px;
  border-left: 1px solid var(--zen-border);
  display: flex;
  flex-direction: column;
  gap: 14px;
}
.cc-inner h3 {
  margin: 0 0 4px;
  font-family: var(--zen-font-serif);
  font-size: 17px;
  font-weight: 400;
  transform: scaleY(1.13);
  transform-origin: left top;
}
.cc-item { display: flex; flex-direction: column; gap: 6px; }
.cc-box { position: relative; }
.cc-list {
  position: absolute;
  top: calc(100% + 2px);
  left: 0;
  right: 0;
  max-height: 200px;
  overflow-y: auto;
  background: var(--zen-panel);
  border: 1px solid var(--zen-border);
  border-radius: 2px;
  box-shadow: 0 6px 16px rgba(28, 27, 24, 0.12);
  z-index: 5;
}
.cc-opt { padding: 6px 10px; font-size: 13px; color: var(--zen-text-secondary); cursor: pointer; }
.cc-opt.hi { background: #F7C7CE; color: var(--zen-red); }
.tabs {
  position: absolute;
  top: 28px;
  left: 100%;
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.side-tab {
  writing-mode: vertical-rl;
  padding: 14px 7px;
  font: inherit;
  font-size: 12px;
  letter-spacing: 0.06em;
  color: var(--zen-panel);
  background: var(--zen-red);
  border: 1px solid var(--zen-red-dark);
  border-left: none;
  border-radius: 0 2px 2px 0;
  cursor: pointer;
}
.side-tab:focus-visible { outline: 2px solid var(--zen-red); outline-offset: 2px; }
.lang-box { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
.lang-box input { width: auto; flex: 1 1 140px; }
.lang-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  padding: 3px 4px 3px 9px;
  border: 1px solid var(--zen-border);
  border-radius: 2px;
  background: var(--zen-bg);
  color: var(--zen-text-secondary);
}
.lang-x { font-size: 14px; line-height: 1; padding: 0 5px; color: var(--zen-text-faint); }
.lang-x:hover { color: var(--zen-red); }
.error { color: var(--zen-red); font-size: 12px; }
.rating-row { display: flex; align-items: center; gap: 10px; }
.rating-stars { display: flex; gap: 2px; }
.star-btn { background: none; border: none; padding: 2px; cursor: pointer; display: flex; }
.star-btn:disabled { cursor: default; }
.clear-btn { font-size: 12px; color: var(--zen-text-faint); }
.clear-btn:disabled { opacity: 0.4; cursor: default; }
.actions { display: flex; justify-content: flex-end; gap: 10px; }
.foot-row { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 10px; }
.nav { display: flex; align-items: center; gap: 8px; }
.nav-btn { height: 36px; padding: 0 12px; font-size: 13px; }
.confirm-scrim {
  position: absolute;
  inset: 0;
  z-index: 20;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(28, 27, 24, 0.35);
}
.confirm {
  max-width: 360px;
  padding: 20px 24px;
  display: flex;
  flex-direction: column;
  gap: 14px;
  background: var(--zen-panel);
  border: 1px solid var(--zen-border);
  border-radius: 2px;
}
.confirm p { margin: 0; font-size: 14px; color: var(--zen-ink); }
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
