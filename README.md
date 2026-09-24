# Pandora

A companion app for calibre libraries. Built from scratch in Rust and Svelte,
inspired by calibre-zen's interface ideas, but its own independent project
rather than a fork or a theme.

Pandora reads calibre's own metadata.db and library folder structure
directly. No Python anywhere in the stack, no calibre installation required
to browse a library, and no shelling out to calibre's own tools.

## What this is right now

Pandora is early and read-only. You can point it at an existing calibre
library and browse it: cover grid, a full metadata preview for the selected
book (cover, title, series, author, tags, description), and a filter panel
for authors, series, and tags with live search across all three.

Nothing is written back to your library yet. Metadata editing, conversion,
and a built in reader are all planned but not built. See [roadmap.md](roadmap.md) for the
full phase by phase plan and current status.

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
- Rust, for the backend: reading and eventually writing metadata.db,
  resolving covers, and later, format conversion
- Svelte, for the frontend

## Status

Phase 1 (read-only browsing) and phase 2 (filter panel, search, and most of
the intended UI polish) are done. See roadmap.md for what's next, including
the currently open gap around custom metadata columns (used for genre and
subgenre hierarchies in many real calibre libraries), which is the next
thing being picked up.

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
