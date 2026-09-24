<script>
import { invoke, convertFileSrc } from '@tauri-apps/api/core';

let { books = [], value = $bindable(''), onSearch, onSelectBook, height = 224 } = $props();

let debounceHandle;
function handleInput() {
  clearTimeout(debounceHandle);
  debounceHandle = setTimeout(() => onSearch?.(value), 250);
}
</script>

<div class="strip" style="height: {height}px;">
  <div class="search">
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="var(--zen-text-faint)" stroke-width="2">
      <circle cx="11" cy="11" r="7" /><line x1="21" y1="21" x2="16.65" y2="16.65" />
    </svg>
    <label for="lib-search" class="sr-only">Search library</label>
    <input
      id="lib-search"
      bind:value
      oninput={handleInput}
      placeholder="Search title, author, tag…"
    />
  </div>

  <div class="grid">
    {#each books as book (book.id)}
      <button type="button" class="zen-btn zen-card card" onclick={() => onSelectBook?.(book)}>
        {#await invoke('get_cover_path', { bookPath: book.path }) then coverPath}
          {#if coverPath}
            <img class="cover" src={convertFileSrc(coverPath)} alt={book.title} />
          {:else}
            <div class="cover fallback"></div>
          {/if}
        {/await}
        <div class="title">{book.title}</div>
      </button>
    {/each}
  </div>
</div>

<style>
.strip {
  flex: none;
  border-left: 1px solid var(--zen-border);
  overflow-y: auto;
  overflow-x: hidden;
  padding: 16px 24px;
}
.search {
  max-width: 320px;
  display: flex;
  align-items: center;
  background: var(--zen-panel);
  border: 1px solid var(--zen-border);
  border-radius: 2px;
  padding: 0 12px;
  height: 36px;
  margin-bottom: 14px;
}
.search svg { flex: none; margin-right: 8px; }
.search input {
  border: none;
  background: none;
  outline: none;
  font-size: 14px;
  width: 100%;
  color: var(--zen-ink);
}
.sr-only {
  position: absolute; width: 1px; height: 1px;
  overflow: hidden; clip: rect(0 0 0 0);
}
.grid { display: grid; grid-template-columns: repeat(auto-fill, 78px); gap: 16px; }
.card { display: flex; flex-direction: column; gap: 6px; border-radius: 8px; padding: 6px; }
.cover { width: 100%; aspect-ratio: 2/3; border-radius: 2px; box-shadow: 0 3px 8px rgba(27,28,30,0.12); object-fit: cover; }
.cover.fallback { background: var(--zen-border); }
.title {
  font-size: 11px; color: var(--zen-text-muted);
  overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
.card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  border-radius: 8px;
  padding: 6px;
  width: 100%;
  text-align: left;
  cursor: pointer;
}
</style>
