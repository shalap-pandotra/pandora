# Pandora Reader: development log

A record of the session that built the reader, 11 October 2026: what was built, every bug and error we ran into, what caused each one, and how it was fixed. The design itself is in `reader-spec.md`. This file covers what happened while building it, including where the build moved away from the spec.

---

## 1. What was built

All five steps of the spec's build order are done, plus several additions.

| Step | Status | Summary |
| --- | --- | --- |
| 1. Bare reader | Done | A reader window opens from Pandora, one per book. Rust serves the EPUB one file at a time and scripts are blocked. |
| 2. Window and settings | Done | Custom slim title bar, percentage, font size, padding panel (Alt+S or Ctrl+comma), and a settings file shared by every reader window. |
| 3. Position | Done | Saved per book and restored on open. |
| 4. Ctrl+O picker | Done | Lists library books by title and author, with search. Browse… opens any EPUB on disk. The book is swapped in the same window. |
| 5. Library teardown | Done | Closing the library releases the database. A second launch brings the library back in the running copy, using the single-instance plugin. |

**Added beyond the spec:**
- **Movement:** one wheel notch moves one line. Up and Down move a line. Page Up, Page Down and Shift+Space move a screen.
- **Pictures:** each picture gets a screen (or a page) of its own.
- **Two-column layout:** an option in the settings panel. It falls back to one column automatically when the window is too narrow.
- **F11:** full screen, which hides the title bar.
- **V in the library:** opens the selected book.
- **Hover colours:** on the title bar buttons, red for close.
- **Slimmer title bar:** in both windows.
- **Animations:** the settings panel and the picker glide open.
- **Library:** no scroll bounce anywhere, and the filter sections start collapsed.
- **Edit dialog:** the cover panel now slides out from the right.

### Files

- **New:**
  - `src-tauri/src/reader.rs`
  - `src/routes/reader/+page.svelte`
  - `src/lib/ReaderSettings.svelte`
  - `src/lib/ReaderPicker.svelte`
  - `src/lib/foliate/` (the copied library)
- **Changed:**
  - `src-tauri/src/main.rs`
  - `src-tauri/src/library.rs`
  - `src-tauri/Cargo.toml` and `Cargo.lock` (new crates: `zip`, `percent-encoding`, `tauri-plugin-single-instance`)
  - `src-tauri/capabilities/default.json`
  - `src/lib/TopBar.svelte`
  - `src/lib/PreviewPanel.svelte`
  - `src/lib/Sidebar.svelte`
  - `src/lib/EditMetadataModal.svelte`
  - `src/lib/theme.css`
  - `src/routes/+page.svelte`
- **Saved data**, in the app config folder:
  - `reader-settings.json`: font step, padding, two columns
  - `reader-positions.json`: reading position per book

---

## 2. Architecture as built

- **Windows:** one process. Reader windows are labelled `reader-N` and created by Rust (`reader::open_window`).
- **Serving the book:** the EPUB stays open in Rust as a zip archive. Each entry is served on request through a custom `book://` protocol (`http://book.localhost/<label>/<entry>` on Windows). The webview never holds the whole file.
- **Rendering:** foliate-js, copied at commit `78914aef` (1 May 2026).
- **Scripts in books:** never run. Script files are never loaded, and every page gets a `Content-Security-Policy: script-src 'none'` meta tag.
- **One column:**
  - foliate's scrolled mode, but the reader does all the moving itself.
  - Each chapter gets a list of stops. Text stops are line tops, and each picture is a stop of its own.
  - The wheel moves one stop. Space and the arrows move a screen's worth of whole lines.
- **Two columns:** foliate's paginated mode with two columns, turned a spread at a time.
- **The resting place:** where you are is remembered as a spot in the text (the character at the top of the screen) or as a picture, never as a pixel offset. Any scroll the reader didn't make, and any layout change, settles back onto that spot.
- **Book identity:** books are identified by the identifier inside the EPUB, falling back to the file path. The spec left this open, and the path breaks when Pandora renames folders.

---

## 3. Bugs and errors, in the order we hit them

### Environment and tooling

