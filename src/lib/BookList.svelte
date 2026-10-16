<script>
import { convertFileSrc } from '@tauri-apps/api/core';
import VirtualGrid from './VirtualGrid.svelte';

let { books = [], selectedId = null, onSelectBook, onEdit, onRestore, height = 224 } = $props();

// The card the arrow keys carry on from. It follows the selected book, so tabbing into the grid
// lands on the book shown in the preview.
let active = $state(0);
$effect(() => {
  const at = books.findIndex((b) => b.id === selectedId);
  active = at >= 0 ? at : 0;
});

// The tool buttons above the grid. Only Edit metadata does anything so far.
const tools = [
  { label: 'Add books', path: 'M3 3h11v18H3zM7 3v18M17.5 14v6M14.5 17h6' },
  { label: 'Remove books', path: 'M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6' },
  { label: 'Edit metadata', path: 'M4 20h4L19 9a2.1 2.1 0 00-3-3L5 17zM14.5 7.5l3 3', run: () => onEdit?.() },
  { label: 'Convert books', path: 'M4 8h14M14 4l4 4-4 4M20 16H6M10 12l-4 4 4 4' },
  { label: 'Save to disk', path: 'M5 3h11l4 4v13a1 1 0 01-1 1H5a1 1 0 01-1-1V4a1 1 0 011-1zM8 3v5h7V3M7 21v-7h10v7' },
  { label: 'Connect / share', path: 'M15 5a3 3 0 1 0 6 0a3 3 0 1 0-6 0M3 12a3 3 0 1 0 6 0a3 3 0 1 0-6 0M15 19a3 3 0 1 0 6 0a3 3 0 1 0-6 0M8.6 13.5l6.8 4M15.4 6.5l-6.8 4' },
  { label: 'Restore from backup', path: 'M3 12a9 9 0 1 0 2.6-6.4L3 8M3 3v5h5M12 7v5l3 2', run: () => onRestore?.() }
];

// Grid or table. Only the grid is drawn so far.
let view = $state('grid');
</script>

<div class="strip" style="height: {height}px;">
  <div class="toolbar" role="toolbar" aria-label="Tools">
    {#each tools as tool}
      <div class="tool-wrap">
        <button type="button" class="zen-btn tool" aria-label={tool.label} onclick={tool.run}>
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <path d={tool.path} />
          </svg>
        </button>
        <span class="tip" role="tooltip">{tool.label}</span>
      </div>
    {/each}
    <div class="spacer"></div>
    <div class="view-switch" role="group" aria-label="View">
      <button type="button" class="zen-btn view-btn" class:on={view === 'grid'} aria-pressed={view === 'grid'} aria-label="Grid view" onclick={() => (view = 'grid')}>
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 3h7v7H3zM14 3h7v7h-7zM3 14h7v7H3zM14 14h7v7h-7z" /></svg>
      </button>
      <button type="button" class="zen-btn view-btn" class:on={view === 'table'} aria-pressed={view === 'table'} aria-label="Table view" onclick={() => (view = 'table')}>
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 5h18v14H3zM3 10h18M3 14.5h18M9.5 5v14" /></svg>
      </button>
    </div>
  </div>

  <div class="scroller">
    <VirtualGrid items={books} itemWidth={78} itemHeight={131} gap={16} bind:active onMove={(_at, book) => onSelectBook?.(book)}>
      {#snippet cell(book, index, tabindex)}
        <button type="button" class="zen-btn zen-card card" data-index={index} {tabindex} onclick={() => onSelectBook?.(book)}>
          {#if book.cover_path}
            <img class="cover" src={convertFileSrc(book.cover_path) + '?v=' + book.cover_version} alt={book.title} loading="lazy" />
          {:else}
            <div class="cover fallback"></div>
          {/if}
          <div class="title">{book.title}</div>
        </button>
      {/snippet}
    </VirtualGrid>
  </div>
</div>

<style>
.strip {
  flex: none;
  border-left: 1px solid var(--zen-border);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
/* The tool row stays put and only the grid below it scrolls. */
.toolbar {
  flex: none;
  height: 40px;
  box-sizing: border-box;
  display: flex;
  align-items: center;
  gap: 14px;
  padding: 0 24px;
  border-bottom: 1px solid var(--zen-border);
}
.scroller {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  overflow-x: hidden;
  padding: 14px 24px 16px;
}
.spacer { flex: 1; }
.tool-wrap { position: relative; }
.tool {
  width: 36px; height: 32px; padding: 0;
  display: flex; align-items: center; justify-content: center;
  border-radius: 2px; color: var(--zen-ink);
}
.tool:hover { background: #F7C7CE; }
.tool svg, .tool svg path, .view-btn svg, .view-btn svg path { stroke-width: 2px !important; }
.tip {
  position: absolute; left: 0; top: 40px; z-index: 6;
  display: none; white-space: nowrap; pointer-events: none;
  padding: 4px 8px; border-radius: 2px;
  background: var(--zen-ink); color: #FFFFFF; font-size: 12px;
}
.tool-wrap:hover .tip, .tool-wrap:has(.tool:focus-visible) .tip { display: block; }
.view-switch {
  display: flex; box-sizing: border-box; height: 32px;
  border: 1px solid var(--zen-border); border-radius: 2px;
  overflow: hidden; background: var(--zen-panel);
}
.view-btn {
  width: 36px; height: 30px; padding: 0; border: none; border-radius: 0;
  display: flex; align-items: center; justify-content: center;
  color: var(--zen-text-secondary);
}
.view-btn + .view-btn { border-left: 1px solid var(--zen-border); }
.view-btn.on { background: var(--zen-red); color: #FFFFFF; }
.card { display: flex; flex-direction: column; gap: 6px; border-radius: 2px !important; }
.card:focus { outline: none; background: #F7C7CE; }
.cover { width: 100%; aspect-ratio: 2/3; border-radius: 2px; box-shadow: 0 3px 8px rgba(27,28,30,0.12); object-fit: cover; }
.cover.fallback { 
  background: var(--zen-red-dark);
}
.title {
  font-size: 11px; line-height: 14px; color: var(--zen-text-muted);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  overscroll-behavior: auto; /* a box that only clips must not swallow the mouse wheel */
}
.card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  border-radius: 8px;
  padding: 6px;
  width: 100%;
  height: 100%;
  box-sizing: border-box;
  text-align: left;
  cursor: pointer;
}
</style>
