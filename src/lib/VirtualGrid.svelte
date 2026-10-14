<script>
  import { untrack } from 'svelte';

  // A grid of same size cells that draws only the rows near the screen. It fits as many cells to a
  // row as the width allows, like a grid with auto-fill columns.
  let { items, itemWidth, itemHeight, gap = 0, buffer = 3, cell } = $props();

  let rootEl = $state();
  let columns = $state(1);
  let firstRow = $state(0);
  let lastRow = $state(8);

  const rowHeight = $derived(itemHeight + gap);
  const rowCount = $derived(Math.ceil(items.length / columns));

  function findScrollParent(el) {
    let node = el.parentElement;
    while (node) {
      const oy = getComputedStyle(node).overflowY;
      if (oy === 'auto' || oy === 'scroll') return node;
      node = node.parentElement;
    }
    return document.scrollingElement;
  }

  function update(scrollParent) {
    const nextColumns = Math.max(1, Math.floor((rootEl.clientWidth + gap) / (itemWidth + gap)));
    const rows = Math.ceil(items.length / nextColumns);
    const parentRect = scrollParent.getBoundingClientRect();
    const rootRect = rootEl.getBoundingClientRect();
    const offsetTop = rootRect.top - parentRect.top;
    const viewport = scrollParent.clientHeight;

    const first = Math.floor(-offsetTop / (itemHeight + gap)) - buffer;
    const last = Math.ceil((viewport - offsetTop) / (itemHeight + gap)) + buffer;

    const nextFirst = Math.max(0, Math.min(first, rows));
    const nextLast = Math.min(rows, Math.max(nextFirst, last));

    untrack(() => {
      if (nextColumns !== columns) columns = nextColumns;
      if (nextFirst !== firstRow) firstRow = nextFirst;
      if (nextLast !== lastRow) lastRow = nextLast;
    });
  }

  $effect(() => {
    if (!rootEl) return;
    const scrollParent = findScrollParent(rootEl);
    const onScroll = () => update(scrollParent);

    scrollParent.addEventListener('scroll', onScroll, { passive: true });
    const ro = new ResizeObserver(onScroll);
    ro.observe(scrollParent);
    ro.observe(rootEl);
    update(scrollParent);

    return () => {
      scrollParent.removeEventListener('scroll', onScroll);
      ro.disconnect();
    };
  });

  // The list changed, from a search or a filter, so the rows on screen may have changed too.
  $effect(() => {
    const count = items.length;
    if (!rootEl || count < 0) return;
    untrack(() => update(findScrollParent(rootEl)));
  });
</script>

<div bind:this={rootEl} class="vg-root" style="height: {Math.max(0, rowCount * rowHeight - gap)}px;">
  <div
    class="vg-window"
    style="transform: translateY({firstRow * rowHeight}px); grid-template-columns: repeat({columns}, {itemWidth}px); grid-auto-rows: {itemHeight}px; gap: {gap}px;"
  >
    {#each items.slice(firstRow * columns, lastRow * columns) as item, i (firstRow * columns + i)}
      {@render cell(item)}
    {/each}
  </div>
</div>

<style>
  .vg-root { position: relative; }
  .vg-window { position: absolute; top: 0; left: 0; right: 0; display: grid; }
</style>
