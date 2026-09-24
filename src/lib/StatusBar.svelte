<script>
  let { libraryName = '', bookCount = 0 } = $props();
</script>

<div class="statusbar">
  <span>{libraryName} — {bookCount} books</span>
  <span aria-hidden="true">·</span>
  <span>Sorted by Title</span>
  <div style="flex-grow: 1;"></div>
  <span>Read‑only mode</span>
</div>

<style>
  .statusbar {
    height: 30px; flex: none; display: flex; align-items: center; gap: 18px;
    padding: 0 16px; background: var(--zen-bg); border-top: 1px solid var(--zen-border);
    font-size: 11px; color: var(--zen-text-faint);
  }
</style>
