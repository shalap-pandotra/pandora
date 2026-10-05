<script>
import { invoke } from '@tauri-apps/api/core';
import { onMount } from 'svelte';
import  VirtualList  from './VirtualList.svelte';

let { onFilterChange, onReset, libraryLoaded, onEditMetadata, refreshKey } = $props();
let authors = $state([]);
let series = $state([]);
let tags = $state([]);
let activeAuthor = $state(null);
let activeSeries = $state(null);
let activeTag = $state(null);
let multiTextColumns = $state([]);
let expandedNodes = $state(new Set());
let activeCustomValue = $state(null); // { columnId, valueId } or null
let authorsExpanded = $state(true);
let seriesExpanded = $state(true);
let tagsExpanded = $state(true);
let collapsedColumns = $state(new Set());

let columnTrees = $derived(
  multiTextColumns.map((col) => ({ ...col,  tree: buildTree(col.values, col.is_hierarchical)}))
);

$effect(() => {
  void refreshKey;
  if (!libraryLoaded) return;
  invoke('get_authors').then((r) => authors = r);
  invoke('get_series').then((r) => {
    series = r;
    if (activeSeries !== null && !r.some((s) => s.id === activeSeries)) {
      activeSeries = null;
      emitFilter();
    }
  });
  invoke('get_tags').then((r) => {
    tags = r;
    if (activeTag !== null && !r.some((t) => t.id === activeTag)) {
      activeTag = null;
      emitFilter();
    }
  });
  invoke('get_multi_text_columns').then((r) => (multiTextColumns = r));
});

function buildTree(values, hierarchical) {
  const root = [];
  const nodeMap = new Map();

  for (const v of values) {
    const parts = hierarchical ? v.value.split('.') : [v.value];
    let currentPath = '';
    let parentChildren = root;

    for (let i = 0; i < parts.length; i++) {
      currentPath = currentPath ? `${currentPath}.${parts[i]}` : parts[i];
      const isLeaf = i === parts.length - 1;
      let node = nodeMap.get(currentPath);

      if (!node) {
        node = { path: currentPath, label: parts[i], children: [], id: null, count: 0 };
        nodeMap.set(currentPath, node);
        parentChildren.push(node);
      }

      if (isLeaf) {
        // Always set id/count on the leaf's own node, whether it already
        // existed as an intermediate stub (created by a descendant seen
        // earlier) or is brand new.
        node.id = v.id;
        node.count = v.count;
      }

      parentChildren = node.children;
    }
  }

  return root;
}

function toggleNode(path) {
  const next = new Set(expandedNodes);
  if (next.has(path)) next.delete(path);
    else next.add(path);
  expandedNodes = next;
}

function selectCustomValue(columnId, valueId) {
  const isSame = activeCustomValue?.columnId === columnId && activeCustomValue?.valueId === valueId;
  activeCustomValue = isSame ? null : { columnId, valueId };
  onFilterChange?.({
    custom_column_id: isSame ? null : columnId,
    custom_value_id: isSame ? null : valueId
  });
}

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
  activeCustomValue = null;
  onReset?.();
  emitFilter();
}

function emitFilter() {
  onFilterChange?.({
    author_id: activeAuthor,
    series_id: activeSeries,
    tag_id: activeTag,
    custom_column_id: activeCustomValue?.columnId ?? null,
    custom_value_id: activeCustomValue?.valueId ?? null
  });
}

function toggleColumnCollapsed(colId) {
  const next = new Set(collapsedColumns);
  if (next.has(colId)) next.delete(colId);
    else next.add(colId);
  collapsedColumns = next;
}

