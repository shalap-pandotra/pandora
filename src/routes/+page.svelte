<script>
import '../lib/theme.css';
import { invoke } from '@tauri-apps/api/core';
import TopBar from '../lib/TopBar.svelte';
import Sidebar from '../lib/Sidebar.svelte';
import { onMount } from 'svelte';
import PreviewPanel from '../lib/PreviewPanel.svelte';
import EditMetadataModal from '../lib/EditMetadataModal.svelte';
import RestoreDialog from '../lib/RestoreDialog.svelte';
import BookList from '../lib/BookList.svelte';
import StatusBar from '../lib/StatusBar.svelte';
import { open } from '@tauri-apps/plugin-dialog';
import { getCurrentWindow } from '@tauri-apps/api/window';

let libraryPath = $state('');
let libraryLoaded = $state(false);
let books = $state([]);
let selectedBook = $state(null);
let sessionBooks = $state([]);
let editIndex = $state(-1);
let restoreOpen = $state(false);
let staged = $state([]);
let sessionUi = $state({ customOpen: false, coverOpen: false });
let sessionError = $state('');
let quitRequest = $state(0);
let editingBook = $derived(editIndex >= 0 ? sessionBooks[editIndex] : null);
let sidebarVersion = $state(0);
let searchTerm = $state('');
let currentFilter = $state({ author_id: null, series_id: null, tag_id: null, search: null, custom_column_id: null, custom_value_id: null });

let gridHeight = $state(224);

const PREVIEW_HEIGHT_KEY = 'pandora.previewHeight';
const RESIZER_HEIGHT = 6;
let sizeRestored = false;
let searchReturnTo = null; 
let previewWanted = null;
function applyPreviewWanted() {
  if (previewWanted === null || !mainColEl) return;
  const main = mainColEl.getBoundingClientRect().height;
  gridHeight = Math.min(main - 296, Math.max(120, main - previewWanted - RESIZER_HEIGHT));
}
function savePreviewHeight() {
  if (!mainColEl) return;
  previewWanted = Math.round(mainColEl.getBoundingClientRect().height - gridHeight - RESIZER_HEIGHT);
  try {
    localStorage.setItem(PREVIEW_HEIGHT_KEY, String(previewWanted));
  } catch {}
}
function restorePreviewHeight() {
  try {
    const saved = Number(localStorage.getItem(PREVIEW_HEIGHT_KEY));
    if (!Number.isFinite(saved) || saved <= 0) return;
    previewWanted = saved;
    applyPreviewWanted();
  } catch {}
}
let dragging = false;
let mainColEl = $state();
/** @type {any} */
let previewPanel = $state();


let cachedRect = null;
let pendingHeight = null;
let rafPending = false;
let dragOffset = 0;

let requestSeq = 0;
let allBooks = null;

