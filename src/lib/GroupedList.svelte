<script>
  import { untrack } from 'svelte';
  import VirtualList from './VirtualList.svelte';
  import { letterGroups } from './letterGroups.js';

  // A list of filter rows, like the authors or the series. Past `cutoff` rows it is grouped under
  // first letters, the way calibre's tag browser does it, and each group opens and closes.
  let { rows, activeId = null, onToggle, cutoff = 100, itemHeight = 32 } = $props();

  let open = $state(new Set());
  const grouped = $derived(rows.length > cutoff);
  const groups = $derived(grouped ? letterGroups(rows) : []);

  const items = $derived(
    grouped
      ? groups.flatMap((g) => [
          { kind: 'group', label: g.label, count: g.rows.length },
          ...(open.has(g.label) ? g.rows.map((row) => ({ kind: 'row', row })) : [])
        ])
      : rows.map((row) => ({ kind: 'row', row }))
  );

  function toggle(label) {
    const next = new Set(open);
    if (!next.delete(label)) next.add(label);
    open = next;
  }

  // A chosen row must not hide inside a closed group.
  $effect(() => {
    if (!grouped || activeId === null) return;
    const group = groups.find((g) => g.rows.some((r) => r.id === activeId));
    if (group && !untrack(() => open.has(group.label))) open = new Set([...untrack(() => open), group.label]);
  });
</script>

<VirtualList {items} {itemHeight}>
  {#snippet row(item)}
    {#if item.kind === 'group'}
      <button class="zen-btn group-row" onclick={() => toggle(item.label)}>
        <span class="group-label">{item.label}</span>
        <span class="group-meta">
          <span class="filter-count">{item.count}</span>
          <span class="chevron">{open.has(item.label) ? '−' : '+'}</span>
        </span>
      </button>
    {:else}
      <button
        class="zen-btn filter-row"
        class:active={activeId === item.row.id}
        class:nested={grouped}
        onclick={() => onToggle?.(item.row.id)}
      >
        <span class="filter-name">{item.row.name}</span>
        <span class="filter-count">{item.row.count}</span>
      </button>
    {/if}
  {/snippet}
</VirtualList>

<style>
  .filter-row, .group-row {
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
  .filter-row.nested { padding-left: 20px; }
  .filter-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    overscroll-behavior: auto;
  }
  .filter-row:hover { background: #F7C7CE; }
  .filter-row.active {
    background: #F7C7CE;
    color: var(--zen-red);
    font-weight: 500;
  }
  .filter-count { font-size: 11px; color: var(--zen-count); flex: none; }
  .group-row:hover { background: var(--zen-pill-bg); }
  .group-label { font-size: 12px; font-weight: 600; color: var(--zen-ink); }
  .group-meta { display: flex; align-items: center; gap: 10px; }
  .chevron { font-size: 14px; color: var(--zen-text-faint); width: 10px; text-align: center; }
</style>