const tools = [
  { label: 'Add books', path: 'M12 5v14M5 12h14' },
  { label: 'Remove books', path: 'M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14' },
  { label: 'Edit metadata', path: 'M4 4h10l6 6v10a1 1 0 01-1 1H4a1 1 0 01-1-1V5a1 1 0 011-1z', run: () => onEditMetadata?.() },
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
        <button class="zen-btn tool-btn" onclick={tool.run}>
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
    <button class="zen-btn section-header" onclick={() => authorsExpanded = !authorsExpanded}>
      <span class="section-title" style="margin:0;">Authors</span>
      <span class="section-chevron">{authorsExpanded ? '−' : '+'}</span>
    </button>
    {#if authorsExpanded}
      <div class="stack">
        <VirtualList items={authors} itemHeight={32}>
          {#snippet row(author)}
            <button
              class="zen-btn filter-row"
              class:active={activeAuthor === author.id}
              onclick={() => toggleAuthor(author.id)}
            >
              <span class="filter-name">{author.name}</span>
              <span class="filter-count">{author.count}</span>
            </button>
          {/snippet}
        </VirtualList>
      </div>
    {/if}
  </div>

  <div>
    <button class="zen-btn section-header" onclick={() => seriesExpanded = !seriesExpanded}>
      <span class="section-title" style="margin:0;">Series</span>
      <span class="section-chevron">{seriesExpanded ? '−' : '+'}</span>
    </button>
    {#if seriesExpanded}
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
    {/if}
  </div>

  <div>
    <button class="zen-btn section-header" onclick={() => tagsExpanded = !tagsExpanded}>
      <span class="section-title" style="margin:0;">Tags</span>
      <span class="section-chevron">{tagsExpanded ? '−' : '+'}</span>
    </button>
    {#if tagsExpanded}
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
    {/if}
  </div>

  {#each columnTrees as col (col.id)}
    <div>
      <button class="zen-btn section-header" onclick={() => toggleColumnCollapsed(col.id)}>
        <span class="section-title" style="margin:0;">{col.name}</span>
        <span class="section-chevron">{collapsedColumns.has(col.id) ? '+' : '−'}</span>
      </button>
      {#if !collapsedColumns.has(col.id)}
        <div class="stack">
          {#each col.tree as node}
            {@render treeNode(node, col.id)}
          {/each}
        </div>
      {/if}
    </div>
  {/each}

  {#snippet treeNode(node, columnId)}
    <div class="tree-row">
      {#if node.children.length}
        <button class="zen-btn tree-toggle" onclick={() => toggleNode(node.path)}>
          {expandedNodes.has(node.path) ? '−' : '+'}
        </button>
      {:else}
        <span class="tree-toggle-spacer"></span>
      {/if}

      {#if node.id !== null}
        <button
          class="zen-btn filter-row"
          class:active={activeCustomValue?.columnId === columnId && activeCustomValue?.valueId === node.id}
          onclick={() => {
            selectCustomValue(columnId, node.id);
            if (node.children.length && !expandedNodes.has(node.path)) {
              toggleNode(node.path);
            }
          }}
        >
          <span>{node.label}</span>
          <span class="filter-count">{node.count}</span>
        </button>
      {:else}
        <button
          class="zen-btn tree-label-only"
          class:active={node.children.length && expandedNodes.has(node.path)}
          onclick={() => node.children.length && toggleNode(node.path)}
        >
          {node.label}
        </button>
      {/if}
    </div>

    {#if node.children.length && expandedNodes.has(node.path)}
      <div class="tree-children">
        {#each node.children as child}
          {@render treeNode(child, columnId)}
        {/each}
      </div>
    {/if}
  {/snippet}

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
  gap: 16px;
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
.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  margin-bottom: 6px;
}
.section-chevron {
  font-size: 13px;
  color: var(--zen-text-faint);
}
.divider {
  height: 1px;
  background: var(--zen-border);
  margin: 0;
  flex-shrink: 0;
}
.filter-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; }
.reset-link { font-size: 12px; color: var(--zen-red); font-weight: 500; }
.section-title { font-size: 12px; font-weight: 600; color: var(--zen-ink); margin-bottom: 6px; }
.filter-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  height: 30px;
  box-sizing: border-box;
  padding: 6px 8px;
  border-radius: 2px;
  font-size: 13px;
  text-align: left;
  width: 100%;
}
.filter-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.filter-row:hover {
  background: #F7C7CE;
}
.filter-row.active {
  background: #F7C7CE;
  color: var(--zen-red);
  font-weight: 500;
}
.filter-count { font-size: 11px; color: var(--zen-count); flex: none; }
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
.tree-row {
  display: flex;
  align-items: center;
  gap: 4px;
}
.tree-toggle {
  width: 18px; height: 18px; flex: none;
  display: flex; align-items: center; justify-content: center;
  font-size: 13px; color: var(--zen-text-faint);
}
.tree-toggle-spacer { width: 18px; flex: none; }
.tree-label-only {
  font-size: 13px; padding: 6px 8px; color: var(--zen-text-faint);
  font-weight: 500;
}
.tree-children {
  margin-left: 9px;
  padding-left: 9px;
}
.tree-children .tree-row {
  position: relative;
}
.tree-children .tree-row::before {
  content: '';
  position: absolute;
  left: -9px;
  top: 0;
  bottom: 50%;
  width: 1px;
  background: var(--zen-text-faint);
}
.tree-children .tree-row::after {
  content: '';
  position: absolute;
  left: -9px;
  top: 50%;
  bottom: 0;
  width: 1px;
  background: var(--zen-text-faint);
}
.tree-children .tree-row:last-child::after {
  display: none;
}
.tree-children .tree-row .tree-toggle,
.tree-children .tree-row .tree-toggle-spacer {
  position: relative;
}
.tree-children .tree-row .tree-toggle::before,
.tree-children .tree-row .tree-toggle-spacer::before {
  content: '';
  position: absolute;
  left: -9px;
  top: 50%;
  width: 9px;
  height: 1px;
  background: var(--zen-text-faint);
}
.tree-label-only {
  font-size: 13px; padding: 6px 8px; color: var(--zen-text-secondary);
  font-weight: 500;
  text-align: left;
  cursor: pointer;
  border-radius: 2px;
  width: 100%;
  display: block;
}
.tree-label-only:hover {
  background: #F7C7CE;
}
.tree-label-only.active {
  background: none;
  color: var(--zen-red);
  font-weight: 500;
}

</style>