onMount(() => {
  const state = new WeakMap(); // el -> { current, target, rafId, timer, edge }

  function tick(el) {
    const s = state.get(el);
    if (!s) return;
    const diff = s.target - s.current;
    if (Math.abs(diff) > 0.01) {
      s.current += diff * 0.35;
      el.style.setProperty('--zen-scrollbar-alpha', s.current);
      s.rafId = requestAnimationFrame(() => tick(el));
    } else {
      s.current = s.target;
      el.style.setProperty('--zen-scrollbar-alpha', s.current);
      s.rafId = null;
    }
  }

  function showBar(el, nearEdge = false) {
    let s = state.get(el);
    if (!s) {
      s = { current: 0, target: 0, rafId: null, timer: null, edge: false };
      state.set(el, s);
    }
    s.edge = nearEdge;
    s.target = 1;
    if (!s.rafId) s.rafId = requestAnimationFrame(() => tick(el));

    clearTimeout(s.timer);
    s.timer = setTimeout(function hide() {
      if (s.edge && el.matches(':hover')) {
        s.timer = setTimeout(hide, 700);
        return;
      }
      s.target = 0;
      if (!s.rafId) s.rafId = requestAnimationFrame(() => tick(el));
    }, 700);
  }

  function handleScroll(e) {
    const el = e.target;
    if (!(el instanceof HTMLElement)) return;
    // A scroll with the pointer on the bar keeps it there, so the edge state is left as it was.
    showBar(el, state.get(el)?.edge ?? false);
  }

  /** The nearest scroll area at or above an element. */
  function scrollerAt(el) {
    for (let node = el; node && node !== document.body; node = node.parentElement) {
      if (!(node instanceof HTMLElement)) continue;
      const oy = getComputedStyle(node).overflowY;
      if ((oy === 'auto' || oy === 'scroll') && node.scrollHeight > node.clientHeight + 1) return node;
    }
    return null;
  }

  let moveFrame = 0;
  let lastMove = null;
  function handlePointerMove(e) {
    lastMove = e;
    if (moveFrame) return;
    moveFrame = requestAnimationFrame(() => {
      moveFrame = 0;
      const el = lastMove?.target instanceof Element ? scrollerAt(lastMove.target) : null;
      if (!el) return;
      const bar = el.offsetWidth - el.clientWidth;
      showBar(el, lastMove.clientX >= el.getBoundingClientRect().right - bar - 24);
    });
  }

  document.addEventListener('scroll', handleScroll, true);
  document.addEventListener('pointermove', handlePointerMove, true);

  function handleKeydown(e) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'o') {
      e.preventDefault();
      pickLibraryFolder();
      return;
    }
    // V opens the selected book, unless something is being typed or edited.
    const typing = e.target instanceof HTMLElement && (e.target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(e.target.tagName));
    if (e.key.toLowerCase() === 'v' && !e.ctrlKey && !e.metaKey && !e.altKey && !typing && editIndex < 0) {
      e.preventDefault();
      previewPanel?.openBook();
    }
    if (e.key === '/' && !e.ctrlKey && !e.metaKey && !e.altKey && !typing && editIndex < 0) {
      e.preventDefault();
      const search = document.getElementById('lib-search');
      const active = document.activeElement;
      searchReturnTo = active instanceof HTMLElement && active !== document.body ? active : null;
      search?.addEventListener('blur', () => (searchReturnTo = null), { once: true });
      search?.focus();
      search?.select();
    } else if (e.key === 'Escape' && e.target instanceof HTMLElement && e.target.id === 'lib-search') {
      e.preventDefault();
      const back = searchReturnTo;
      e.target.blur();
      if (back?.isConnected) back.focus();
    }
  }
  window.addEventListener('keydown', handleKeydown);

  let unlistenClose;
  getCurrentWindow()
    .onCloseRequested((event) => {
      if (editIndex >= 0) {
        event.preventDefault();
        quitRequest += 1;
      }
    })
    .then((fn) => (unlistenClose = fn));

  invoke('load_last_library_path').then((savedPath) => {
    if (savedPath) {
      libraryPath = savedPath;
      openLibrary();
    }
  });

  return () => {
    document.removeEventListener('scroll', handleScroll, true);
    document.removeEventListener('pointermove', handlePointerMove, true);
    cancelAnimationFrame(moveFrame);
    window.removeEventListener('keydown', handleKeydown);
    unlistenClose?.();
  };
});

$effect(() => {
  if (!mainColEl) return;

  if (!sizeRestored) {
    sizeRestored = true;
    restorePreviewHeight();
  }

  const resizeObserver = new ResizeObserver(() => {
    const rect = mainColEl.getBoundingClientRect();
    const gridMin = 120;
    const previewMin = 296;
    const max = rect.height - previewMin;
    if (gridHeight > max) {
      gridHeight = Math.max(gridMin, max);
    }
    applyPreviewWanted();
  });
  resizeObserver.observe(mainColEl);

  return () => resizeObserver.disconnect();
});

function isWholeLibrary(f) {
  return f.author_id == null && f.series_id == null && f.tag_id == null && !f.search && f.custom_column_id == null && f.custom_value_id == null;
}

async function refreshBooks(keepId = null) {
  const seq = ++requestSeq;
  const whole = isWholeLibrary(currentFilter);
  let result = whole ? allBooks : null;
  if (!result) {
    result = await invoke('get_books_filtered', { filter: currentFilter });
    if (seq !== requestSeq) return; 
    if (whole) allBooks = result;
  }
  books = result;
  selectedBook = (keepId !== null && books.find((b) => b.id === keepId)) || (books.length ? books[0] : null);
}

