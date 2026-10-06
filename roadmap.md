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
- [x] Library path remembered across restarts, Ctrl+O opens a native
      folder picker, and the whole app shell shows an empty state before a
      library is open
- [x] Collapsible Authors, Series, Tags and custom column sections
- [x] Authors list virtualized: only the visible rows are rendered, tracked
      against the sidebar's own scroll position, so the sidebar still
      scrolls as one piece
- [x] Review fixes: one read connection held for the whole session, tags
      folded into the book query, cover paths computed in Rust, comments
      fetched only for the selected book and sanitized before display,
      books sorted by calibre's sort field, "|" shown as "," in author
      names, and dot hierarchy applied only to columns opted in through
      calibre's categories_using_hierarchy preference
- Deliberately skipped: tree parent nodes show no aggregated count. An
  accurate count needs distinct book ids per node, and clicking a parent
  would then have to filter its whole subtree

Custom metadata columns (built by the user in calibre, not the built in
tags field): the first slice is done, covering multi value text columns.
This turned out to be a genuinely larger surface than first assumed, so
the research is documented below, along with what shipped and what is
deferred.

### Custom columns research

calibre's own documented custom column datatypes: bool, comments,
composite, datetime, enumeration, float, int, rating, series, text. Nine
distinct types, several structurally very different from each other, not
variations on one theme. Mapped directly against calibre's own
"Create a custom column" dialog options:

| Dialog option                                   | datatype    | is_multiple |
|--------------------------------------------------|-------------|--------------|
| Text, shown in Tag browser                        | text        | 0            |
| Comma separated text, like tags, shown in Tag browser | text    | 1            |
| Long text, like comments, not shown in Tag browser | comments   | 0            |
| Text column for keeping series-like information    | series     | 0            |
| Text, but with a fixed set of permitted values     | enumeration | 0           |
| Date                                              | datetime    | 0            |
| Floating point numbers                            | float       | 0            |
| Integers                                          | int         | 0            |
| Ratings, shown with stars                         | rating      | 0            |
| Yes/No                                            | bool        | 0            |
| Column built from other columns                   | composite   | 0            |
| Column built from other columns, behaves like tags | composite  | 1            |

A few of these are notably not simple key value pairs:

