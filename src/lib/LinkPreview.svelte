<script>
import { kindOf, previewFor, noteFor, addressOf, idsOf, labelAt } from './linkPreview.js';

let { getBook, onJump, delay = 300 } = $props();

const WIDTH = 340;
/** @type {{ x: number, y: number, above: boolean, max?: number, dir?: string, kind: string, pinned?: boolean, label?: string, href?: string, cut?: boolean, title?: string | null, html?: string, text?: string } | null} */
let box = $state(null);
let hoverLink = null;
let overBox = false;
let pinned = false;
/** The marker the pinned note belongs to, so a second click on it closes the box. */
let pinnedFor = null;
/** The marker whose pinned note the last click closed, if any. */
let closedFor = null;
/** Whether the last click on the book had Ctrl (or Cmd) held. */
let ctrl = false;
let token = 0;
/** @type {ReturnType<typeof setTimeout> | undefined} */
let showTimer;
/** @type {ReturnType<typeof setTimeout> | undefined} */
let hideTimer;
const indexOf = new WeakMap();

// The book's elements belong to another window, so instanceof Element would say no.
const linkAt = (/** @type {any} */ target) => (typeof target?.closest === 'function' ? target.closest('a[href]') : null);

/** Starts watching a chapter's document for pointers resting on links and for clicks. */
export function attach(/** @type {Document} */ doc, /** @type {number} */ index) {
  indexOf.set(doc, index);
  doc.addEventListener('mouseover', over);
  doc.addEventListener('mouseout', out);
  // Before the book's own link handling runs: note whether Ctrl was held, and close a pinned note,
  // since any click in the book dismisses it. Remembering which marker it belonged to lets a click
  // on that same marker close it for good instead of opening it again.
  doc.addEventListener(
    'click',
    (e) => {
      ctrl = e.ctrlKey || e.metaKey;
      closedFor = pinned ? pinnedFor : null;
      if (pinned) hide();
    },
    true
  );
}

export function hide() {
  clearTimeout(showTimer);
  clearTimeout(hideTimer);
  hoverLink = null;
  overBox = false;
  pinned = false;
  pinnedFor = null;
  token++;
  box = null;
}

export function isPinned() {
  return pinned;
}

/**
   * A click on a note marker shows its note here and stays. Returns false when the click should
   * jump instead: Ctrl was held, the link is not a note marker, or what it points at does not look
   * like a note (a heading, a whole section, or a link back from a note to its marker).
   */
export async function pinNote(/** @type {Element} */ a, /** @type {string} */ href) {
  if (closedFor === a) {
    closedFor = null; // the same marker again closes the box, and that click is the whole job
    return true;
  }
  const book = getBook();
  if (!book || ctrl || book.isExternal?.(href) || kindOf(a) !== 'footnote') return false;
  const found = await noteFor(book, href, idsOf(a), labelAt(a));
  if (!found) return false;
  clearTimeout(showTimer);
  token++;
  pinned = true;
  pinnedFor = a;
  // The popup is headed by the marker as printed in the book, like 1 or *3.
  const label = (a.textContent ?? '').replace(/\s+/g, ' ').trim();
  box = { ...place(a, 'footnote'), kind: 'footnote', pinned: true, label: label.length <= 12 ? label : '', href, ...found };
  return true;
}

function place(/** @type {Element} */ a, /** @type {string} */ kind) {
  const frame = a.ownerDocument.defaultView?.frameElement?.getBoundingClientRect();
  const r = a.getClientRects()[0] ?? a.getBoundingClientRect();
  const left = (frame?.left ?? 0) + r.left;
  const right = (frame?.left ?? 0) + r.right;
  const top = (frame?.top ?? 0) + r.top;
  const bottom = (frame?.top ?? 0) + r.bottom;
  const wide = kind === 'outside' ? 320 : WIDTH;
  const below = window.innerHeight - bottom;
  const above = top > below && below < 220;
  const x = Math.max(8, Math.min((left + right) / 2 - wide / 2, window.innerWidth - wide - 8));
  // The box never reaches past the edge of the window, however long the note is.
  const max = Math.max(120, Math.min(320, (above ? top : below) - 14));
  return { x, y: above ? window.innerHeight - top + 6 : bottom + 6, above, max };
}

function over(/** @type {Event} */ e) {
  if (pinned) return; // a pinned note stays until it is closed
  const a = linkAt(e.target);
  if (!a) return;
  clearTimeout(hideTimer);
  if (a === hoverLink) return;
  hoverLink = a;
  clearTimeout(showTimer);
  showTimer = setTimeout(() => open(a), delay);
}

function out(/** @type {any} */ e) {
  if (pinned) return;
  const a = linkAt(e.target);
  if (!a || linkAt(e.relatedTarget) === a) return;
  clearTimeout(showTimer);
  hoverLink = null;
  scheduleHide();
}

