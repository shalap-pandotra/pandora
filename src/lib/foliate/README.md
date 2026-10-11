# foliate-js

A fixed copy of [foliate-js](https://github.com/johnfactotum/foliate-js), MIT licensed
(see LICENSE). It is not tracked or updated.

- Commit: 78914aef4466eb960965702401634c2cb348e9b1 (1 May 2026)
- Copied: the modules `view.js` needs, plus `vendor/zip.js` and `vendor/fflate.js`.
- Left out: the upstream `pdf.js` and `vendor/pdfjs`. `pdf.js` here is a stub of our own.

Changes to the upstream files, all marked so they can be redone after replacing the copy:

- A `// @ts-nocheck` line at the top of each file, so `npm run check` skips them.
- `paginator.js`: `scrollbar-width: none` on the scrolled `#container`, because the
  reader shows no scrollbar. Its shadow root is closed, so this cannot be done from outside.

Keep the changes this small, so the copy can be compared with a later commit.
