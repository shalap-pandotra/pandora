<script>
import { invoke, convertFileSrc } from '@tauri-apps/api/core';

let { book, libraryLoaded} = $props();
</script>

<div class="preview">
  {#if book}
    <div class="big-cover">
      {#await invoke('get_cover_path', { bookPath: book.path }) then coverPath}
        {#if coverPath}
          <img class="cover-img" src={convertFileSrc(coverPath)} alt={book.title} />
        {:else}
          <div class="cover-fallback">
            <span>{book.title}</span>
          </div>
        {/if}
      {/await}
    </div>

    <div class="meta">
      <div class="meta-scroll">
        <div>
          <h1>{book.title}</h1>
          {#if book.series}
            <div class="series">{book.series}{book.series_index ? `, Book ${book.series_index}` : ''}</div>
          {/if}
        </div>

        <div class="author">by {book.authors}</div>

        {#if book.tags?.length}
          <div class="pills">
            {#each book.tags as tag}
              <span class="pill">{tag}</span>
            {/each}
          </div>
        {/if}

        {#if book.comments}
          <div class="description">{@html book.comments}</div>
        {/if}
      </div>

      <div class="meta-actions">
        <button class="zen-btn btn-primary">Open book</button>
        <button class="zen-btn btn-secondary">Edit metadata</button>
      </div>
    </div>

  {:else}
  <div class="empty">
    {#if libraryLoaded}
      Select a book to preview it here.
    {:else}
      Hint: Press Ctrl+O to add your existing Calibre Library
    {/if}
  </div>
  {/if}
</div>

<style>
.description {
  font-size: 14px;
  line-height: 1.65;
  color: var(--zen-text-secondary);
  max-width: 640px;
}
.description :global(p) {
  margin: 0 0 12px;
}
.description :global(p:last-child) {
  margin-bottom: 0;
}
.preview {
  flex: 1 1 auto; min-height: 0;
  border: 1px solid var(--zen-border); 
  border-top-left-radius: 4px;
  background: var(--zen-panel);
  display: flex; gap: 32px; padding: 32px; overflow: hidden;
}
.empty {
  margin: auto; color: var(--zen-text-faint); font-size: 14px;
}
.big-cover {
  height: 100%;
  max-width: 420px;
  aspect-ratio: 2 / 3;
  flex: none;
  border-radius: 2px;
  box-shadow: 0 14px 34px rgba(27,28,30,0.20);
  overflow: hidden;
}
.cover-img {
  width: 100%; height: 100%; object-fit: cover; display: block;
}
.cover-fallback {
  width: 100%; height: 100%;
  background: linear-gradient(160deg, #2B2D31, #19191B);
  display: flex; align-items: flex-end; padding: 28px;
  box-sizing: border-box;
}
.cover-fallback span {
  font-family: var(--zen-font-serif); font-size: 26px; font-weight: 500;
  color: #F5F5F6; line-height: 1.25;
}
.meta {
  flex: 1; min-width: 0;
  display: flex; flex-direction: column;
  min-height: 0;
}
.meta-scroll {
  flex: 1; min-height: 0;
  overflow-y: auto;
  display: flex; flex-direction: column; gap: 16px;
}
.meta-actions {
  flex: none;
  display: flex; gap: 10px;
  padding-top: 16px;
}
.meta h1 { font-family: var(--zen-font-serif); font-size: 32px; font-weight: 600; color: var(--zen-ink); line-height: 1.15; margin: 0; }
.series { font-size: 15px; color: var(--zen-text-muted); margin-top: 6px; }
.author { font-size: 15px; color: var(--zen-text-secondary); }
.pills { display: flex; flex-wrap: wrap; gap: 8px; }
.pill { padding: 4px 12px; border-radius: 999px; font-size: 12px; background: var(--zen-pill-bg); color: var(--zen-text-secondary); }
.btn-primary { height: 38px; padding: 0 18px; border-radius: 2px; background: var(--zen-red); color: white; font-size: 13px; font-weight: 500; }
.btn-secondary { height: 38px; padding: 0 18px; border-radius: 2px; background: none; border: 1px solid var(--zen-ink); color: var(--zen-ink); font-size: 13px; font-weight: 500; }
</style>