function scheduleHide() {
  clearTimeout(hideTimer);
  if (pinned) return;
  hideTimer = setTimeout(() => {
    if (overBox) return;
    token++;
    box = null;
  }, 160);
}

async function open(/** @type {Element} */ a) {
  const book = getBook();
  const index = indexOf.get(a.ownerDocument);
  if (!book || index === undefined) return;
  const raw = a.getAttribute('href') ?? '';
  const href = book.sections[index]?.resolveHref?.(raw) ?? raw;
  const mine = ++token;
  /** @type {any} */
  let content;
  if (book.isExternal?.(href)) content = { kind: 'outside', text: addressOf(raw) };
    else {
      const kind = kindOf(a);
      const found = await previewFor(book, href, kind, idsOf(a), labelAt(a)).catch(() => null);
      if (!found) return;
      content = { kind, href, ...found };
    }
  if (mine !== token || hoverLink !== a || pinned) return; // the pointer moved on while this was loading
  box = { ...place(a, content.kind), ...content };
}

function readMore() {
  const href = box?.href;
  hide();
  if (href) onJump?.(href);
}
</script>

<svelte:window
  onblur={hide}
  onpointerdown={(e) => {
    if (pinned && !(e.target instanceof Element && e.target.closest('.link-preview'))) hide();
  }}
/>

{#if box}
  <div
    class="link-preview"
    class:outside={box.kind === 'outside'}
    class:pinned={box.pinned}
    dir={box.dir ?? 'ltr'}
    role="tooltip"
    style:left={box.pinned ? null : `${box.x}px`}
    style:top={box.pinned || box.above ? null : `${box.y}px`}
    style:bottom={box.pinned || !box.above ? null : `${box.y}px`}
    style:max-height={box.pinned || !box.max ? null : `${box.max}px`}
    onmouseenter={() => {
      overBox = true;
      clearTimeout(hideTimer);
    }}
    onmouseleave={() => {
      overBox = false;
      scheduleHide();
    }}
  >
    {#if box.pinned}
      <div class="lp-head">
        <span class="lp-label">{box.label}</span>
        <button class="lp-close" aria-label="Close" onclick={hide}>&times;</button>
      </div>
    {/if}
    {#if box.kind === 'outside'}
      {box.text}
    {:else}
      {#if box.title}<div class="lp-title">{box.title}</div>{/if}
      <div class="lp-body">{@html box.html}</div>
      {#if box.cut}<button class="lp-more" onclick={readMore}>...read more</button>{/if}
    {/if}
  </div>
{/if}

<style>
.link-preview {
  position: fixed;
  z-index: 3;
  box-sizing: border-box;
  width: 340px;
  max-height: min(320px, 45vh);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  padding: 10px 14px;
  background: var(--zen-panel);
  color: var(--zen-ink);
  border: 1px solid var(--zen-border);
  border-radius: 6px;
  box-shadow: 0 6px 22px rgba(0, 0, 0, 0.08);
  font: 13px/1.5 var(--zen-font-sans);
  overscroll-behavior: none;
}
.link-preview.pinned {
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
  width: min(460px, calc(100vw - 32px));
  max-height: min(70vh, 560px);
  padding: 0;
  font-size: 14px;
  box-shadow: 0 12px 48px rgba(0, 0, 0, 0.18);
}
.link-preview.pinned .lp-body { padding: 14px 18px 12px; }
.link-preview.outside {
  display: block;
  width: auto;
  max-width: min(420px, calc(100vw - 16px));
  padding: 5px 10px;
  font-size: 12px;
  color: var(--zen-text-secondary);
  overflow-wrap: anywhere;
}
.lp-title { flex: none; font-weight: 600; margin-bottom: 4px; }
.lp-body { flex: 1 1 auto; min-height: 0; overflow-y: auto; overscroll-behavior: none; }
.lp-body :global(p) { margin: 0 0 6px; }
.lp-body :global(p:last-child) { margin-bottom: 0; }
.lp-head {
  flex: none;
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 36px;
  padding-inline: 18px 8px;
  border-bottom: 1px solid var(--zen-border);
}
.lp-label { font-weight: 600; }
.lp-close {
  width: 26px;
  height: 26px;
  padding: 0;
  border: none;
  border-radius: 3px;
  background: none;
  color: var(--zen-ink);
  font: 600 22px/26px var(--zen-font-sans);
  cursor: pointer;
}
.lp-close:hover { background: var(--zen-pill-bg); }
.lp-more {
  display: block;
  flex: none;
  align-self: flex-end;
  margin-top: 8px;
  padding: 0;
  border: none;
  background: none;
  color: #0ba6e9;
  direction: ltr;
  font: inherit;
  cursor: pointer;
}
.lp-more:hover { text-decoration: underline; }
.link-preview.pinned .lp-more { margin: 0; padding: 0 18px 14px; }
</style>
