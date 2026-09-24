<script>
import { invoke } from '@tauri-apps/api/core';
import { onMount } from 'svelte';

let { onFilterChange, onReset } = $props();

let authors = $state([]);
let series = $state([]);
let tags = $state([]);

let activeAuthor = $state(null);
let activeSeries = $state(null);
let activeTag = $state(null);

onMount(async () => {
  authors = await invoke('get_authors');
  series = await invoke('get_series');
  tags = await invoke('get_tags');
});

function toggleAuthor(id) {
  activeAuthor = activeAuthor === id ? null : id;
  emitFilter();
}
function toggleSeries(id) {
  activeSeries = activeSeries === id ? null : id;
  emitFilter();
}
function toggleTag(id) {
  activeTag = activeTag === id ? null : id;
  emitFilter();
}
function reset() {
  activeAuthor = null;
  activeSeries = null;
  activeTag = null;
  onReset?.();
  emitFilter();
}
function emitFilter() {
  onFilterChange?.({
    author_id: activeAuthor,
    series_id: activeSeries,
    tag_id: activeTag,
    search: null
  });
}

const tools = [
  { label: 'Add books', path: 'M12 5v14M5 12h14' },
  { label: 'Remove books', path: 'M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14' },
  { label: 'Edit metadata', path: 'M4 4h10l6 6v10a1 1 0 01-1 1H4a1 1 0 01-1-1V5a1 1 0 011-1z' },
  { label: 'Convert books', path: 'M17 3l4 4-11 11H6v-4z' },
  { label: 'Save to disk', path: 'M4 19V6a2 2 0 012-2h8l6 6v9a2 2 0 01-2 2H6a2 2 0 01-2-2z' },
  { label: 'Connect / share', path: 'M3 9h18M8 4v5', rect: true }
];
</script>

<div class="sidebar">

  <div>
    <div class="eyebrow">Tools</div>
    <div class="stack">
      {#each tools as tool}
        <button class="zen-btn tool-btn">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--zen-text-secondary)" stroke-width="2">
            {#if tool.rect}<rect x="3" y="4" width="18" height="16" rx="2" />{/if}
            <path d={tool.path} />
          </svg>
          <span>{tool.label}</span>
        </button>
      {/each}
    </div>
  </div>

  <div class="divider"></div>

  <div>
    <div class="filter-head">
      <span class="eyebrow" style="margin:0;">Filters</span>
      <button class="zen-btn reset-link" onclick={reset}>Reset</button>
    </div>
  </div>

  <div>
    <div class="section-title">Authors</div>
    <div class="stack">
      {#each authors as row (row.id)}
        <button
          class="zen-btn filter-row"
          class:active={activeAuthor === row.id}
          onclick={() => toggleAuthor(row.id)}
        >
          <span>{row.name}</span>
          <span class="filter-count">{row.count}</span>
        </button>
      {/each}
    </div>
  </div>

  <div>
    <div class="section-title">Series</div>
    <div class="stack">
      {#each series as row (row.id)}
        <button
          class="zen-btn filter-row"
          class:active={activeSeries === row.id}
          onclick={() => toggleSeries(row.id)}
        >
          <span>{row.name}</span>
          <span class="filter-count">{row.count}</span>
        </button>
      {/each}
    </div>
  </div>

  <div>
    <div class="section-title">Tags</div>
    <div class="tag-row">
      {#each tags as row (row.id)}
        <button
          class="zen-btn tag-pill"
          class:active={activeTag === row.id}
          onclick={() => toggleTag(row.id)}
        >
          {row.name}
        </button>
      {/each}
    </div>
  </div>

</div>

<style>
.sidebar {
  width: 248px;
  box-sizing: border-box;
  flex: none;
  background: var(--zen-bg);
  padding: 20px 16px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 24px;
}
.stack { display: flex; flex-direction: column; gap: 2px; }
.eyebrow {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--zen-text-faint);
  margin-bottom: 8px;
}
.tool-btn {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 7px 8px;
  border-radius: 6px;
  font-size: 13px;
  text-align: left;
  width: 100%;
}
.divider { height: 1px; background: var(--zen-border); margin: -4px 0; }
.filter-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; }
.reset-link { font-size: 12px; color: var(--zen-red); font-weight: 500; }
.section-title { font-size: 12px; font-weight: 600; color: var(--zen-ink); margin-bottom: 6px; }
.filter-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 8px;
  border-radius: 6px;
  font-size: 13px;
  text-align: left;
  width: 100%;
}
.filter-row.active {
  background: var(--zen-pill-bg);
  color: var(--zen-red);
  font-weight: 500;
}
.filter-count { font-size: 11px; color: var(--zen-count); }
.tag-row { display: flex; flex-wrap: wrap; gap: 6px; }
.tag-pill {
  padding: 5px 10px;
  border-radius: 999px;
  font-size: 12px;
  background: var(--zen-panel);
  border: 1px solid var(--zen-border);
  color: var(--zen-text-secondary);
}
.tag-pill.active {
  background: var(--zen-red);
  border-color: var(--zen-red);
  color: white;
}
</style>