| # | Problem | Cause | Fix |
| --- | --- | --- | --- |
| E1 | `cargo` not found in my shell | Rust is installed through Scoop at `C:\Users\shalap\scoop\persist\rustup\.cargo\bin`, which wasn't on PATH | My builds ran through a script that loads the MSVC environment and that folder |
| E2 | `cargo check` failed on `vswhom-sys`: "cl.exe not found" | The build needs the Visual Studio C++ tools environment | Ran builds through `vcvars64.bat` |
| E3 | `npm run tauri dev`: "failed to run cargo metadata: program not found" | Your user PATH had lost both `scoop\shims` and the cargo bin folder. It contained only `.local\bin`. This happened outside the session, before it started. | Gave you the commands to add both back to PATH, for the session or permanently |
| E4 | `git clone` of foliate failed: "Filename too long" | The scratchpad path was too long for Windows | Cloned to a short temp path |
| E5 | `python` not available | Not installed | Used the editor and sed instead |
| E6 | A long bash heredoc broke the shell ("unexpected EOF") | Quoting inside the heredoc | Wrote the text with the file tool instead |
| E7 | Debug script couldn't find the reader window | Git Bash rewrote the argument `/reader` into a Windows path | Matched on `1420/reader` instead |
| E8 | Debug tool: "Origin header is not a valid URL" | Your `tauri dev` (and its Vite server on port 1420) stopped mid-session, so the windows lost their backend | Ran my own Vite server only while testing |
| E9 | Debug copy exited at once | Once single-instance was added, a second Pandora hands over to the running one and exits | Only debug while your copy is closed, and check first (saved as a memory note) |
| E10 | The debug copy dropped the new `two_columns` setting | I had only type-checked the Rust (`cargo check`), not rebuilt it | Rebuilt with `cargo build` |
| E11 | `npm run check` reported 1,766 errors | About 1,630 came from the copied foliate files. The other roughly 140 were already in Pandora's own code, nearly all "implicit any". | Added `// @ts-nocheck` to each foliate file. Pandora's own 140 were left. |
| E12 | `git add -N` changed the git index while I was counting lines | My mistake | Undid it with `git reset` on those paths |

### Reader bugs

| # | Symptom | Cause | Fix |
| --- | --- | --- | --- |
| R1 | Reader window couldn't set its title | Missing permission `core:window:allow-set-title` | Added it to the window permissions |
| R2 | Images vanished after changing chapter | Width was measured from the image's parent before the new chapter had been laid out, so it read 0 | Sized pictures with CSS only, measuring nothing |
| R3 | Wheel scrolling stopped at a chapter's end | foliate shows one chapter at a time, and only its keyboard paging moves to the next | Added an "extra push at the end" rule, later replaced by line stops (R14) |
| R4 | Pictures not on their own page: one Space showed a sliver of the picture, the next showed it | Screen-step guessing around pictures, plus WebView2 delaying images below the screen, so they hadn't been sized yet | Several attempts, ending in the line-stop design (R14) and reading image sizes ahead of time (R9) |
| R5 | Switching to pages (paginated) left a blank page before every picture | 1. `100vh` rounds to 617 px while the page is 616.67 px, so the picture didn't fit. 2. The page break moved up to the `<p>` around the picture, which kept its top margin. 3. Empty page-number markers (`<span epub:type="pagebreak"/>`) still count as a line. | Picture height `calc(100vh - 1px)`, wrapper margins cleared, empty markers taken out of the flow. 54 blank pages went to 0 in "abcd". |
| R6 | Pages mode removed free scrolling | My mistake: the option I offered said the wheel turns pages, but I didn't say plainly that this drops the spec's free scrolling | Went back to scrolling, then designed line-by-line scrolling with you |
| R7 | Long automated whole-book test runs | My process. They took too long. | You asked me to stop. Saved as a memory note: make the fix, you test it. |
| R8 | Sliver still appeared after the scrolling fix | Space stopped short for a paragraph's empty margin, as if text was left | "Finish the text first" now only applies if real text is left (later replaced by R14) |
| R9 | Sliver still appeared | WebView2 lazy-loads images below the screen and holds back their load events, so pictures weren't sized when Space looked ahead | Read each image's size from its data the moment a chapter loads. All images set to load straight away. |
| R10 | Text with a picture just below it showed the top of the picture on the same screen (your screenshot) | One continuous scroll strip | Pictures are hidden unless the screen rests exactly on them, then line-by-line scrolling (R14) |
| R11 | Blank space before pictures, as an idea | You rejected it, since mid-scroll you'd see a gap scroll by | Not used. Led to the line-by-line design. |
| R12 | Lines clipped in half at the top and bottom | Screen height is rarely a whole number of lines. The stop before a picture lined up the bottom, not the top. | Stops always start on a line top, and a cover hides any half-shown bottom line |
| R13 | Up and Down arrows did full page turns | A stale window running older code. It worked once the window was reopened. | None needed |
| R14 | Design change: line-by-line scrolling | Replaces R3, R4, R8 and R10 | One wheel notch is one line. Lines stop when the last line before a picture reaches the bottom. One notch turns to the picture, the next back to text. |
| R15 | At the end of a book, nothing worked, scrolling or keys | foliate's "next" past the last chapter throws before clearing its busy flag, so it ignored all input from then on | The reader moves between chapters itself and does nothing at either end of the book |
| R16 | A blank page when going back into a chapter | The reader jumped to the chapter's end before its pictures were sized, and the layout grew afterwards | Wait until a chapter's pictures are sized before settling |
| R17 | Clipping in Project Hail Mary at chapter starts (screenshot) | Heading images loaded late and moved every line after the reader had measured. foliate also scrolled back to its own stale remembered place after re-layouts, which caused the blank page going back. | Remembered place as a spot in the text, a watch on each chapter's size, and a guard that undoes any scroll the reader didn't make |
| R18 | Covers stretched to full screen in 4 books | calibre writes `preserveAspectRatio="none"` when "preserve cover aspect ratio" is off at conversion | Cover SVGs set to keep their shape, sized from their proportions |
| R19 | Green Mars clipped at the top and bottom | Tight line spacing (38 px text boxes on 28 px lines) made neighbouring lines overlap, so every paragraph was read as one giant "line" | Lines only merge when they mostly overlap, and the edge between overlapping lines is the middle of the overlap. A general rule, not specific to the book. |
| R20 | Book window closed when a book was opened quickly after launch | In dev, Vite reloads a page the first time it uses new libraries. Rust handed a window its file only once, so the reload found "No book for this window". | Each window keeps its file for its whole life |
| R21 | Two-column mode turned spreads in a one-column view, cutting text (Striders) | "Do two columns fit?" was asked fresh everywhere and depends on text size learned per chapter, so near the borderline it changed answer | One source of truth: the layout actually on screen, re-judged only when its inputs change |
| R22 | Space moved only about 140 px before a picture and re-showed text (Striders screenshot) | The "finish text before picture" stop put the last line at the bottom, which is only a small step when the picture comes soon | Space now puts the first unseen line at the top and leaves the rest of the screen empty. The wheel keeps its rule. |
| R23 | Two-column padding applied to the whole spread, not each page | foliate gives the outer edges the same space as the middle gap | Gap set to left plus right padding, and foliate's area widened past the edges by the difference |
| R24 | A narrow tall window showed one column even with two chosen | foliate's own fallback for windows taller than they are wide | Overridden on foliate's layout element. Later the reader's own room check (R25) handles narrow windows. |
| R25 | Asked for: two columns visually fall back to one when too narrow | — | Each column needs at least 20 em of text, measured in the book's text size at the current font step |