async function handleFilterChange(filter) {
  currentFilter = { ...currentFilter, ...filter };
  await refreshBooks();
}

async function handleSearch(term) {
  currentFilter = { ...currentFilter, search: term || null };
  await refreshBooks();
}

function handleReset() {
  searchTerm = '';
  currentFilter = { ...currentFilter, search: null };
  refreshBooks();
}

function openEditor() {
  if (!selectedBook) return;
  sessionBooks = books.slice();
  editIndex = Math.max(0, sessionBooks.findIndex((b) => b.id === selectedBook.id));
  staged = [];
  sessionUi = { customOpen: false, coverOpen: false };
  sessionError = '';
}

function quitApp() {
  getCurrentWindow().destroy();
}

function closeEditor() {
  editIndex = -1;
  sessionBooks = [];
  staged = [];
  sessionError = '';
}

// A book with no changes is not staged, and one that was reverted drops out again.
function upsertStaged(id, entry) {
  const at = staged.findIndex((s) => s.id === id);
  if (Object.keys(entry.edit).length === 0) {
    if (at >= 0) staged.splice(at, 1);
    return;
  }
  const next = { id, edit: entry.edit, draft: entry.draft };
  if (at >= 0) staged[at] = next;
    else staged.push(next);
}

function handleNavigate(delta, entry, ui) {
  upsertStaged(editingBook.id, entry);
  sessionUi = ui;
  sessionError = '';
  const next = editIndex + delta;
  if (next >= 0 && next < sessionBooks.length) editIndex = next;
}

async function handleSave(entry) {
  const current = editingBook.id;
  upsertStaged(current, entry);
  const items = staged.map((s) => ({ book_id: s.id, edit: s.edit }));
  if (items.length === 0) {
    closeEditor();
    return;
  }
  try {
    await invoke('save_metadata_batch', { items });
  } catch (e) {
    const message = e?.message ?? String(e);
    const at = e?.book_id != null ? sessionBooks.findIndex((b) => b.id === e.book_id) : -1;
    if (at >= 0 && at !== editIndex) {
      // Nothing was saved. Jump to the book that failed, so the error is shown next to it.
      sessionError = message;
      editIndex = at;
      return;
    }
    throw new Error(message);
  }
  closeEditor();
  sidebarVersion += 1;
  allBooks = null; 
  await refreshBooks(current);
}

// A restore changed the database and perhaps the folders, so everything on screen is read again.
async function handleRestored() {
  restoreOpen = false;
  sidebarVersion += 1;
  allBooks = null;
  await refreshBooks(selectedBook?.id ?? null);
}

function handleSelectBook(book) {
  selectedBook = book;
}

function startDrag(e) {
  dragging = true;
  pendingHeight = null;
  e.target.setPointerCapture(e.pointerId);
  document.body.style.cursor = 'row-resize';
  document.body.style.userSelect = 'none';
  cachedRect = mainColEl.getBoundingClientRect();

  // Preserve the gap between the raw cursor-to-height mapping and the
  // divider's actual current position, so grabbing it doesn't snap it
  // to the cursor.
  const rawFromBottom = cachedRect.bottom - e.clientY;
  dragOffset = gridHeight - rawFromBottom;

  window.addEventListener('pointermove', onDrag);
  window.addEventListener('pointerup', stopDrag);
}

function onDrag(e) {
  if (!dragging || !cachedRect) return;
  const fromBottom = cachedRect.bottom - e.clientY + dragOffset;
  const gridMin = 120;
  const previewMin = 296;
  const max = cachedRect.height - previewMin;
  pendingHeight = Math.min(max, Math.max(gridMin, fromBottom));

  if (!rafPending) {
    rafPending = true;
    requestAnimationFrame(() => {
      gridHeight = pendingHeight;
      rafPending = false;
    });
  }
}

