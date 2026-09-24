<script>
import '../lib/theme.css';
import { invoke } from '@tauri-apps/api/core';
import TopBar from '../lib/TopBar.svelte';
import Sidebar from '../lib/Sidebar.svelte';
import { onMount } from 'svelte';
import PreviewPanel from '../lib/PreviewPanel.svelte';
import BookList from '../lib/BookList.svelte';
import StatusBar from '../lib/StatusBar.svelte';

let libraryPath = $state('');
let libraryLoaded = $state(false);
let books = $state([]);
let selectedBook = $state(null);
let searchTerm = $state('');
let currentFilter = $state({ author_id: null, series_id: null, tag_id: null, search: null });

let gridHeight = $state(224);
let dragging = false;
let mainColEl = $state();

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

  return () => {
    document.removeEventListener('scroll', handleScroll, true);
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

async function openLibrary() {
  await invoke('set_library_path', { path: libraryPath });
  libraryLoaded = true;
  await refreshBooks();
}

async function refreshBooks() {
  books = await invoke('get_books_filtered', { filter: currentFilter });
  if (!selectedBook && books.length) selectedBook = books[0];
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
}

function handleSelectBook(book) {
  selectedBook = book;
}

function startDrag(e) {
  dragging = true;
  e.target.setPointerCapture(e.pointerId);
  document.body.style.cursor = 'row-resize';
  document.body.style.userSelect = 'none';
  window.addEventListener('pointermove', onDrag);
  window.addEventListener('pointerup', stopDrag);
}
function onDrag(e) {
  if (!dragging || !mainColEl) return;
  const rect = mainColEl.getBoundingClientRect();
  const fromBottom = rect.bottom - e.clientY;
  const gridMin = 120;
  const previewMin = 280;
  const max = rect.height - previewMin;
  gridHeight = Math.min(max, Math.max(gridMin, fromBottom));
}
function stopDrag() {
  dragging = false;
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
</script>

{#if !libraryLoaded}
  <div class="opener">
    <input bind:value={libraryPath} placeholder="/path/to/calibre-library" />
    <button onclick={openLibrary}>Open</button>
  </div>
{:else}
  <div class="frame">
    <TopBar />
    <div class="body-row">
      <Sidebar onFilterChange={handleFilterChange} onReset={handleReset} />
      <div class="main-col" bind:this={mainColEl}>
        <PreviewPanel book={selectedBook} />
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
{/if}

<style>
.opener { display: flex; gap: 8px; padding: 40px; }
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
