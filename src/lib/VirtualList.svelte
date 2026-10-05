<script>
  import { untrack } from 'svelte';

  let { items, itemHeight = 32, buffer = 8, row } = $props();

  let rootEl = $state();
  let start = $state(0);
  let end = $state(40);

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
    const parentRect = scrollParent.getBoundingClientRect();
    const rootRect = rootEl.getBoundingClientRect();
    const offsetTop = rootRect.top - parentRect.top;
    const viewport = scrollParent.clientHeight;

    const first = Math.floor(-offsetTop / itemHeight) - buffer;
    const last = Math.ceil((viewport - offsetTop) / itemHeight) + buffer;

    const nextStart = Math.max(0, first);
    const nextEnd = Math.min(items.length, Math.max(nextStart, last));

    untrack(() => {
      if (nextStart !== start) start = nextStart;
      if (nextEnd !== end) end = nextEnd;
    });
  }

  $effect(() => {
    if (!rootEl) return;
    const scrollParent = findScrollParent(rootEl);
    const onScroll = () => update(scrollParent);

    scrollParent.addEventListener('scroll', onScroll, { passive: true });
    const ro = new ResizeObserver(onScroll);
    ro.observe(scrollParent);
    update(scrollParent);

    return () => {
      scrollParent.removeEventListener('scroll', onScroll);
      ro.disconnect();
    };
  });
</script>

<div bind:this={rootEl} class="vl-root" style="height: {items.length * itemHeight}px;">
  <div class="vl-window" style="transform: translateY({start * itemHeight}px);">
    {#each items.slice(start, end) as item, i (start + i)}
      <div class="vl-row" style="height: {itemHeight}px;">
        {@render row(item)}
      </div>
    {/each}
  </div>
</div>

<style>
  .vl-root { position: relative; }
  .vl-window { position: absolute; top: 0; left: 0; right: 0; }
  .vl-row { display: flex; align-items: center; }
</style>
