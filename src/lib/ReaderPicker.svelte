<script>
import { invoke } from '@tauri-apps/api/core';
import { onMount, tick } from 'svelte';
import { fade, slide } from 'svelte/transition';
import { cubicOut } from 'svelte/easing';

// It opens and closes with the same 70ms glide as the reader's settings panel and the
// edit dialog's side panels: the box grows from its top edge and the dimming fades.
const GLIDE = { duration: 70, easing: cubicOut };

// The reader's Ctrl+O picker. It lists the library's books by title and author only, and
// the search box searches those entries only. Any other file can be opened from disk.
let { error = '', onPick, onBrowse, onClose } = $props();

/** @type {{ id: number, title: string, authors: string }[]} */
let books = $state([]);
let loadError = $state('');
let loading = $state(true);
let query = $state('');
let selected = $state(0);
/** @type {HTMLInputElement | undefined} */
let searchEl = $state();
/** @type {HTMLElement | undefined} */
let listEl = $state();

const SHOWN = 300;

/** Lowercase, without accents, so "emile" finds "Émile". */
const fold = (/** @type {string} */ s) => s.normalize('NFD').replace(/\p{M}/gu, '').toLowerCase();

let folded = $derived(books.map((b) => fold(`${b.title} ${b.authors}`)));
let matches = $derived.by(() => {
  const words = fold(query).split(/\s+/).filter(Boolean);
  if (!words.length) return books;
  return books.filter((_, i) => words.every((w) => folded[i].includes(w)));
});

$effect(() => {
  void query;
  selected = 0;
});

async function keepSelectedInView() {
  await tick();
  listEl?.querySelector('.selected')?.scrollIntoView({ block: 'nearest' });
}

function onKeydown(/** @type {KeyboardEvent} */ e) {
  if (e.key === 'Escape') {
    e.preventDefault();
    onClose();
  } else if (e.key === 'ArrowDown') {
    e.preventDefault();
    selected = Math.min(selected + 1, Math.min(matches.length, SHOWN) - 1);
    keepSelectedInView();
  } else if (e.key === 'ArrowUp') {
    e.preventDefault();
    selected = Math.max(selected - 1, 0);
    keepSelectedInView();
  } else if (e.key === 'Enter') {
    e.preventDefault();
    const book = matches[selected];
    if (book) onPick(book);
  }
  // Nothing typed here reaches the book behind.
  e.stopPropagation();
}

onMount(() => {
  searchEl?.focus();
  invoke('reader_library_books')
    .then((list) => (books = /** @type {any} */ (list)))
    .catch((e) => (loadError = String(e)))
    .finally(() => (loading = false));
});
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="backdrop" role="presentation" transition:fade|global={GLIDE} onclick={(e) => e.target === e.currentTarget && onClose()}>
  <div class="picker" role="dialog" aria-label="Open a book" tabindex="-1" transition:slide|global={GLIDE} onkeydown={onKeydown}>
    <div class="head">
      <input
        class="zen-input search"
        type="text"
        autocomplete="off"
        placeholder="Search the library"
        spellcheck="false"
        bind:this={searchEl}
        bind:value={query}
      />
      <button class="zen-btn browse" aria-label="Browse files" title="Browse files" onclick={onBrowse}>
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z" /></svg>
      </button>
    </div>
    {#if error}<div class="note error">{error}</div>{/if}
    <div class="list" bind:this={listEl}>
      {#if loading}
        <div class="note">Loading the library…</div>
      {:else if loadError}
        <div class="note">{loadError}</div>
      {:else if !matches.length}
        <div class="note">{books.length ? 'No books match.' : 'The library has no EPUB books.'}</div>
      {:else}
        {#each matches.slice(0, SHOWN) as book, i (book.id)}
          <button
            class="row"
            class:selected={i === selected}
            onclick={() => onPick(book)}
            onmousemove={() => (selected = i)}
          >
            <span class="title">{book.title}</span>
            <span class="authors">{book.authors}</span>
          </button>
        {/each}
        {#if matches.length > SHOWN}
          <div class="note">{matches.length - SHOWN} more. Type to narrow the list.</div>
        {/if}
      {/if}
    </div>
  </div>
</div>

<style>
.backdrop {
  position: absolute; inset: 0; z-index: 2;
  display: flex; justify-content: center; align-items: flex-start;
  padding-top: 8vh;
  background: rgba(28, 27, 24, 0.18);
}
.picker {
  width: min(560px, calc(100% - 32px));
  max-height: 70%;
  display: flex; flex-direction: column;
  background: var(--zen-panel);
  border: 1px solid var(--zen-border); border-radius: 2px;
  box-shadow: 0 14px 34px rgba(27, 28, 30, 0.2);
  outline: none;
}
.head { display: flex; gap: 8px; padding: 12px; border-bottom: 1px solid var(--zen-border); }
.search {
  flex: 1; min-width: 0; height: 32px; box-sizing: border-box; padding: 0 10px;
  font-family: var(--zen-font-sans); font-size: 14px; color: var(--zen-ink);
  background: var(--zen-panel); border: 1px solid var(--zen-border); border-radius: 2px;
}
.search:focus { outline: none; border-color: var(--zen-ink); }
.browse {
  flex: none; width: 32px; height: 32px; padding: 0; border-radius: 2px;
  display: flex; align-items: center; justify-content: center;
  border: 1px solid var(--zen-ink); color: var(--zen-ink);
}
.browse svg, .browse svg path { stroke-width: 2px !important; }
.list { overflow-y: auto; min-height: 0; padding: 4px 0; }
.row {
  display: flex; flex-direction: column; align-items: flex-start; gap: 2px;
  width: 100%; padding: 8px 14px; box-sizing: border-box;
  border: none; background: none; text-align: left; cursor: pointer;
  font-family: var(--zen-font-sans);
}
.row.selected { background: var(--zen-pill-bg); }
.title { font-size: 14px; color: var(--zen-ink); }
.authors { font-size: 12px; color: var(--zen-text-muted); }
.note { padding: 10px 14px; font-size: 13px; color: var(--zen-text-faint); }
.note.error { color: var(--zen-red); }
</style>
