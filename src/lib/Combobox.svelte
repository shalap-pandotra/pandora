<script module>
let uid = 0;
</script>

<script>
import { tick } from 'svelte';

// A text input with a list of suggestions under it: the same dropdown the custom metadata fields
// use, so it never falls back on the browser's own list of saved entries. Give it `options` (plain
// strings), or a `source(fragment, text)` that returns strings or { text, hint, fill } objects.
// With a `separator` the field holds several values, and the list works on the one being typed.
let {
  value = $bindable(''),
  options = [],
  source = null,
  separator = null,
  minLength = 0,
  bare = false,
  class: className = '',
  onpick,
  onenter,
  onkeydown,
  onfocus,
  onblur,
  oninput,
  ...rest
} = $props();

const listId = `cb-list-${++uid}`;
let open = $state(false);
let hi = $state(-1);

const joiner = $derived(separator === '&' ? ' & ' : ', ');
const norm = (/** @type {any} */ s) => (typeof s === 'string' ? { text: s } : s);

const items = $derived.by(() => {
  const fragment = (separator ? value.split(separator).pop() ?? '' : value).trim();
  if (fragment.length < minLength) return [];
  if (source) return source(fragment, value).slice(0, 50).map(norm);
  const lower = fragment.toLowerCase();
  // Values already in the field are not offered again.
  const chosen = separator ? value.split(separator).map((p) => p.trim().toLowerCase()).filter(Boolean) : [];
  return options
    .filter((o) => !chosen.includes(o.toLowerCase()) && o.toLowerCase().includes(lower))
    .slice(0, 50)
    .map(norm);
});

function pick(/** @type {any} */ item) {
  const fill = item.fill ?? item.text;
  if (separator) {
    const head = value.split(separator).slice(0, -1).map((p) => p.trim()).filter((p) => p !== '');
    value = [...head, fill].join(joiner) + joiner;
  } else {
    value = fill;
    open = false;
  }
  hi = -1;
  onpick?.(item);
}

async function move(/** @type {number} */ delta) {
  if (!items.length) return;
  open = true;
  hi = Math.max(-1, Math.min(Math.min(hi, items.length - 1) + delta, items.length - 1));
  if (hi < 0) return;
  await tick();
  document.getElementById(`${listId}-${hi}`)?.scrollIntoView({ block: 'nearest' });
}

function keydown(/** @type {KeyboardEvent} */ e) {
  if (e.key === 'ArrowDown') {
    e.preventDefault();
    if (!open) {
      open = true;
      hi = 0;
    } else move(1);
  } else if (e.key === 'ArrowUp') {
    e.preventDefault();
    move(-1);
  } else if (e.key === 'Enter') {
    // Only picks something you actively highlighted, so a typed value is never replaced.
    if (open && hi >= 0 && items[hi]) {
      e.preventDefault();
      pick(items[hi]);
    } else {
      open = false;
      onenter?.(e);
    }
  } else if (e.key === 'Escape' && open && items.length) {
    // Closes the list first. Whatever is behind it only sees a second Escape. With no list
    // showing there is nothing to close, so the first Escape goes straight through.
    e.stopPropagation();
    open = false;
  }
  if (!e.defaultPrevented) onkeydown?.(e);
}
</script>

<div class="cb-box {className}">
  <input
    {...rest}
    class="cb-input"
    class:bare
    role="combobox"
    aria-expanded={open && items.length > 0}
    aria-controls={listId}
    aria-autocomplete="list"
    aria-activedescendant={open && hi >= 0 ? `${listId}-${hi}` : undefined}
    autocomplete="off"
    bind:value
    onfocus={(e) => {
      open = true;
      hi = -1;
      onfocus?.(e);
    }}
    oninput={(e) => {
      open = true;
      hi = -1;
      oninput?.(e);
    }}
    onblur={(e) => {
      open = false;
      onblur?.(e);
    }}
    onkeydown={keydown}
  />
  {#if open && items.length}
    <div class="cb-list" id={listId} role="listbox" tabindex="-1" onmousedown={(e) => e.preventDefault()}>
      {#each items as item, i}
        <div
          id="{listId}-{i}"
          class="cb-opt"
          class:hi={i === hi}
          role="option"
          aria-selected={i === hi}
          tabindex="-1"
          onmousedown={(e) => {
            e.preventDefault();
            pick(item);
          }}
          onmouseenter={() => (hi = i)}
        >
          <span class="cb-text">{item.text}</span>
          {#if item.hint}<span class="cb-hint">{item.hint}</span>{/if}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
.cb-box { position: relative; min-width: 0; }
.cb-input {
  font: inherit;
  font-size: 13px;
  padding: 8px 10px;
  border: 1px solid var(--zen-border);
  border-radius: 2px;
  background: var(--zen-panel);
  color: var(--zen-ink);
  box-sizing: border-box;
  width: 100%;
}
.cb-input:disabled { background: var(--zen-bg); color: var(--zen-text-faint); }
/* Inside a box that draws its own border, such as the search bar. */
.cb-input.bare { border: none; background: none; outline: none; padding: 0; user-select: text; }
.cb-list {
  position: absolute;
  top: calc(100% + 2px);
  left: 0;
  right: 0;
  max-height: 200px;
  overflow-y: auto;
  background: var(--zen-panel);
  border: 1px solid var(--zen-border);
  border-radius: 2px;
  box-shadow: 0 6px 16px rgba(28, 27, 24, 0.12);
  z-index: 5;
}
.cb-opt { display: flex; justify-content: space-between; gap: 12px; padding: 6px 10px; font-size: 13px; color: var(--zen-text-secondary); cursor: pointer; }
.cb-opt.hi { background: #F7C7CE; color: var(--zen-red); }
.cb-text { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cb-hint { flex: none; font-size: 11px; color: var(--zen-text-faint); }
.cb-opt.hi .cb-hint { color: var(--zen-red); }
</style>
