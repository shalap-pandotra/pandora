<script>
import { invoke } from '@tauri-apps/api/core';
import { onMount, untrack } from 'svelte';
import GroupedList from './GroupedList.svelte';
import { letterGroups } from './letterGroups.js';

let { onFilterChange, onReset, libraryLoaded, onEditMetadata, refreshKey, libraryKey } = $props();
let authors = $state([]);
let series = $state([]);
let tags = $state([]);
let activeAuthor = $state(null);
let activeSeries = $state(null);
let activeTag = $state(null);
let multiTextColumns = $state([]);
let expandedNodes = $state(new Set());
let activeCustomValue = $state(null); // { columnId, valueId } or null
// Every filter section starts collapsed.
let authorsExpanded = $state(false);
let seriesExpanded = $state(false);
let tagsExpanded = $state(false);
let expandedColumns = $state(new Set());

let openTagGroups = $state(new Set());
const tagGroups = $derived(tags.length > 100 ? letterGroups(tags) : null);
function toggleTagGroup(label) {
  const next = new Set(openTagGroups);
  if (!next.delete(label)) next.add(label);
  openTagGroups = next;
}
// A chosen tag must not hide inside a closed group.
$effect(() => {
  if (!tagGroups || activeTag === null) return;
  const group = tagGroups.find((g) => g.rows.some((t) => t.id === activeTag));
  if (group && !untrack(() => openTagGroups.has(group.label))) openTagGroups = new Set([...untrack(() => openTagGroups), group.label]);
});

// Pointing the app at another library starts clean: no filter chosen, nothing open.
let seenLibrary;
let libraryKnown = false;
$effect(() => {
  const key = libraryKey;
  if (!libraryKnown) {
    libraryKnown = true;
    seenLibrary = key;
    return;
  }
  if (key === seenLibrary) return;
  seenLibrary = key;
  untrack(() => {
    activeAuthor = null;
    activeSeries = null;
    activeTag = null;
    activeCustomValue = null;
    expandedNodes = new Set();
    expandedColumns = new Set();
    openTagGroups = new Set();
  });
});

let columnTrees = $derived(
  multiTextColumns.map((col) => ({ ...col,  tree: buildTree(col.values, col.is_hierarchical)}))
);

$effect(() => {
  void refreshKey;
  if (!libraryLoaded) return;
  invoke('get_authors').then((r) => {
    authors = r;
    if (activeAuthor !== null && !r.some((a) => a.id === activeAuthor)) {
      activeAuthor = null;
      emitFilter();
    }
  });
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
  invoke('get_multi_text_columns').then((r) => {
    multiTextColumns = r;
    if (activeCustomValue !== null) {
      const col = r.find((c) => c.id === activeCustomValue.columnId);
      if (!col || !col.values.some((v) => v.id === activeCustomValue.valueId)) {
        activeCustomValue = null;
        emitFilter();
      }
    }
  });
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

function toggleColumn(colId) {
  const next = new Set(expandedColumns);
  if (next.has(colId)) next.delete(colId);
    else next.add(colId);
  expandedColumns = next;
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
        {#key libraryKey}
          <GroupedList rows={authors} activeId={activeAuthor} onToggle={toggleAuthor} />
        {/key}
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
        {#key libraryKey}
          <GroupedList rows={series} activeId={activeSeries} onToggle={toggleSeries} />
        {/key}
      </div>
    {/if}
  </div>

  <div>
    <button class="zen-btn section-header" onclick={() => tagsExpanded = !tagsExpanded}>
      <span class="section-title" style="margin:0;">Tags</span>
      <span class="section-chevron">{tagsExpanded ? '−' : '+'}</span>
    </button>
    {#if tagsExpanded}
      {#if tagGroups}
        {#each tagGroups as group (group.label)}
          <button class="zen-btn group-head" onclick={() => toggleTagGroup(group.label)}>
            <span class="group-label">{group.label}</span>
            <span class="group-meta">
              <span class="filter-count">{group.rows.length}</span>
              <span class="group-chevron">{openTagGroups.has(group.label) ? '−' : '+'}</span>
            </span>
          </button>
          {#if openTagGroups.has(group.label)}
            <div class="tag-row group-tags">
              {#each group.rows as row (row.id)}
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
        {/each}
      {:else}
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
    {/if}
  </div>

  {#each columnTrees as col (col.id)}
    <div>
      <button class="zen-btn section-header" onclick={() => toggleColumn(col.id)}>
        <span class="section-title" style="margin:0;">{col.name}</span>
        <span class="section-chevron">{expandedColumns.has(col.id) ? '−' : '+'}</span>
      </button>
      {#if expandedColumns.has(col.id)}
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
.group-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  height: 30px;
  box-sizing: border-box;
  padding: 6px 8px;
  border-radius: 2px;
  width: 100%;
  text-align: left;
}
.group-head:hover { background: var(--zen-pill-bg); }
.group-label { font-size: 12px; font-weight: 600; color: var(--zen-ink); }
.group-meta { display: flex; align-items: center; gap: 10px; }
.group-chevron { font-size: 14px; color: var(--zen-text-faint); width: 10px; text-align: center; }
.group-tags { padding: 4px 8px 10px; }
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