### Library and window bugs

| # | Symptom | Cause | Fix |
| --- | --- | --- | --- |
| L1 | Title bar hover colour covered the dividers | Each divider was drawn inside the next button. A first fix (a 1 px strip) failed because buttons have an invisible 1 px border. | Dividers are now separate elements between the buttons |
| L2 | Scroll bounce in the library | WebView2's elastic overscroll on touchpads | `overscroll-behavior: none` everywhere, in `theme.css` |
| L3 | Cover panel slid out from the left | `order: -1` on `.cv-panel` | Removed it, and moved its divider to its left edge |

---

## 4. Changes from the spec

| Spec | As built | Why |
| --- | --- | --- |
| Scrolled layout only, free scrolling, Space jumps a screen minus overlap | One column: the wheel moves one line, Space a screen of whole lines, with no overlap needed. Two columns are an option. | Pictures could never sit cleanly on their own screen in a free scroll |
| Reading position identity left open | The EPUB's internal identifier, falling back to the path | Pandora renames folders, which breaks paths |
| foliate copied and left unchanged | Two marked changes: `// @ts-nocheck` in each file, and `scrollbar-width: none` in `paginator.js`. Our own PDF stand-in. | Type checking, and the scrollbar is inside foliate's closed shadow DOM |
| Ctrl+comma undecided | Opens the settings panel, the same as Alt+S | You decided |
| Reading starts from Pandora: undecided | The "Open book" button and the V key | You decided |
| Padding defaults 32 and 48 | Kept. The bottom padding is never under 20 px, so the percentage fits. | Quiet clamp |

---

## 5. Known limits and things still open

- **Not handled:**
  - Vertical text, such as Japanese or Chinese read top to bottom.
  - Books with their own background colour: the bottom-line cover uses Pandora's white.
  - Unusual mixed lines, such as large superscripts, ruby annotations, or images within a line.
- **Fixed-pixel fonts:** books whose fonts are set in pixels don't change size with the font steps. The reader scales the base size.
- **App-wide security policy:** there's still no app-wide Content Security Policy. Scripts in books are blocked by the reader itself.
- **Two columns with uneven padding:** the middle gap is exact, but the outer edges may differ slightly.
- **Pandora's own type errors:** about 140 remain from before the session, mostly "implicit any".
- **foliate's unused formats:** MOBI, FB2, comics, text-to-speech and search are kept for later format support. Each loads only when needed.
- **Your uncommitted edits from before the session** are still in the working tree: the release profile in `Cargo.toml` and the 70 ms and 340 px edits in `EditMetadataModal.svelte`.
- **Not committed:** nothing from this session has been committed yet.
