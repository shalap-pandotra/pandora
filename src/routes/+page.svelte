<script>
import '../lib/theme.css';
import { invoke } from '@tauri-apps/api/core';
import TopBar from '../lib/TopBar.svelte';
import Sidebar from '../lib/Sidebar.svelte';
import { onMount } from 'svelte';
import PreviewPanel from '../lib/PreviewPanel.svelte';
import EditMetadataModal from '../lib/EditMetadataModal.svelte';
import BookList from '../lib/BookList.svelte';
import StatusBar from '../lib/StatusBar.svelte';
import { open } from '@tauri-apps/plugin-dialog';

let libraryPath = $state('');
let libraryLoaded = $state(false);
let books = $state([]);
let selectedBook = $state(null);
let editingBook = $state(null);
let sidebarVersion = $state(0);
let searchTerm = $state('');
let currentFilter = $state({ author_id: null, series_id: null, tag_id: null, search: null, custom_column_id: null, custom_value_id: null });

let gridHeight = $state(224);
let dragging = false;
let mainColEl = $state();


let cachedRect = null;
let pendingHeight = null;
let rafPending = false;
let dragOffset = 0;

let requestSeq = 0;

onMount(() => {
  const state = new WeakMap(); // el -> { current, target, rafId, timer }

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

  function handleScroll(e) {
    const el = e.target;
    if (!(el instanceof HTMLElement)) return;

    let s = state.get(el);
    if (!s) {
      s = { current: 0, target: 0, rafId: null, timer: null };
      state.set(el, s);
    }

    s.target = 1;
    if (!s.rafId) s.rafId = requestAnimationFrame(() => tick(el));

    clearTimeout(s.timer);
    s.timer = setTimeout(() => {
      s.target = 0;
      if (!s.rafId) s.rafId = requestAnimationFrame(() => tick(el));
    }, 700);
  }

  document.addEventListener('scroll', handleScroll, true);

  function handleKeydown(e) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'o') {
      e.preventDefault();
      pickLibraryFolder();
    }
  }
  window.addEventListener('keydown', handleKeydown);

  invoke('load_last_library_path').then((savedPath) => {
    if (savedPath) {
      libraryPath = savedPath;
      openLibrary();
    }
  });

  return () => {
    document.removeEventListener('scroll', handleScroll, true);
    window.removeEventListener('keydown', handleKeydown);
  };
});

$effect(() => {
  if (!mainColEl) return;

  const resizeObserver = new ResizeObserver(() => {
    const rect = mainColEl.getBoundingClientRect();
    const gridMin = 120;
    const previewMin = 280;
    const max = rect.height - previewMin;
    if (gridHeight > max) {
      gridHeight = Math.max(gridMin, max);
    }
  });
  resizeObserver.observe(mainColEl);

  return () => resizeObserver.disconnect();
});

async function refreshBooks(keepId = null) {
  const seq = ++requestSeq;
  const result = await invoke('get_books_filtered', { filter: currentFilter });
  if (seq !== requestSeq) return; // a newer request already started, discard this stale one
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
  if (selectedBook) editingBook = selectedBook;
}

async function handleSaved(id) {
  editingBook = null;
  sidebarVersion += 1;
  await refreshBooks(id);
}

function handleSelectBook(book) {
  selectedBook = book;
}

function startDrag(e) {
  dragging = true;
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
  const previewMin = 280;
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
  if (e.key === 'ArrowUp') { gridHeight = Math.min(mainColEl.getBoundingClientRect().height - 280, gridHeight + step); }
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
    await invoke('set_library_path', { path: libraryPath });
    await invoke('get_books_filtered', { filter: { author_id: null, series_id: null, tag_id: null, search: null, custom_column_id: null, custom_value_id: null } });
  } catch (e) {
    console.error('Failed to open library at this path:', e);
    return; // bail before saving, don't persist a path that doesn't actually work
  }

  await invoke('save_last_library_path', { path: libraryPath });
  libraryLoaded = true;
  await refreshBooks();
}

</script>

{#if editingBook}
  <EditMetadataModal book={editingBook} onClose={() => (editingBook = null)} onSaved={handleSaved} />
{/if}
<div class="frame" inert={editingBook != null}>
  <TopBar />
  <div class="body-row">
    <Sidebar onFilterChange={handleFilterChange} onReset={handleReset} libraryLoaded={libraryLoaded} onEditMetadata={openEditor} refreshKey={sidebarVersion} />
    <div class="main-col" bind:this={mainColEl}>
      <PreviewPanel book={selectedBook} libraryLoaded={libraryLoaded} onEdit={openEditor} />
      <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <div
        class="resizer"
        role="separator"
        aria-orientation="horizontal"
        aria-label="Resize book grid"
        aria-valuenow={gridHeight}
        aria-valuemin={120}
        aria-valuemax={mainColEl ? mainColEl.getBoundingClientRect().height - 280 : 999}
        tabindex="0"
        onpointerdown={startDrag}
        onkeydown={handleResizerKeydown}
      ></div>
      <BookList {books} bind:value={searchTerm} onSearch={handleSearch} onSelectBook={handleSelectBook} height={gridHeight} />
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
  cursor: row-resize;
  background: transparent;
}
.resizer:hover {
  background: var(--zen-border);
}
</style>
