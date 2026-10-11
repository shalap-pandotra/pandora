<script>
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import DOMPurify from 'dompurify';

let { book, libraryLoaded, onEdit } = $props();

let comments = $state(null);
let openError = $state('');

// Also called from the library's V key, so its errors show here too.
export async function openBook() {
  if (!book) return;
  openError = '';
  try {
    await invoke('open_book', { bookId: book.id });
  } catch (e) {
    openError = String(e);
  }
}

$effect(() => {
  openError = '';
  if (!book) {
    comments = null;
    return;
  }
  invoke('get_comments', { bookId: book.id }).then((r) => (comments = r));
});

let sanitizedComments = $derived(comments ? DOMPurify.sanitize(comments) : null);
</script>

<div class="preview">
  {#if book}
    <div class="big-cover">
      {#if book.cover_path}
        <img class="cover-img" src={convertFileSrc(book.cover_path) + '?v=' + book.cover_version} alt={book.title} />
      {:else}
        <div class="cover-fallback">
          <span>{book.title}</span>
        </div>
      {/if}
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

        {#if book.rating}
          {@const stars = Math.floor(book.rating / 2)}
          <div class="stars" role="img" aria-label="Rated {stars} out of 5">
            {#each [1, 2, 3, 4, 5] as n}
              <svg width="16" height="16" viewBox="0 0 24 24" fill={n <= stars ? 'var(--zen-red)' : 'none'} stroke="var(--zen-red)" stroke-width="1.5">
                <path d="M12 2l2.9 6.6 7.1.7-5.4 4.8 1.6 7-6.2-3.7-6.2 3.7 1.6-7-5.4-4.8 7.1-.7z" />
              </svg>
            {/each}
          </div>
        {/if}

        {#if book.tags?.length}
          <div class="pills">
            {#each book.tags as tag}
              <span class="pill">{tag}</span>
            {/each}
          </div>
        {/if}

        {#if sanitizedComments}
          <div class="description">{@html sanitizedComments}</div>
        {/if}     </div>

      <div class="meta-actions">
        <button class="zen-btn btn-primary" onclick={openBook}>Open book</button>
        <button class="zen-btn btn-secondary" onclick={onEdit}>Edit metadata</button>
        {#if openError}<span class="open-error">{openError}</span>{/if}
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
  border-radius: 2px 0 0 2px;
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
  background: var(--zen-red-dark);
  display: flex; align-items: flex-end; padding: 28px;
  box-sizing: border-box;
}
.cover-fallback span {
  font-family: var(--zen-font-serif); font-size: 16px; font-weight: 400;
  color: #F5F5F6; line-height: 1.25;
  transform: scaleY(2.23);
  overflow: hidden;
  transform-origin: left bottom;
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
.open-error { align-self: center; font-size: 13px; color: var(--zen-red); }
.meta-actions {
  flex: none;
  display: flex; gap: 10px;
  padding-top: 16px;
}
.meta h1 { font-family: var(--zen-font-serif); font-size: 32px; font-weight: 400; transform: scaleY(1.15); color: var(--zen-ink); line-height: 1.15; margin: 0; }
.stars { display: flex; gap: 3px; }
.series { font-size: 15px; color: var(--zen-text-muted); margin-top: 6px; }
.author { font-size: 15px; color: var(--zen-text-secondary); }
.pills { display: flex; flex-wrap: wrap; gap: 8px; }
.pill { padding: 4px 12px; border-radius: 999px; font-size: 12px; background: var(--zen-pill-bg); color: var(--zen-text-secondary); }
.btn-primary { height: 38px; padding: 0 18px; border-radius: 2px; background: var(--zen-red); color: white; font-size: 13px; font-weight: 500; }
.btn-secondary { height: 38px; padding: 0 18px; border-radius: 2px; background: none; border: 1px solid var(--zen-ink); color: var(--zen-ink); font-size: 13px; font-weight: 500; }
</style>