function stopDrag() {
  dragging = false;
  if (pendingHeight !== null) gridHeight = pendingHeight;
  savePreviewHeight();
  cachedRect = null;
  dragOffset = 0;
  document.body.style.cursor = '';
  document.body.style.userSelect = '';
  window.removeEventListener('pointermove', onDrag);
  window.removeEventListener('pointerup', stopDrag);
}

function handleResizerKeydown(e) {
  const step = 20;
  if (e.key === 'ArrowDown') { gridHeight = Math.max(120, gridHeight - step); }
  if (e.key === 'ArrowUp') { gridHeight = Math.min(mainColEl.getBoundingClientRect().height - 296, gridHeight + step); }
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') savePreviewHeight();
}

async function pickLibraryFolder() {
  const selected = await open({ directory: true, multiple: false });
  if (selected) {
    libraryPath = selected;
    await openLibrary();
  }
}

async function openLibrary() {
  try {
    allBooks = null;
    await invoke('set_library_path', { path: libraryPath });
    allBooks = await invoke('get_books_filtered', { filter: { author_id: null, series_id: null, tag_id: null, search: null, custom_column_id: null, custom_value_id: null } });
  } catch (e) {
    console.error('Failed to open library at this path:', e);
    return; // bail before saving, don't persist a path that doesn't actually work
  }

  await invoke('save_last_library_path', { path: libraryPath });
  const switching = libraryLoaded;
  currentFilter = { author_id: null, series_id: null, tag_id: null, search: null, custom_column_id: null, custom_value_id: null };
  searchTerm = '';
  libraryLoaded = true;
  if (switching) sidebarVersion += 1;
  await refreshBooks();
}

</script>

{#if editingBook}
  {#key editingBook.id}
    <EditMetadataModal
      book={editingBook}
      draft={staged.find((s) => s.id === editingBook.id)?.draft ?? null}
      ui={sessionUi}
      startError={sessionError}
      position={{ n: editIndex + 1, total: sessionBooks.length }}
      stagedElsewhere={staged.filter((s) => s.id !== editingBook.id).length}
      onNavigate={handleNavigate}
      onSave={handleSave}
      onClose={closeEditor}
      {quitRequest}
      onQuit={quitApp}
    />
  {/key}
{/if}
{#if restoreOpen}
  <RestoreDialog onClose={() => (restoreOpen = false)} onRestored={handleRestored} />
{/if}
<div class="frame" inert={editingBook != null || restoreOpen}>
  <TopBar bind:searchValue={searchTerm} onSearch={handleSearch} />
  <div class="body-row">
    <Sidebar onFilterChange={handleFilterChange} onReset={handleReset} libraryLoaded={libraryLoaded} refreshKey={sidebarVersion} libraryKey={libraryPath} />
    <div class="main-col" bind:this={mainColEl}>
      <PreviewPanel bind:this={previewPanel} book={selectedBook} libraryLoaded={libraryLoaded} noResults={libraryLoaded && !books.length && !isWholeLibrary(currentFilter)} onEdit={openEditor} />
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div
        class="resizer"
        role="separator"
        aria-orientation="horizontal"
        aria-label="Resize book grid"
        aria-valuenow={gridHeight}
        aria-valuemin={120}
        aria-valuemax={mainColEl ? mainColEl.getBoundingClientRect().height - 296 : 999}
        tabindex="0"
        onpointerdown={startDrag}
        onkeydown={handleResizerKeydown}
      ></div>
      <BookList {books} selectedId={selectedBook?.id} onSelectBook={handleSelectBook} onEdit={openEditor} onRestore={() => { if (libraryLoaded) restoreOpen = true; }} height={gridHeight} />
    </div>
  </div>
  <StatusBar libraryName={libraryPath.split(/[\\/]/).pop()} bookCount={books.length} />
</div>

<style>
.frame {
  width: 100vw; height: 100vh; box-sizing: border-box;
  background: var(--zen-bg);
  display: flex; flex-direction: column; overflow: hidden;
}
.body-row { flex: 1; display: flex; min-height: 0; }
.main-col { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.resizer {
  height: 6px;
  flex: none;
  box-sizing: border-box;
  border-left: 1px solid var(--zen-border);
  cursor: row-resize;
  background: transparent;
}
.resizer:hover {
  background: var(--zen-border);
}
</style>
