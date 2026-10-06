# Pandora

A companion app for calibre libraries. Built from scratch in Rust and Svelte,
inspired by calibre-zen's interface ideas, but its own independent project
rather than a fork or a theme.

Pandora reads calibre's own metadata.db and library folder structure
directly. No Python anywhere in the stack, no calibre installation required
to browse a library, and no shelling out to calibre's own tools.

## What this is right now

Pandora is early. You can point it at an existing calibre library (it
remembers the last one, and Ctrl+O picks a different folder) and browse it:
cover grid, a full metadata preview for the selected book (cover, title,
series, author, rating, tags, description), a filter panel for authors,
series, tags and your own custom columns (dot separated genre hierarchies
show as a collapsible tree), and live search across titles, authors and
tags.

It can also edit. The Edit metadata dialog changes a book's title, authors,
series and number, rating, tags and description. Title and author edits
rename the book's folder and files on disk the way calibre does, and each
kind of change has been checked by opening the result in calibre. Pandora
backs up metadata.db before the first save of each session and keeps the
newest ten backups. Editing is still young, so work on a copy of your
library and keep your own backup of the folder.

Not built yet: editing the remaining fields (publisher, languages,
identifiers, dates, cover, custom columns), adding and removing books,
conversion, and a built in reader. See roadmap.md for the phase by phase
plan and current status.

## Why this exists

calibre-zen is a genuine, well made fork of calibre with a reworked
interface, but it is written in Python and forks calibre's own codebase
directly. This project exists because a from scratch, Python free
alternative was more appealing to me personally than restyling calibre from the inside.

The design goal is for Pandora to eventually stand fully on its own (no
calibre installation needed at all, not even for conversion), while
permanently staying compatible with calibre's own library format. A library
built or edited in Pandora should always be a library calibre itself can
still open.

## Stack

- Tauri, for the native shell and packaging
- Rust, for the backend: reading and writing metadata.db, renaming book
  folders and files, resolving covers, and later, format conversion
- Svelte, for the frontend

## Status

Phases 1 and 2 (browsing, filters, search, and the interface polish) are
done. Phase 3 (metadata editing) is in progress: description, rating, tags,
series, title and authors can be edited. Next up is editing custom columns
such as genre. See roadmap.md for the full plan.

## Building it yourself

This is a Tauri project. You'll need Rust and Node installed.

```
npm install
npm run tauri dev
```

On Windows, this project uses the MSVC toolchain rather than the GNU one.
If you hit linker errors mentioning mingw or gcc, make sure your default
Rust toolchain is stable-x86_64-pc-windows-msvc, and that you have Visual
Studio Build Tools installed with the Desktop development with C++
workload.

## License

MIT. See LICENSE.
