# calibre-zen-rs — Roadmap

A Tauri (Rust + Svelte) companion app for calibre libraries. Inspired by
calibre-zen's UI ideas, built independently — no Python, no forking calibre
itself. Reads calibre's `metadata.db` and library folder structure directly.

Stack: **Tauri + Rust (backend) + Svelte (frontend)**. No Python anywhere,
by design.

---

## Phase 1 — Read-only browsing ✅ done

- [x] Rust reads `metadata.db` directly via `rusqlite` (read-only connection)
- [x] Query books, authors, series, tags
- [x] Cover resolution from each book's folder (`cover.jpg`)
- [x] Tauri commands (`set_library_path`, `get_books`, `get_cover_path`)
- [x] Svelte cover grid — title, author, cover rendering
- [x] Toolchain sorted: MSVC (not GNU), `usedev` alias for loading the dev shell

**Status:** working end-to-end against a real calibre library.

---

## Phase 2 — Filter panel & richer browsing

- [ ] Tags/authors/series sidebar, querying the same DB
- [ ] Search and sort
- [ ] Possibly: saved filter states
- Still fully read-only — no writes yet

---

## Phase 3 — Metadata read/write

- [ ] Editing title, author, tags, series, etc.
- [ ] Writing back to `metadata.db` safely — respect calibre's schema/triggers
      so a real library doesn't get corrupted
- [ ] Careful handling of concurrent access if calibre itself might also be
      open
- No Python needed — native Rust the whole way

---

## Phase 4 — Split preview pane, status bar, UI polish

- [ ] Split centre pane: preview above list (calibre-zen-inspired)
- [ ] Status bar: current library, sort, job status
- [ ] Grid/table view switcher, density options
- Mostly frontend work

---

## Phase 5 — EPUB conversion, reimplemented in Rust

- [ ] Start with **one format pair** — EPUB is the natural first target
      (zipped XHTML/CSS, no proprietary binary format to reverse-engineer)
- [ ] Use calibre's source as **reference material only** — not code to port.
      calibre's conversion logic is tightly coupled to its own internal
      architecture (metadata classes, plugin system, config), so this is a
      reimplementation informed by their approach, not a translation
- [ ] No shell-out to calibre's CLI, no embedded Python interpreter — the
      whole point is standing alone without requiring calibre installed
      on the user's system

**Note:** this is the point where the project stops being a "weekend/months"
scope and becomes a genuinely long-term one, format by format.

---

## Phase 6 — Built-in reader (EPUB-first)

- [ ] Render EPUB chapter HTML inside the existing webview (Tauri already
      gives you a full web engine for free here)
- [ ] Pagination — reflowing content into discrete "pages" rather than pure
      scroll (this is the deep part of "reader," not the rendering itself)
- [ ] Respect/override the book's embedded CSS for theming
- [ ] Reading position sync, bookmarks, highlights/annotations

**Estimate:** a solid EPUB-only reader with pagination + themes is realistically
a few months, not years — much friendlier than conversion.

---

## Phase 7 — Expand format coverage

- [ ] MOBI/AZW3 — proprietary binary formats, meaningfully harder to parse
      from scratch than EPUB
- [ ] PDF viewing — more "document viewer" than reflowable reader
- [ ] Wider conversion matrix across formats
- [ ] DRM — separate legal/technical problem entirely; calibre itself
      handles this via plugins, not its core. Explicitly out of scope unless
      revisited later.

**Note:** full parity with calibre's format matrix is genuinely multi-year
territory if pursued completely. Treat this phase as open-ended / ongoing
rather than something to "finish."

---

## Design principles (keep these honest across all phases)

1. **No Python, ever** — not shelled out to, not embedded. This is the
   entire reason this project exists instead of contributing to calibre-zen.
2. **Standalone by the time conversion/reading land** — a user should not
   need calibre installed once phases 5–6 are done. (Phases 1–4 do assume
   an existing calibre library as the data source — that's a format/data
   dependency, not a runtime dependency, and is a separate question from
   whether calibre itself needs to be *installed*.)
3. **Stay compatible with calibre's library format, permanently** — "standalone"
   means not requiring calibre's *program*, not abandoning calibre's *data
   format*. Keep reading/writing the same `metadata.db` + folder structure
   indefinitely, even after conversion/reading are fully native. Most people
   already have a calibre library and won't migrate into a new, unproven
   schema — point-and-open compatibility with an existing library is the
   actual adoption path, not a stepping-stone to be dropped later.
4. **Read-only until Phase 3** — no writes to a real library until metadata
   read/write is deliberately built and tested against schema/trigger
   correctness.
5. **New libraries are calibre-schema from birth** — even with no calibre
   install and no existing library to point at, a library created by this
   app uses calibre's own schema (tables, triggers, `Author/Title (id)/files`
   folder convention), reverse-engineered from calibre's open-source schema
   definitions. One data format, always — not "calibre mode" vs "native
   mode" as separate code paths. A library created here should be openable
   in real calibre too, not just the reverse.
6. **calibre's source is reference material, not code to reuse** — informs
   correct behavior for tricky format edge cases; not something ported
   line-by-line, given how tightly it's coupled to calibre's own internals.
