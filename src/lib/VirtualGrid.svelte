<script>
import { untrack, tick } from 'svelte';

// A grid of same size cells that draws only the rows near the screen. It fits as many cells to a
// row as the width allows, like a grid with auto-fill columns. It is one stop in the tab order,
// and the arrow keys move between cells. Each cell is drawn with cell(item, index, tabindex) and
// must carry data-index={index} on the element that takes the focus.
let { items, itemWidth, itemHeight, gap = 0, buffer = 3, cell, active = $bindable(0), onMove } = $props();

let rootEl = $state();
let columns = $state(1);
let firstRow = $state(0);
let lastRow = $state(8);

const rowHeight = $derived(itemHeight + gap);
const rowCount = $derived(Math.ceil(items.length / columns));

// The cell that holds the tab stop. If it is scrolled out of the drawn rows, the first drawn cell
// takes the tab stop instead, so the grid can always be tabbed into.
const firstIndex = $derived(firstRow * columns);
const lastIndex = $derived(Math.min(items.length, lastRow * columns));
const current = $derived(Math.max(0, Math.min(active, items.length - 1)));
const tabStop = $derived(current >= firstIndex && current < lastIndex ? current : firstIndex);

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

/** Scrolls just far enough for a cell's row to be fully in view. */
function scrollToIndex(index) {
  const parent = findScrollParent(rootEl);
  const row = Math.floor(index / columns);
  const top = rootEl.getBoundingClientRect().top - parent.getBoundingClientRect().top + row * rowHeight;
  const bottom = top + itemHeight;
  const margin = 8;
  if (top < margin) parent.scrollTop += top - margin;
    else if (bottom > parent.clientHeight - margin) parent.scrollTop += bottom - (parent.clientHeight - margin);
  update(parent);
}

async function onKeydown(e) {
  if (e.ctrlKey || e.altKey || e.metaKey || e.shiftKey || !items.length) return;
  const step = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -columns, ArrowDown: columns }[e.key];
  if (step === undefined) return;
  e.preventDefault();
  const at = e.target instanceof Element ? e.target.closest('[data-index]') : null;
  const from = at ? Number(at.dataset.index) : current;
  const next = Math.max(0, Math.min(items.length - 1, from + step));
  active = next;
  if (next !== from) onMove?.(next, items[next]);
  scrollToIndex(next);
  await tick();
  rootEl.querySelector(`[data-index="${next}"]`)?.focus({ preventScroll: true });
}

// Whatever takes the focus, by a click or by Tab, is where the arrow keys carry on from.
function onFocusIn(e) {
  const at = e.target instanceof Element ? e.target.closest('[data-index]') : null;
  if (at) active = Number(at.dataset.index);
}

// The list changed, from a search or a filter, so the rows on screen may have changed too.
$effect(() => {
  const count = items.length;
  if (!rootEl || count < 0) return;
  untrack(() => update(findScrollParent(rootEl)));
});
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div bind:this={rootEl} class="vg-root" role="group" aria-label="Books" onkeydown={onKeydown} onfocusin={onFocusIn} style="height: {Math.max(0, rowCount * rowHeight - gap)}px;">
  <div
    class="vg-window"
    style="transform: translateY({firstRow * rowHeight}px); grid-template-columns: repeat({columns}, {itemWidth}px); grid-auto-rows: {itemHeight}px; gap: {gap}px;"
  >
    {#each items.slice(firstRow * columns, lastRow * columns) as item, i (firstRow * columns + i)}
      {@const index = firstRow * columns + i}
      {@render cell(item, index, index === tabStop ? 0 : -1)}
    {/each}
  </div>
</div>

<style>
.vg-root { position: relative; }
.vg-window { position: absolute; top: 0; left: 0; right: 0; display: grid; }
</style>
