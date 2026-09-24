# A Roadmap for Pandora

A Tauri (Rust + Svelte) companion app for calibre libraries. Inspired by
calibre-zen's UI ideas, built independently. Reads calibre's metadata.db and
library folder structure directly.

Stack: Tauri + Rust (backend) + Svelte (frontend).

---

## Phase 1: Read-only browsing (done)

- [x] Rust reads metadata.db directly via rusqlite (read-only connection)
- [x] Query books, authors, series, tags
- [x] Cover resolution from each book's folder (cover.jpg)
- [x] Tauri commands (set_library_path, get_books, get_cover_path)
- [x] Svelte cover grid: title, author, cover rendering
- [x] Toolchain sorted: MSVC (not GNU), usedev alias for loading the dev shell

Status: working end to end against a real calibre library.

---

## Phase 2: Filter panel, richer browsing, and UI polish (done)

- [x] Tags, authors, series sidebar, querying the same DB
- [x] Search across titles, authors, and tags
- [x] Reset clears both sidebar filters and the search box
- [x] Split preview pane (cover + full metadata) above a resizable book grid
- [x] Comments field rendered as the book description
- [x] Draggable resizer between the preview panel and the book grid, with
      sane min heights on both sides, mouse and keyboard support
- [x] Real native window chrome: no OS titlebar, custom minimize, maximize,
      close buttons wired to the actual window, whole bar draggable, native
      double click to maximize
- [x] Thin auto-hiding scrollbars, styled to match the app
- [x] Full visual identity locked: warm neutral palette, one red accent,
      serif display type over a plain sans body, own app icon
- [x] Renamed from the calibre-zen-inspired working title to Pandora
- [x] Pushed to a private GitHub repo

Open gap: custom metadata columns (built by the user in calibre, not the
built in tags field) are not read yet. This matters specifically for genre,
since calibre users commonly build genre and subgenre hierarchies as a
custom column using dot separated values (for example Fiction.Sci-Fi.Space
Opera). This needs its own schema investigation (custom_columns table plus
a per column custom_column_N table and link table) before it can be queried
and shown as a real tag tree in the sidebar. Not done. Picking this up is
the actual next step before phase 2 is fully closed out.

---

## Phase 3: Metadata read and write

- [ ] Editing title, author, tags, series, and so on
- [ ] Writing back to metadata.db safely, respecting calibre's schema and
      triggers so a real library does not get corrupted
- [ ] Careful handling of concurrent access if calibre itself might also be
      open at the same time
- No Python needed, native Rust the whole way

---

## Phase 4: Remaining UI polish

Most of this phase's original scope (split pane, status bar, resizing)
already landed in phase 2. What's left:

- [ ] Grid and table view switcher, density options
- [ ] Selected book state visibly highlighted in the compact grid
- [ ] Active filter states in the sidebar (already partly done for
      author, series, and tag rows)
- [ ] Empty states: no library loaded, zero search results, empty filter
- [ ] Long description handling if it ever overflows badly at minimum
      preview height (currently scrolls in its own region, seems fine, but
      worth a real stress test with a very long description)

---

## Phase 5: EPUB conversion, reimplemented in Rust

- [ ] Start with one format pair. EPUB is the natural first target since
      it is zipped XHTML and CSS, with no proprietary binary format to
      reverse engineer
- [ ] Use calibre's source as reference material only, not code to port.
      calibre's conversion logic is tightly coupled to its own internal
      architecture (metadata classes, plugin system, config), so this is a
      reimplementation informed by their approach, not a translation
- [ ] No shelling out to calibre's CLI, no embedded Python interpreter. The
      whole point is standing alone without requiring calibre installed on
      the user's system

Note: this is the point where the project stops being a weekend or months
scope and becomes a genuinely long term one, format by format.

---

## Phase 6: Built in reader (EPUB first)

- [ ] Render EPUB chapter HTML inside the existing webview (Tauri already
      gives a full web engine for free here)
- [ ] Pagination: reflowing content into discrete pages rather than pure
      scroll. This is the deep part of "reader," not the rendering itself
- [ ] Respect or override the book's embedded CSS for theming
- [ ] Reading position sync, bookmarks, highlights, and annotations

Estimate: a solid EPUB only reader with pagination and themes is realistically
a few months, not years. Much friendlier than conversion.

---

## Phase 7: Expand format coverage

- [ ] MOBI and AZW3, proprietary binary formats, meaningfully harder to
      parse from scratch than EPUB
- [ ] PDF viewing, more of a document viewer than a reflowable reader
- [ ] Wider conversion matrix across formats
- [ ] DRM: a separate legal and technical problem entirely. calibre itself
      handles this via plugins, not its core. Explicitly out of scope
      unless revisited later

Note: full parity with calibre's format matrix is genuinely multi year
territory if pursued completely. Treat this phase as open ended and ongoing
rather than something to finish.

---

## Design principles (keep these honest across all phases)

1. No Python, ever. Not shelled out to, not embedded. This is the entire
   reason this project exists instead of contributing to calibre-zen.
2. Standalone by the time conversion and reading land. A user should not
   need calibre installed once phases 5 and 6 are done. (Phases 1 through 4
   do assume an existing calibre library as the data source. That is a
   format and data dependency, not a runtime dependency, and is a separate
   question from whether calibre itself needs to be installed.)
3. Stay compatible with calibre's library format, permanently. Standalone
   means not requiring calibre's program, not abandoning calibre's data
   format. Keep reading and writing the same metadata.db plus folder
   structure indefinitely, even after conversion and reading are fully
   native. Most people already have a calibre library and won't migrate
   into a new, unproven schema. Point and open compatibility with an
   existing library is the actual adoption path, not a stepping stone to
   be dropped later.
4. Read-only until phase 3. No writes to a real library until metadata
   read and write is deliberately built and tested against schema and
   trigger correctness.
5. New libraries are calibre schema from birth. Even with no calibre
   install and no existing library to point at, a library created by this
   app uses calibre's own schema (tables, triggers, the Author/Title (id)
   folder convention), reverse engineered from calibre's open source schema
   definitions. One data format, always, not a "calibre mode" versus
   "native mode" split into separate code paths. A library created here
   should be openable in real calibre too, not just the reverse.
6. calibre's source is reference material, not code to reuse. It informs
   correct behavior for tricky format edge cases. It is not something
   ported line by line, given how tightly it is coupled to calibre's own
   internals.