- composite is not stored data at all. It is a live template formula
  (composite_template, composite_sort in calibre's own field metadata)
  evaluated on the fly by calibre's template engine at read time. There is
  no fixed table of values to query for this type, supporting it properly
  would mean interpreting or reimplementing meaningful parts of calibre's
  own template language.
- series type custom columns automatically get a companion #label_index
  field, mirroring how the built in series and series_index pair works.
  Not one column, a linked pair.
- enumeration has a fixed, pre declared list of legal values (enum_values)
  plus optional per value colors (enum_colors), a genuinely different UI
  than a free text tag list.
- Every custom column also carries a display JSON blob with per type
  configuration: is_names and use_decorations for text columns, date_format
  for datetime, number_format for int and float, and so on. Behavior can
  differ even within one datatype depending on this.

Important negative finding: there is no explicit "hierarchical" flag
anywhere in calibre's schema or documentation. The dot separated genre
convention (Fiction.Sci-Fi.Space Opera) is a pure user and tag browser
display convention layered on top of a plain is_multiple text column. It
is not a distinct stored column type, calibre's tag browser simply
interprets literal periods in the stored string as tree separators at
display time. This means the same tree building logic applies to any
is_multiple text column, not something specific to a column happening to
be labeled genre.

### First slice, scoped deliberately narrow

Given the real size of the full surface above, the first implementation
targets one specific shape only: any custom column that is datatype text
with is_multiple set to 1 (the "comma separated text, like tags" dialog
option). Discovered dynamically by querying custom_columns at runtime,
not hardcoded to a column labeled genre specifically, or to any fixed
label at all. Whatever such columns exist in a given library, however many,
whatever they are labeled, each becomes its own filterable section in the
sidebar using calibre's own stored name for it.

This covers the genre and subgenre use case, since that is exactly this
shape, but also generalizes for free to anyone else's differently purposed
multi value text column (mood, setting, whatever a user invented it for),
since the query and tree building logic only depends on the shape
(text, is_multiple = 1), never the label or its intended meaning.

The dot separated hierarchy display applies here too, generically, since
it was confirmed above to be a display convention on the stored string
value, not a genre specific feature.

Explicitly deferred to a later slice, tracked here so it is not forgotten:

- [ ] Single value text custom columns (is_multiple = 0)
- [ ] comments type (long text, not shown in Tag browser)
- [ ] series type custom columns (and their paired _index field)
- [ ] enumeration type (fixed permitted value lists, plus colors)
- [ ] datetime, float, int, bool, rating type custom columns
- [ ] composite type (would require interpreting or reimplementing part
      of calibre's own template language, a project of its own)

Next concrete steps for the first slice:

- [x] Rust: dynamically discover all custom_columns rows where
      datatype = 'text' and is_multiple = 1, rather than looking up a
      single hardcoded label
- [x] Rust: for each discovered column, query its custom_column_N and
      books_custom_column_N_link tables for values and per book book counts
- [x] Svelte: build the dot separated tree client side from the raw flat
      values, rather than doing it in Rust
- [x] Svelte: render one collapsible section per discovered column in the
      sidebar, with expandable subgenre style tree nodes
- [x] Tested against a real library with a genuine multi level hierarchy
      (Classics > Literary Fiction), not just a flat single level list

First slice status: done. What actually shipped, beyond the plan above:

- Empty branch pruning: an implied intermediate node (for example
  "Classics," which exists only as a substring of the stored value
  "Classics.Literary Fiction," never as its own row) is dropped entirely
  unless it or some descendant, however deep, actually has entries. A
  dead end branch with nothing real under it does not render at all.
- A surviving node with no entries of its own but with real descendants
  renders as a plain, non filtering label, purely a waypoint down to
  something real, clickable only to expand or collapse its subtree.
- A leaf (or mid tree node) that does have its own entries renders as a
  real filter button. If it also has children, clicking it does both:
  applies the filter and expands the subtree, in one click.
- Selecting a value actually narrows the book grid (required fixing a
  real bug: the custom column's JOIN clause was originally being
  appended to the SQL string after WHERE and ORDER BY, producing a
  syntax error. JOIN clauses must be built before conditions are
  joined into the WHERE clause, and param binding order must match
  the same relative position as its corresponding condition).
- Reset clears every filter type together now: author, series, tag,
  search, and custom column value, not just the original three.
- Selecting a book, or having the grid narrow to a new filtered set,
  auto selects the first result and the preview panel updates to match,
  rather than leaving a stale selection from before the filter changed.

This generalizes to any library with one or more columns of this shape,
however many, whatever they are labeled, not just a column happening to
be named genre.

---

## Phase 3: Metadata read and write

Status: in progress. Every item marked done was tested on a copy of a real
library and then checked by opening that copy in calibre.

- [x] Safety layer: a separate read write connection used only for saving,
      with calibre's title_sort registered on it, a backup of metadata.db
      before the first write of a session, and one transaction per save
- [x] Edit metadata dialog, opened from the preview panel and from the
      sidebar, which sends only the fields that changed
- [x] Description, rating, tags, series and series number
- [x] Title and authors, including renaming the book's folder and files
- [ ] Publisher, languages, identifiers, published date, and cover
- [ ] Editing custom columns, starting with multi value text columns such
      as genre
- [ ] Add books and remove books (adding needs calibre's uuid4 function)
- [ ] Restore from a backup, using the rename log to undo folder moves
- [ ] OPF generation, see the notes below
- [ ] Careful handling of concurrent access if calibre itself might also be
      open at the same time. For now the rule is to close calibre first
- No Python needed, native Rust the whole way

### What writing taught us (checked against calibre on a real library)

Triggers. calibre's triggers call only two functions that are not stored
in the database: title_sort (title updates and both series triggers) and
uuid4 (inserting a book). Pandora registers title_sort on its write
connection and will need uuid4 when adding books is built. No trigger
touches authors, tags, publishers, ratings or comments beyond foreign key
checks.

Rating. Stored as stars times two (0 to 10), with one shared row per value
in the ratings table. No link row at all means unrated. The built in
rating column has no half stars (only custom rating columns can), and
Pandora shows an odd stored value as whole stars, rounded down.

Tags, series and authors follow one rule. A name is matched ignoring case,
and using a different capitalization renames the shared row for every book
that uses it. This was verified in calibre for all three. Rows that no book
uses anymore are deleted.

Series number. Stored on the book as a number that is never NULL, with a
default of 1.0. Clearing a series writes 1.0 (every seriesless book in the
test library was exactly 1.0). Choosing an existing series pre fills the
next whole number after the highest in that series.

Authors. Only & separates authors, and a comma inside a name is stored as
|. An existing author keeps its stored sort (it may be hand set), and only a
brand new author gets a derived one. When an author is renamed, the author's
own sort is left alone and each book's author_sort is recomputed from the
new name. As a result a book's author_sort can differ in capitalization
alone from its authors' sorts joined together, which is harmless. The case
of a renamed co-author next to one that was not renamed is a guess, not
verified.

Folders and files. A book lives at {first author}/{title} ({id}), using
only the first author, so changing a second author moves nothing. Every
format file shares one base name, {title} - {first author}. The characters
? " : / < > | * + and \ become _, ".." becomes _, a trailing dot becomes _,
and non ASCII text is transliterated to ASCII in paths only (names keep
their Unicode). Names are cut to 36 characters for folder parts and 31 for
file name parts on Windows (derived from a limit of 40 that shrinks for long
library paths, believed to be 100 elsewhere, unverified). All of this
matched calibre exactly across the test library, including accents, Hindi
and awkward characters. Calibre reads books.path from the database and does
not recompute it, so a name that differs from calibre's for some script
(Chinese and Japanese are untested) would only look different, it could not
make a book unfindable.

Order of operations on a save. The database work is one transaction. Files
and folders are renamed first, and if anything fails, including the commit,
every move is undone. A move never overwrites anything, and a rename that
only changes capitalization goes through a temporary name so it works on
Windows. Author folders left empty are removed, as calibre does.

Backups. calibre does not copy metadata.db. Its safety net is a
metadata.opf file in each book folder plus metadata_db_prefs_backup.json in
the library root, which "Restore database" rebuilds from. Pandora's own
backups are exact database snapshots in its config folder, taken before the
first write of a session, skipped when identical to the newest, and limited
to the newest ten per library. A snapshot cannot undo a rename, so every
folder and file move is also appended to a rename log next to the backups.
There is no restore button yet.

OPF files. Pandora does not write metadata.opf yet, so an edit leaves that
book's OPF out of date until calibre rewrites it (calibre's "queue all
books for backup" regenerates all of them). This matters only if
metadata.db is ever rebuilt from OPFs. The decision is to build the full
standalone version later: generate the whole file from the database, not
patch the existing one, and verify it with a round trip through calibre's
Restore database on a scratch copy.

### Author sort, derived automatically, no separate field

calibre keeps author name and author sort as two separate fields the user
can edit independently. If someone types a display name but sets a
mismatched or manually typed sort string, calibre ends up with two
genuinely distinct rows in its authors table (for example "Andy Weir" and
"Weir, Andy"), each with its own id, each linked to its own folder on disk,
even though a human reading them knows it is the same person. This is real
data, not a display bug, and any tool honestly reading calibre's schema
will surface the same split.

Pandora's fix: there is no separate author sort field exposed to the user
at all. The user types a name once, and Pandora derives the sort string
automatically on save. This removes the failure mode structurally rather
than detecting or merging duplicates after the fact.

The algorithm, confirmed against calibre's own actual behavior on real
examples, turns out to be simpler than expected:

1. Take the last word of the name as the surname.
2. Everything before that last word becomes the first name portion, kept
   in its original order, with no special handling for surname particles
   (van, de, Le, and so on).
3. Exception: if the last word is a recognized suffix (Jr., Sr., III, and
   similar), back up one more word to find the actual surname instead, and
   move the suffix to the very end of the sort string rather than treating
   it as the surname.
4. Result format: "Surname, First name portion [suffix if any]"

Verified test cases (calibre's actual output, confirmed by hand):

- Martin Luther King Jr. -> King, Martin Luther Jr.
- Ludwig van Beethoven -> Beethoven, Ludwig van
- Gabriel Garcia Marquez -> Marquez, Gabriel Garcia
- Ursula K. Le Guin -> Guin, Ursula K. Le

Notably, "van" and "Le" are not treated as part of the surname in these
real examples, despite genuinely being part of the compound surname in
normal usage. This is calibre's own actual convention, not a guess, so
Pandora should match it rather than trying to be "more correct" and
diverging from what calibre itself produces.

Resolved: the word lists are calibre's default tweaks (the copy method is
'comma', plus three lists), confirmed from calibre's published defaults and
by testing on a real library. Prefixes (Mr, Mrs, Ms, Dr, Prof) are dropped
from the sort entirely. Suffixes (Jr, Sr, Inc, Ph.D, Phd, MD, M.D, I, II,
III, IV, Junior, Senior) move to the end. A name containing a "copy word"
(Agency, Corporation, Company, Co., Council, Committee, Inc., Institute,
National, Society, Club, Team) is left unchanged, and so is a name that
already contains a comma. These are only the defaults: calibre keeps the
user's own tweaks in its config folder, not in metadata.db, so Pandora
cannot see customized lists. This affects only the sort given to a brand
new author. The implementation has a unit test over ten cases taken from
calibre's real output. Not handled: calibre also strips text in brackets
before sorting, which has not been verified.

Additional confirmed rule, tested directly rather than assumed: if the
name contains no whitespace at all, author sort is left completely
unchanged, identical to the name. This is not a script or language
specific carve out (an earlier draft of this note wrongly framed it as
being about "non-Western names," which was inherited uncritically from
calibre's own FAQ phrasing rather than checked). The real and only
variable is whether the string has a space to split on at all. Confirmed
directly: a Hindi name in Devanagari script with spaces (Ramdhari Singh
Dinkar) split and rearranged exactly like an English name would. A
Chinese name with no space at all (Mo Yan, written with no space between
the two characters) was left completely unchanged, then, once a space was
manually inserted between the two characters, it split and rearranged the
same way any other two token name would. So the rule is genuinely
script agnostic: it operates on whitespace tokens, nothing else. Any name
with no space in it (this includes plenty of single word Western stage
names or mononyms, not just non-Latin scripts) will hit the same "nothing
to split on, leave unchanged" fallback.

This also means any book whose author is typed or edited through Pandora
can never produce the Weir-Andy-vs-Andy-Weir style split in the first
place, since there is only ever one field to type into.

### Title sort, same approach, a much narrower rule

calibre also auto derives a title sort value, and it turns out to be an
even simpler rule than author sort: if the title begins with a leading
article (a, an, the), move just that one word to the end of the string,
separated by a comma. If the title does not begin with one of those three
words, the title is left completely unchanged. Nothing else is inspected,
an article appearing anywhere other than the very first word is never
touched.

Verified test cases:

- The Three-Body Problem -> Three-Body Problem, The
- The Dark Forest -> Dark Forest, The
- Death's End -> Death's End (no leading article, unchanged)
- East of Eden -> East of Eden (no leading article, unchanged)
- The Man in the High Castle -> Man in the High Castle, The (the mid title
  "the" in "in the High Castle" is correctly left alone, only the leading
  one moves)

Given how narrow and confirmed this rule is, title sort gets the same
treatment as author sort: it is derived automatically on save, with no
separate editable field. This is built, and the derived value matched
calibre's stored sort on every book in the test library.

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

Design idea for when conversion arrives: build it as a separate worker
binary tuned for speed (opt-level 3, aggressive inlining), spawned by the
main app only when sustained work is needed. The main app stays tuned for
size and instant startup (size optimization, panic set to abort, stripped
symbols). Both come from the same Rust workspace. The cost is a second
build target and a small job protocol between the two. Instant startup is a
size and loading concern, while sustained conversion is a speed concern,
which is why they pull in opposite directions.

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

## Cleanup backlog

Left over from the code review, none of it blocking:

- [ ] A proper error type instead of stringly typed errors, and splitting
      main.rs into commands.rs and config.rs. library.rs has also grown
      large and could split into queries, writes and naming
- [ ] Filter state owned by the page instead of duplicated in Sidebar
- [ ] One FilterSection component instead of three near identical blocks
      for Authors, Series and Tags
- [ ] Named constants for the panel minimum heights (120 and 280)
- [ ] Simpler drag code using pointer capture on the resizer itself
- [ ] Turn the verified naming and sort cases into unit tests (title_sort,
      folder and file names), the way author sort already is
- [ ] An intermittent sidebar bug: once, after a title and author edit, the
      authors list did not show a new author until a restart. It did not
      reproduce, so it is not understood yet

---

## Rendering engine watch: Servo

Pandora's webview, like any Tauri app, uses whatever native webview the
host OS already provides: WebView2 (Chromium based) on Windows, WKWebView
(WebKit) on macOS, WebKitGTK (WebKit) on Linux. This is not a choice
Pandora made in favor of Chromium specifically, it is a side effect of
which engines the underlying OS vendors chose to expose as stable,
embeddable, auto updating system components in the first place. Nobody
building a lightweight native app framework today has a Gecko option:
Mozilla built one once (XULRunner) and deliberately deprecated it, by
their own account, specifically so Gecko's internals could keep changing
freely without being locked down by external third party consumers
depending on a frozen interface. GeckoView exists as a modern equivalent
but is scoped to Android only, not desktop.

Servo (originally Mozilla Research, transferred to the Linux Foundation in
2020, then Linux Foundation Europe in 2023) is a genuinely active,
Rust native, from scratch browser engine, and as of September 2026 it is
meaningfully further along than "abandoned research project": published
as a real crate on crates.io, actively developed (488 commits in a single
month as of mid 2026), and its own team explicitly distinguishes the risk
profile of "rendering your own known, trusted content" (Pandora's actual
use case, since Pandora only ever renders its own UI, never arbitrary
internet content) from "rendering arbitrary web content," calling the
former a meaningfully safer category today.

Two real gates before this becomes worth actually doing, not one:

1. Servo reaching a genuine 1.0, or an equivalent explicit "safe for
   production embedding" milestone from its own team, since as of now
   Servo's own documentation says plainly that production usage should be
   carefully evaluated pre 1.0.
2. Separately, and this is easy to undercount: Servo (or someone) would
   need to build an Evergreen style system wide, auto updating
   redistributable runtime with a stable public interface, the same
   category of infrastructure Microsoft built for WebView2. Today, Servo
   is a compile time Rust crate you statically link into your own binary,
   not a runtime component apps can point at on a user's system. Without
   that second piece, adopting Servo means compiling an entire browser
   engine into Pandora's own binary, the same architectural move Electron
   makes with Chromium, which would trade away Tauri's core size advantage
   entirely (likely pushing Pandora from roughly 10 to 20MB into
   Electron adjacent territory) for uncertain, unproven real world
   performance, since Servo's parallel rendering architecture is
   promising on paper but has not had anywhere near WebView2's decade of
   real world runtime optimization yet.

There is also a longer term dependency risk worth naming honestly, since
switching engines trades one kind of risk for a different kind rather
than removing risk outright. Staying on the OS native webview means the
maintenance burden belongs to Microsoft, Apple, or the relevant Linux
desktop, organizations with every incentive to keep their own engine
alive indefinitely. Embedding Servo directly would make that Pandora's own
responsibility. If Servo ever stopped being maintained, already built
copies of Pandora would keep working exactly as shipped, nothing
self destructs, but Pandora would stop receiving fixes and new standards
support, and since Servo is genuinely open source under Linux Foundation
governance, the code itself would remain forkable by anyone motivated
enough, the same safety net XULRunner theoretically had and nobody
actually used.

Action: revisit this as a real, tracked option once both gates above are
met, not before. Until then, treat it as a side branch experiment at most,
never a dependency Pandora's main line actually ships on.

---

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
4. Writing is earned, not assumed. Every write path is tested against a
   copy of a real library and then checked by opening that copy in
   calibre. metadata.db is backed up before the first write of a session,
   each save is one transaction, and a failed save must leave both the
   database and the disk as they were.
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
