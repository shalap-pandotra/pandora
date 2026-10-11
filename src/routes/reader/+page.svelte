<script>
import '../../lib/theme.css';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { onMount, tick } from 'svelte';
import TopBar from '../../lib/TopBar.svelte';
import ReaderSettings from '../../lib/ReaderSettings.svelte';
import ReaderPicker from '../../lib/ReaderPicker.svelte';
import { open as openFileDialog } from '@tauri-apps/plugin-dialog';
import { EPUB } from '../../lib/foliate/epub.js';
import '../../lib/foliate/view.js';

const appWindow = getCurrentWindow();

// Must match the limits in reader.rs.
const FONT_STEP_MIN = -5;
const FONT_STEP_MAX = 10;
const FONT_STEP_FACTOR = 1.1;
// Left and right padding never squeeze the text narrower than this, nor top and bottom shorter.
const MIN_TEXT_WIDTH = 240;
const MIN_TEXT_HEIGHT = 120;
// The percentage sits in the bottom padding, which is never thinner than this.
const PERCENT_ROOM = 20;

let title = $state('');
let percent = $state(/** @type {number | null} */ (null));
let panelOpen = $state(false);
let panelHeight = $state(0);
let pickerOpen = $state(false);
let pickerError = $state('');
let fullscreen = $state(false);
/** Height of the cover over a part-shown line at the bottom of the screen (see coverCutLine). */
let cutoff = $state(0);
let settings = $state({ font_step: 0, padding: { top: 32, right: 48, bottom: 32, left: 48 }, two_columns: false });
let area = $state({ width: 0, height: 0 });
let readingEl = $state();
/** @type {any} */
let view;
let positionKey = '';
let ready = false;

// What is applied, after the quiet clamps that depend on the window's size.
let inset = $derived.by(() => {
  let { top, right, bottom, left } = settings.padding;
  bottom = Math.max(bottom, PERCENT_ROOM);
  const fit = (/** @type {number} */ a, /** @type {number} */ b, /** @type {number} */ room) => {
    const k = a + b > room ? Math.max(0, room) / (a + b) : 1;
    return [Math.floor(a * k), Math.floor(b * k)];
  };
  if (area.width) [left, right] = fit(left, right, area.width - MIN_TEXT_WIDTH);
  if (area.height) [top, bottom] = fit(top, bottom, area.height - MIN_TEXT_HEIGHT);
  return { top, right, bottom, left };
});

// ---- Loading the book

// Placed on every page of the book. Scripts inside an EPUB never run.
const NO_SCRIPTS = `<meta http-equiv="Content-Security-Policy" content="script-src 'none'"/>`;
const PAGE_TYPES = ['application/xhtml+xml', 'text/html'];

/** @param {string} html */
function blockScripts(html) {
  if (typeof html !== 'string') return html;
  const head = html.match(/<head\b[^>]*>/i);
  if (head?.index != null) {
    const at = head.index + head[0].length;
    return html.slice(0, at) + NO_SCRIPTS + html.slice(at);
  }
  return NO_SCRIPTS + html;
}

/** Entries are fetched from Rust one at a time, as the book asks for them. */
function makeLoader(/** @type {{name: string, size: number}[]} */ entries) {
  const sizes = new Map(entries.map((e) => [e.name, e.size]));
  const url = (/** @type {string} */ name) => convertFileSrc(`${appWindow.label}/${name}`, 'book');
  const get = async (/** @type {string} */ name) => {
    if (!sizes.has(name)) return null;
    const res = await fetch(url(name));
    if (!res.ok) throw new Error(`${name}: ${res.status}`);
    return res;
  };
  return {
    loadText: async (/** @type {string} */ name) => (await get(name))?.text() ?? null,
    loadBlob: async (/** @type {string} */ name) => (await get(name))?.blob() ?? null,
    getSize: (/** @type {string} */ name) => sizes.get(name) ?? 0,
    sha1: undefined, // foliate falls back to Web Crypto
  };
}

/** Metadata titles are either a string or a map of languages to strings. */
function titleOf(/** @type {any} */ meta) {
  const t = meta?.title;
  if (typeof t === 'string') return t;
  if (t && typeof t === 'object') return Object.values(t)[0] ?? '';
  return '';
}

// A book is known by the identifier inside it, which survives the file being moved or
// renamed. Without one, its path is the best there is.
function keyFor(/** @type {any} */ book, /** @type {string} */ path) {
  const id = book.metadata?.identifier;
  return typeof id === 'string' && id.trim() ? `id:${id.trim()}` : `path:${path}`;
}

// ---- Font size

/** The book's own root size for each page, before any step is applied. */
const baseSizes = new WeakMap();

function applyFont(/** @type {Document} */ doc) {
  const root = doc.documentElement;
  if (!root || !doc.defaultView) return;
  if (!baseSizes.has(doc)) baseSizes.set(doc, parseFloat(doc.defaultView.getComputedStyle(root).fontSize) || 16);
  const size = baseSizes.get(doc) * FONT_STEP_FACTOR ** settings.font_step;
  root.style.setProperty('font-size', `${size}px`, 'important');
}

let appliedStep = 0;
$effect(() => {
  const step = settings.font_step;
  if (!view || step === appliedStep) return;
  appliedStep = step;
  for (const { doc } of view.renderer.getContents()) {
    applyFont(doc);
    layoutFigures(doc);
  }
  // Re-rendering scrolls back to the text that was at the top, so the place is kept.
  rerender();
});

function stepFont(/** @type {number} */ delta) {
  const font_step = Math.min(FONT_STEP_MAX, Math.max(FONT_STEP_MIN, settings.font_step + delta));
  if (font_step !== settings.font_step) saveSettings({ ...settings, font_step });
}

// ---- Settings, shared with every other reader window

function saveSettings(/** @type {typeof settings} */ next) {
  settings = next;
  invoke('set_reader_settings', { settings: next })
    .then((stored) => {
      if (JSON.stringify(stored) !== JSON.stringify(settings)) settings = /** @type {any} */ (stored);
    })
    .catch((e) => console.error('Could not save the reader settings:', e));
}

// ---- Position

/** @type {ReturnType<typeof setTimeout> | undefined} */
let saveTimer;
/** @type {{cfi: string, fraction: number} | null} */
let unsaved = null;

function savePositionSoon(/** @type {string} */ cfi, /** @type {number} */ fraction) {
  unsaved = { cfi, fraction };
  clearTimeout(saveTimer);
  saveTimer = setTimeout(savePosition, 800);
}

async function savePosition() {
  clearTimeout(saveTimer);
  if (!unsaved || !positionKey) return;
  const position = unsaved;
  unsaved = null;
  try {
    await invoke('save_reader_position', { key: positionKey, position });
  } catch (e) {
    console.error('Could not save the reading position:', e);
  }
}

// ---- Pictures
//
// A picture never sits inline with text. It becomes a block exactly one screen tall, with
// the image fitted inside (never enlarged), and paging stops on it so it shows on a screen
// of its own. Small images, such as ornaments and drop caps, stay inline but never grow
// taller than a screen.

const FIGURE_MIN = 150; // px, both sides, natural size
/** The pictures laid out as screens, per page of the book. */
const figuresOf = new WeakMap();
/** Images whose size is still being read, and when it will be known. */
const waiting = new WeakMap();

// Until then the chapter's layout is not final. Going to its end (or a saved place) before
// that would land somewhere the pictures then push around.
function whenSized(/** @type {Document | undefined} */ doc) {
  if (!doc?.body) return Promise.resolve();
  return Promise.all([...doc.body.querySelectorAll('img')].map((img) => waiting.get(img)).filter(Boolean));
}
/** Natural sizes read ahead of the images themselves loading. */
const sizes = new WeakMap();

/** @param {ElementCSSInlineStyle} el @param {Record<string, string>} styles */
function setImportant(el, styles) {
  for (const [k, v] of Object.entries(styles)) el.style.setProperty(k, v, 'important');
}

const CONTENT = 'img, svg, video, audio, iframe, object, embed, canvas, input, br, hr';

// Two columns need room. Each must hold about 40 to 45 characters, measured in the book's
// text size at the current font step, so larger text needs a wider window before the
// spread opens. Narrower than that, the reader shows one column. The setting itself stays
// as chosen, and two columns come back once there is room again.
const MIN_COLUMN_EM = 20;

function roomForTwo() {
  const doc = currentDoc();
  const text = ((doc && baseSizes.get(doc)) || 16) * FONT_STEP_FACTOR ** settings.font_step;
  // Each page has the left and right padding of its own (see applyLayout), so the two
  // columns share what is left after the padding twice over.
  const columns = area.width - 2 * (inset.left + inset.right);
  return columns >= 2 * MIN_COLUMN_EM * text;
}

/** Whether two columns should show now: chosen, and fitting. */
const wantTwo = () => settings.two_columns && roomForTwo();

/**
 * Whether two columns are showing. This is the layout actually drawn, and the one thing
 * every part of the reader goes by. Room is only judged again when something it depends
 * on changes (see syncLayout), never mid-step, so no part can act on a layout that is not
 * on screen.
 */
let shownTwoUp = $state(false);
const twoUp = () => shownTwoUp;

/** Whether a node shows nothing: whitespace, or an empty inline such as a page number marker. */
function isEmpty(/** @type {Node} */ n) {
  if (n.nodeType !== Node.ELEMENT_NODE) return !n.textContent?.trim();
  const el = /** @type {Element} */ (n);
  return !el.textContent?.trim() && !el.matches(CONTENT) && !el.querySelector(CONTENT)
    && (el.ownerDocument.defaultView?.getComputedStyle(el).display ?? '').startsWith('inline');
}

/** Whether only empty nodes lie between `child` and the start (dir -1) or end (dir 1) of its parent. */
function onlyEmpty(/** @type {Node} */ child, /** @type {number} */ dir) {
  for (let n = dir < 0 ? child.previousSibling : child.nextSibling; n; n = dir < 0 ? n.previousSibling : n.nextSibling) {
    if (!isEmpty(n)) return false;
  }
  return true;
}

// The wrappers a picture starts or ends, such as the <p> in <p><img/></p>, would leave
// their margins around it, and it would never fit a screen (or a page) exactly.
//
// On pages, the break before a picture also moves up to those wrappers, and anything left
// in front of it there, even an empty marker, still keeps a page to itself: a blank page
// before the picture. Those markers are taken out of the flow. They stay in the document
// and keep their place, so links to them still land.
function clearAround(/** @type {Element} */ el) {
  const body = el.ownerDocument.body;
  let top = true, bottom = true;
  for (let child = /** @type {Element} */ (el), parent = el.parentElement; parent && (top || bottom); child = parent, parent = parent.parentElement) {
    top &&= onlyEmpty(child, -1);
    bottom &&= onlyEmpty(child, 1);
    if (top && twoUp()) {
      for (let n = child.previousSibling; n; n = n.previousSibling) {
        if (n.nodeType === Node.ELEMENT_NODE) setImportant(/** @type {HTMLElement} */ (n), { position: 'absolute' });
      }
    }
    if (parent === body) break;
    const p = /** @type {HTMLElement} */ (parent);
    if (top) setImportant(p, { 'margin-top': '0', 'padding-top': '0', 'border-top-width': '0' });
    if (bottom) setImportant(p, { 'margin-bottom': '0', 'padding-bottom': '0', 'border-bottom-width': '0' });
  }
}

/**
 * Lays a picture out as a screen (one column) or a page (two columns) of its own, fitted
 * and never enlarged. `ratio` is its width over its height; `widest` its natural width.
 */
function placePicture(/** @type {HTMLElement | SVGElement} */ el, /** @type {number} */ ratio, /** @type {number} */ widest) {
  clearAround(el);
  const common = {
    display: 'block', float: 'none', 'box-sizing': 'border-box',
    'max-width': '100%', 'max-height': 'none',
    margin: '0 auto', padding: '0', 'object-fit': 'contain',
  };
  const cap = Number.isFinite(widest) ? `${widest}px, ` : '';
  if (twoUp()) {
    // Inside a page's frame 100vh is one page tall. It is rounded to whole pixels and can
    // be a fraction taller than the page, which would push the picture on a page.
    setImportant(el, {
      ...common,
      width: `min(100%, ${cap}calc((100vh - 1px) * ${ratio}))`, height: 'calc(100vh - 1px)',
      'break-before': 'column', 'break-after': 'column',
      visibility: 'visible',
    });
  } else {
    const H = view.renderer.size;
    setImportant(el, {
      ...common,
      width: `min(100%, ${cap}${H * ratio}px)`, height: `${H}px`,
      'break-before': 'auto', 'break-after': 'auto',
      // Hidden until the screen rests on it (see showPictures).
      visibility: 'hidden',
    });
  }
}

/** The tallest an inline image may be: one screen, or one page. */
const inlineMax = () => (twoUp() ? 'calc(100vh - 1px)' : `${view.renderer.size}px`);

// Nothing is measured but the screen: a newly loaded chapter is not laid out yet, and
// every width in it would read as 0.
function layoutFigures(/** @type {Document} */ doc) {
  if (!view?.renderer?.size || !doc.body) return;
  /** @type {Set<Element>} */
  const figures = new Set();
  figuresOf.set(doc, figures);
  for (const img of /** @type {NodeListOf<HTMLImageElement>} */ (doc.body.querySelectorAll('img'))) {
    // Every image loads straight away, so the layout settles as early as it can.
    img.loading = 'eager';
    const size = sizes.get(img) ?? (img.complete && img.naturalWidth ? { nw: img.naturalWidth, nh: img.naturalHeight } : null);
    if (!size) {
      // WebView2 defers images below the screen until they are scrolled near, so their
      // size would arrive only once a sliver of them is already showing. The size is read
      // from the image's data instead, straight away.
      if (!waiting.has(img)) {
        const probe = new Image();
        probe.src = img.currentSrc || img.src;
        waiting.set(img, probe.decode().then(() => {
          sizes.set(img, { nw: probe.naturalWidth, nh: probe.naturalHeight });
          layoutFigures(doc);
          rerender();
        }, () => {}).finally(() => waiting.delete(img)));
      }
      continue;
    }
    const { nw, nh } = size;
    if (nw < FIGURE_MIN || nh < FIGURE_MIN) {
      setImportant(img, { 'max-height': inlineMax() });
      continue;
    }
    placePicture(img, nw / nh, nw);
    figures.add(img);
  }
  for (const svg of doc.body.querySelectorAll('svg')) {
    // calibre's cover pages are an SVG wrapping one image.
    if (svg.querySelector('image')) {
      // calibre often converts with "preserve cover aspect ratio" off, which tells the
      // cover to stretch to whatever box it is given (preserveAspectRatio="none"). Given a
      // whole screen, it would be pulled out of shape. The cover keeps its own shape.
      svg.setAttribute('preserveAspectRatio', 'xMidYMid meet');
      for (const image of svg.querySelectorAll('image')) image.setAttribute('preserveAspectRatio', 'xMidYMid meet');
      const box = /** @type {SVGSVGElement} */ (svg).viewBox?.baseVal;
      const ratio = box?.width && box?.height ? box.width / box.height : 0;
      if (ratio) placePicture(svg, ratio, Infinity);
      else placePicture(svg, 1e6, Infinity);
      figures.add(svg);
    } else setImportant(svg, { 'max-height': inlineMax() });
  }
  // A screen of room after the text, so the last stops can always reach the top of the
  // screen. Pages need none.
  setImportant(doc.body, { 'padding-bottom': twoUp() ? '0' : `${view.renderer.size}px` });
  stopCache.delete(doc);
}

/** Lays the chapter out again (keeping the place), then brings the screen back onto a stop. */
function rerender() {
  if (!view?.renderer) return;
  view.renderer.render();
  settle();
}

function layoutAllFigures() {
  if (!view?.renderer) return;
  applyLayout();
  for (const { doc } of view.renderer.getContents()) layoutFigures(doc);
  rerender();
}

// ---- Layout: one scrolling column, or two pages side by side

/**
 * Tells foliate which layout to draw.
 *
 * One column: the reader's own container provides the padding.
 *
 * Two columns: each page has the padding of its own, like two pages of a book, so the
 * outer edges get the left and right padding and the middle gets both together. foliate
 * leaves the same space at the outer edges as between its pages: a share `g` of its
 * width `W` (gW each). So the gap is set to the left and right padding together, and
 * foliate's area reaches past the reading area by the difference at each edge (the right
 * padding on the left, the left padding on the right; see the .two style), which leaves
 * exactly the left padding at the left edge and the right padding at the right.
 */
function applyLayout() {
  const r = view?.renderer;
  if (!r) return;
  const sides = inset.left + inset.right;
  const W = area.width + sides;
  /** @type {Record<string, string>} */
  const attrs = twoUp() && area.width > 0
    ? (() => {
        const g = sides / W;
        return {
          flow: 'paginated',
          'max-column-count': '2',
          // foliate puts two columns side by side once a column may be no wider than half
          // of its own area, which is W less the outer spaces.
          'max-inline-size': `${Math.ceil((W * (1 - g)) / 2) + 1}px`,
          'max-block-size': '100000px',
          gap: `${(100 * g).toFixed(4)}%`,
        };
      })()
    : { flow: 'scrolled', 'max-column-count': '1', 'max-inline-size': '100000px', gap: '0%' };
  attrs.margin = '0px';
  // Every change makes foliate draw the chapter again, so only real changes are made.
  for (const [k, v] of Object.entries(attrs)) if (r.getAttribute(k) !== v) r.setAttribute(k, v);
  // In a window taller than it is wide, foliate falls back to one column by itself. Two
  // columns were asked for, so they stay. The setting sits on foliate's own layout element,
  // inside its closed shadow root, reached from the chapter's frame.
  const doc = currentDoc();
  let top = doc?.defaultView?.frameElement?.parentElement;
  while (top && top.id !== 'top') top = top.parentElement;
  const portrait = twoUp() ? '2' : '1';
  if (top && top.style.getPropertyValue('--_max-column-count-portrait') !== portrait) {
    top.style.setProperty('--_max-column-count-portrait', portrait);
    r.render();
  }
}

/**
 * Shows one or two columns, whichever fits now. Switching keeps the place: where the
 * reader was is found again in the new layout.
 */
function syncLayout() {
  const two = wantTwo();
  if (!view?.renderer || changingChapter || two === shownTwoUp) return;
  const cfi = view.lastLocation?.cfi;
  shownTwoUp = two;
  resting = null;
  cutoff = 0;
  applyLayout();
  for (const { doc } of view.renderer.getContents()) layoutFigures(doc);
  (cfi ? view.goTo(cfi) : Promise.resolve()).catch(() => {}).then(() => settle());
}

// Room is judged again when the setting, the window, the padding or the font size changes,
// and when a chapter loads (its text size is known only then; see the load handler).
$effect(() => {
  void [settings.two_columns, settings.font_step, area.width, inset.left, inset.right];
  syncLayout();
});

function setLayout(/** @type {boolean} */ two_columns) {
  if (two_columns !== settings.two_columns) saveSettings({ ...settings, two_columns });
}

// Pictures are sized to the screen, so they follow it when the window or the padding changes.
$effect(() => {
  void [area.width, area.height, inset.top, inset.right, inset.bottom, inset.left];
  requestAnimationFrame(layoutAllFigures);
});

// ---- Movement
//
// The reader moves the text itself, in steps. A chapter has a list of stops: places the
// top of the screen can rest. Text stops are the tops of its lines, so the screen always
// starts on a whole line. A picture is a stop of its own, filling the screen, and it is
// only drawn while the reader rests on it, so it is never seen beside text.
//
// One wheel notch moves to the next or previous stop: one line of text, or a turn to or
// from a picture. Lines stop where the last line before a picture reaches the bottom of
// the screen; the next notch shows the picture. Space and the arrows move a screen of
// lines, by the same stops. Past the last stop of a chapter is the first of the next.
//
// The chapter's frame is never scrolled itself (the container around it is), so positions
// inside its document are positions in the chapter.

/** Each chapter's stops, and the layout they were worked out for. */
const stopCache = new WeakMap();

const currentDoc = () => /** @type {Document | undefined} */ (view?.renderer?.getContents()[0]?.doc);

/** The element that scrolls the chapter, inside foliate's (closed) shadow root. */
function scrollerOf(/** @type {Document} */ doc) {
  for (let el = doc.defaultView?.frameElement?.parentElement; el; el = el.parentElement) {
    const overflow = el.ownerDocument.defaultView?.getComputedStyle(el).overflowY;
    if (overflow === 'auto' || overflow === 'scroll') return /** @type {HTMLElement} */ (el);
  }
  return null;
}

/**
 * @typedef {{ at: number, picture?: Element, tail?: boolean }} Stop
 * @typedef {{ top: number, bottom: number }} Span
 */

function stopsOf(/** @type {Document} */ doc) {
  const H = view.renderer.size;
  const key = `${H}|${doc.body.getBoundingClientRect().height}`;
  const cached = stopCache.get(doc);
  if (cached?.key === key) return cached;

  // Rects from the chapter's frame belong to its window, so `instanceof DOMRect` would fail.
  const span = (/** @type {any} */ b) => ({ top: b.top, bottom: b.bottom });
  /** @type {Set<Element>} */
  const figureSet = figuresOf.get(doc) ?? new Set();
  const pictures = [...figureSet]
    .map((el) => ({ el, ...span(el.getBoundingClientRect()) }))
    .sort((a, b) => a.top - b.top);

  // Every line of text and every inline image. Pieces on the same line (a word in italics,
  // a footnote mark) are merged into one line.
  /** @type {Span[]} */
  const pieces = [];
  const walker = doc.createTreeWalker(doc.body, NodeFilter.SHOW_TEXT);
  const range = doc.createRange();
  for (let node; (node = walker.nextNode());) {
    if (!node.textContent?.trim()) continue;
    range.selectNodeContents(node);
    for (const b of range.getClientRects()) if (b.height > 0) pieces.push(span(b));
  }
  for (const el of doc.body.querySelectorAll('img, svg, video, hr')) {
    if (figureSet.has(el)) continue;
    const b = el.getBoundingClientRect();
    if (b.height > 0) pieces.push(span(b));
  }
  pieces.sort((a, b) => a.top - b.top);
  // A piece's box is its font's full height, which is often taller than the line spacing
  // a book sets, so neighbouring lines can overlap. Pieces are one line only when they
  // mostly overlap, by over half the smaller one.
  /** @type {Span[]} */
  const lines = [];
  for (const p of pieces) {
    const last = lines.at(-1);
    const shared = last ? Math.min(last.bottom, p.bottom) - Math.max(last.top, p.top) : 0;
    if (last && shared > 0.5 * Math.min(last.bottom - last.top, p.bottom - p.top)) {
      last.top = Math.min(last.top, p.top);
      last.bottom = Math.max(last.bottom, p.bottom);
    } else lines.push({ ...p });
  }
  // Where two lines overlap, the edge between them is the middle of the overlap. That is
  // where the browser's own line box starts, so stops and the bottom cover land on it.
  for (let i = 1; i < lines.length; i++) {
    const prev = lines[i - 1], line = lines[i];
    if (line.top < prev.bottom) {
      const edge = (line.top + prev.bottom) / 2;
      prev.bottom = edge;
      line.top = edge;
    }
  }

  // The text between pictures, one run at a time. A run's stops are its line tops, up to
  // where its last line reaches the bottom of the screen. A run shorter than a screen has
  // a single stop, with the rest of the screen left empty.
  /** @type {Stop[]} */
  const stops = [];
  const textStops = (/** @type {number} */ from, /** @type {number} */ to, /** @type {boolean} */ first, beforePicture = false) => {
    const inRun = lines.filter((l) => l.top >= from - 0.5 && l.bottom <= to + 0.5);
    if (!inRun.length) return;
    const start = first ? 0 : inRun[0].top;
    // The last stop shows the run's last line in full, starting on a whole line itself.
    const lastBottom = /** @type {Span} */ (inRun.at(-1)).bottom;
    const last = Math.max(start, inRun.find((l) => l.top >= lastBottom - H - 0.5)?.top ?? start);
    /** @type {number[]} */
    const run = [start];
    for (const l of inRun) if (l.top > start + 0.5 && l.top < last - 0.5) run.push(l.top);
    if (last > start + 0.5) run.push(last);
    // Something taller than the screen, such as a large table: step through it a screen at a time.
    for (let i = 0; i < run.length; i++) {
      stops.push({ at: run[i] });
      const gap = (run[i + 1] ?? run[i]) - run[i];
      for (let k = 1; k * (H - 48) < gap; k++) stops.push({ at: run[i] + k * (H - 48) });
    }
    // Before a picture, the lines after the last stop are stops too, but only for a screen
    // at a time (Space and the arrows): the first line not yet fully seen goes to the top,
    // with the rest of the screen empty, rather than a short step that shows the same text
    // again. Line by line, the wheel still stops at the last stop and turns to the picture.
    if (beforePicture) for (const l of inRun) if (l.top > last + 0.5) stops.push({ at: l.top, tail: true });
  };
  let from = 0;
  for (const p of pictures) {
    textStops(from, p.top, from === 0, true);
    stops.push({ at: p.top, picture: p.el });
    from = p.bottom;
  }
  textStops(from, Infinity, from === 0);
  if (!stops.length) stops.push({ at: 0 });

  const result = { key, stops, lines };
  stopCache.set(doc, result);
  return result;
}

/** The stop the screen rests on (or has just passed), as an index. */
function stopIndex(/** @type {Stop[]} */ stops, /** @type {number} */ s) {
  let i = 0;
  while (i + 1 < stops.length && stops[i + 1].at <= s + 0.5) i++;
  return i;
}

/** A picture is drawn only while the screen rests on it. */
function showPictures(/** @type {Document} */ doc, /** @type {number} */ s) {
  for (const el of figuresOf.get(doc) ?? []) {
    const top = el.getBoundingClientRect().top;
    setImportant(/** @type {HTMLElement} */ (el), { visibility: Math.abs(top - s) < 1 ? 'visible' : 'hidden' });
  }
}

/**
 * The screen height is rarely a whole number of lines, so the bottom line is often cut.
 * That part of the screen is covered, leaving a clean edge; the line shows in full on the
 * next step. Anything taller than half a screen (a large table) is left as it is.
 */
function coverCutLine(/** @type {Document} */ doc, /** @type {number} */ s) {
  const H = view.renderer.size;
  const bottom = s + H;
  const line = stopsOf(doc).lines.find((/** @type {Span} */ l) => l.bottom > bottom + 0.5);
  cutoff = line && line.top < bottom - 0.5 && line.bottom - line.top < H / 2 ? bottom - line.top : 0;
}

// ---- Resting place
//
// Where the reader rests is remembered as a spot in the text (the character at the top of
// the screen) or a picture, never as a pixel offset. Whenever the layout changes (a late
// image or font, a resize, a font size step) or anything else scrolls the chapter (foliate
// returning to a place it remembers from before), the reader finds that spot again and
// settles back onto its line. Nothing about the font, the window or the timing of loads can
// leave the screen between stops.

/** @type {{ doc: Document, start?: boolean, picture?: Element, node?: Node, offset?: number } | null} */
let resting = null;
/** The scroll offset the reader itself last set, to tell its own scrolling from anyone else's. */
let written = -1;

function remember(/** @type {Document} */ doc, /** @type {Stop} */ stop) {
  if (stop.picture) resting = { doc, picture: stop.picture };
  else if (stop.at < 0.5) resting = { doc, start: true };
  else {
    const x = doc.body.getBoundingClientRect().left + 4;
    const caret = doc.caretRangeFromPoint?.(x, stop.at + 2);
    resting = caret ? { doc, node: caret.startContainer, offset: caret.startOffset } : null;
  }
}

/** Where the remembered spot is now, or null when there is none for this chapter. */
function restingAt(/** @type {Document} */ doc) {
  if (!resting || resting.doc !== doc) return null;
  if (resting.start) return 0;
  if (resting.picture) return resting.picture.isConnected ? resting.picture.getBoundingClientRect().top : null;
  const node = resting.node;
  if (!node?.isConnected) return null;
  const range = doc.createRange();
  const length = node.nodeType === Node.TEXT_NODE ? /** @type {Text} */ (node).length : node.childNodes.length;
  const at = Math.min(resting.offset ?? 0, length);
  range.setStart(node, at);
  range.setEnd(node, Math.min(at + 1, length));
  const rect = range.getClientRects()[0] ?? range.getBoundingClientRect();
  return rect.height > 0 ? rect.top : null;
}

/** Puts the screen on a stop. Every move is instant: a line, a screen, or a picture. */
function moveTo(/** @type {Document} */ doc, /** @type {Stop} */ stop) {
  const el = scrollerOf(doc);
  if (!el) return;
  showPictures(doc, stop.at);
  el.scrollTop = stop.at;
  written = el.scrollTop;
  coverCutLine(doc, stop.at);
  remember(doc, stop);
}

/** Brings the screen back onto a stop: the remembered spot's, or the chapter's last. */
function settle(/** @type {{ toEnd?: boolean }} */ { toEnd = false } = {}) {
  // Pages have no stops: foliate keeps them aligned itself.
  if (twoUp()) return;
  const doc = currentDoc();
  const el = doc && scrollerOf(doc);
  if (!doc?.body || !el) return;
  const { stops } = stopsOf(doc);
  const y = restingAt(doc) ?? el.scrollTop;
  const stop = toEnd ? stops.at(-1) : stops[stopIndex(stops, y)];
  if (stop) moveTo(doc, stop);
}

let settleFrame = 0;

/** Settles on the next frame, once whatever is changing the layout has finished. A chapter
 * change settles by itself when it is done. */
function settleSoon() {
  if (settleFrame) return;
  settleFrame = requestAnimationFrame(() => {
    settleFrame = 0;
    if (!changingChapter) settle();
  });
}

/** Scrolling the reader did not do itself is undone, back onto the remembered stop. */
function onScrollGuard(/** @type {Event} */ e) {
  if (twoUp()) return;
  const el = /** @type {HTMLElement} */ (e.currentTarget);
  if (changingChapter || Math.abs(el.scrollTop - written) < 1.5) return;
  settleSoon();
}

const guarded = new WeakSet();

/** Watches a newly loaded chapter: its size, and the element that scrolls it. */
function watchChapter(/** @type {Document} */ doc) {
  new ResizeObserver(() => {
    stopCache.delete(doc);
    settleSoon();
  }).observe(doc.body);
  const el = scrollerOf(doc);
  if (el && !guarded.has(el)) {
    guarded.add(el);
    el.addEventListener('scroll', onScrollGuard, { passive: true });
  }
}

let changingChapter = false;

/** Moves to the next (1) or previous (-1) chapter. At either end of the book, nothing happens. */
async function changeChapter(/** @type {number} */ dir) {
  const r = view.renderer;
  const sections = view.book.sections;
  const current = r.getContents()[0]?.index;
  if (current == null || changingChapter) return;
  let index = current + dir;
  while (index >= 0 && index < sections.length && sections[index].linear === 'no') index += dir;
  if (index < 0 || index >= sections.length) return;
  changingChapter = true;
  resting = null;
  try {
    await r.goTo({ index, anchor: dir > 0 ? 0 : 1 });
    await whenSized(currentDoc());
    settle({ toEnd: dir < 0 });
  } finally {
    changingChapter = false;
  }
  // The new chapter's text size may change whether two columns fit; that was held off
  // while the chapter changed.
  syncLayout();
}

/** One stop on: a line, or a turn to or from a picture. */
function stepLine(/** @type {number} */ dir) {
  if (twoUp()) return turnSpread(dir);
  const doc = currentDoc();
  const el = doc && scrollerOf(doc);
  if (!doc?.body || !el || changingChapter) return;
  // Line by line, the stops past a run's last stop before a picture are passed over.
  const stops = stopsOf(doc).stops.filter((/** @type {Stop} */ st) => !st.tail);
  const s = el.scrollTop;
  const i = stopIndex(stops, s);
  if (dir > 0) {
    if (i + 1 < stops.length) moveTo(doc, stops[i + 1]);
    else changeChapter(1);
  } else if (stops[i].at < s - 0.5) moveTo(doc, stops[i]);
  else if (i > 0) moveTo(doc, stops[i - 1]);
  else changeChapter(-1);
}

/** One screen of lines on, never passing over a picture without stopping on it. */
function stepScreen(/** @type {number} */ dir) {
  if (twoUp()) return turnSpread(dir);
  const doc = currentDoc();
  const el = doc && scrollerOf(doc);
  if (!doc?.body || !el || changingChapter) return;
  const H = view.renderer.size;
  /** @type {{ stops: Stop[], lines: Span[] }} */
  const { stops, lines } = stopsOf(doc);
  const s = el.scrollTop;
  if (dir > 0) {
    const ahead = stops.filter((st) => st.at > s + 0.5);
    if (!ahead.length) return changeChapter(1);
    // The first line not fully on screen goes to the top.
    const cut = lines.find((l) => l.bottom > s + H + 0.5);
    const target = (cut ? ahead.findLast((st) => st.at <= cut.top + 0.5) : ahead.at(-1)) ?? ahead[0];
    const picture = ahead.find((st) => st.picture && st.at <= target.at);
    moveTo(doc, picture ?? target);
  } else {
    const behind = stops.filter((st) => st.at < s - 0.5);
    if (!behind.length) return changeChapter(-1);
    const target = behind.find((st) => st.at >= s - H - 0.5) ?? /** @type {Stop} */ (behind.at(-1));
    const picture = behind.findLast((st) => st.picture && st.at >= target.at);
    moveTo(doc, picture ?? target);
  }
}

// ---- Keys

async function toggleFullscreen() {
  const next = !fullscreen;
  try {
    await appWindow.setFullscreen(next);
    fullscreen = next;
  } catch (e) {
    console.error('Could not change full screen:', e);
  }
}

async function togglePanel() {
  panelOpen = !panelOpen;
  if (!panelOpen) {
    await tick();
    readingEl?.focus();
  }
}

function onKey(/** @type {KeyboardEvent} */ e) {
  if (e.altKey && !e.ctrlKey && e.code === 'KeyS') {
    e.preventDefault();
    togglePanel();
    return;
  }
  if (e.key === 'F11') {
    e.preventDefault();
    toggleFullscreen();
    return;
  }
  if (e.key === 'Escape' && pickerOpen) {
    e.preventDefault();
    closePicker();
    return;
  }
  if (e.key === 'Escape' && panelOpen) {
    e.preventDefault();
    togglePanel();
    return;
  }
  if (e.ctrlKey && !e.altKey) {
    // The webview's own zoom never fires. Font size changes the text instead.
    if (e.code === 'Equal' || e.code === 'NumpadAdd' || e.key === '+' || e.key === '=') {
      e.preventDefault();
      stepFont(1);
    } else if (e.code === 'Minus' || e.code === 'NumpadSubtract' || e.key === '-') {
      e.preventDefault();
      stepFont(-1);
    } else if (e.code === 'Digit0' || e.code === 'Numpad0') {
      e.preventDefault();
    } else if (e.code === 'KeyO') {
      e.preventDefault();
      openPicker();
    } else if (e.code === 'Comma' || e.key === ',') {
      // The same as Alt+S.
      e.preventDefault();
      togglePanel();
    }
    return;
  }
  // While the panel or the picker is open, it owns the keyboard.
  if (!view || panelOpen || pickerOpen || e.altKey || e.metaKey) return;
  /** @type {Record<string, () => void>} */
  const moves = {
    ' ': () => stepScreen(e.shiftKey ? -1 : 1),
    ArrowRight: () => stepScreen(1),
    PageDown: () => stepScreen(1),
    ArrowLeft: () => stepScreen(-1),
    PageUp: () => stepScreen(-1),
    ArrowDown: () => stepLine(1),
    ArrowUp: () => stepLine(-1),
  };
  const move = moves[e.key];
  if (move) {
    e.preventDefault();
    move();
  }
}

// A mouse wheel notch is one line. A touchpad's small movements add up, a line's worth of
// travel at a time.
const WHEEL_NOTCH = 50; // px: a single event this large is a mouse wheel notch
const TOUCHPAD_LINE = 30; // px of touchpad travel per line
let wheelTravel = 0;

function onWheel(/** @type {WheelEvent} */ e) {
  // The picker's list scrolls as usual.
  if (pickerOpen && !e.ctrlKey) return;
  e.preventDefault();
  if (e.ctrlKey || !view) return;
  const delta = e.deltaMode === 1 ? e.deltaY * 40 : e.deltaY;
  if (!delta) return;
  if (twoUp()) return wheelSpread(delta);
  if (Math.abs(delta) >= WHEEL_NOTCH) {
    wheelTravel = 0;
    stepLine(Math.sign(delta));
    return;
  }
  if (Math.sign(delta) !== Math.sign(wheelTravel)) wheelTravel = 0;
  wheelTravel += delta;
  if (Math.abs(wheelTravel) >= TOUCHPAD_LINE) {
    stepLine(Math.sign(wheelTravel));
    wheelTravel = 0;
  }
}

// With two columns, a wheel notch turns a spread. A touchpad gesture turns one spread,
// however long its momentum runs: the gesture ends once the touchpad has been quiet briefly.
const TOUCHPAD_SPREAD = 60; // px of touchpad travel per spread
const GESTURE_QUIET = 160; // ms
let gestureTurned = false;
/** @type {ReturnType<typeof setTimeout> | undefined} */
let gestureEnd;

function wheelSpread(/** @type {number} */ delta) {
  clearTimeout(gestureEnd);
  gestureEnd = setTimeout(() => {
    wheelTravel = 0;
    gestureTurned = false;
  }, GESTURE_QUIET);
  if (Math.abs(delta) >= WHEEL_NOTCH) return turnSpread(Math.sign(delta));
  if (gestureTurned) return;
  if (Math.sign(delta) !== Math.sign(wheelTravel)) wheelTravel = 0;
  wheelTravel += delta;
  if (Math.abs(wheelTravel) < TOUCHPAD_SPREAD) return;
  gestureTurned = true;
  turnSpread(Math.sign(wheelTravel));
}

/** The next (1) or previous (-1) spread. foliate carries on into the next chapter itself. */
function turnSpread(/** @type {number} */ dir) {
  const r = view?.renderer;
  // At either end of the book, nothing: foliate cannot go on from there.
  if (!r || (dir > 0 ? r.atEnd : r.atStart)) return;
  return dir > 0 ? r.next() : r.prev();
}

// ---- Opening another book (Ctrl+O)

function openPicker() {
  if (panelOpen) panelOpen = false;
  pickerError = '';
  pickerOpen = true;
}

async function closePicker() {
  pickerOpen = false;
  await tick();
  readingEl?.focus();
}

/**
 * Replaces the book in this window, never opening a new one. The place in the current book
 * is saved first. Rust checks the new file opens before anything is let go, so a file that
 * does not open leaves the current book as it was.
 */
async function replaceBook(/** @type {{ bookId?: number, path?: string }} */ choice) {
  pickerError = '';
  try {
    await invoke('reader_replace', { bookId: choice.bookId ?? null, path: choice.path ?? null });
  } catch (e) {
    pickerError = String(e);
    return;
  }
  await savePosition();
  pickerOpen = false;
  closeView();
  await openBook();
  readingEl?.focus();
}

async function browseDisk() {
  const path = await openFileDialog({ multiple: false, directory: false, filters: [{ name: 'EPUB', extensions: ['epub'] }] });
  if (typeof path === 'string') replaceBook({ path });
}

/** Lets go of the book being shown, ready for another. */
function closeView() {
  view?.close();
  view?.remove();
  view = undefined;
  ready = false;
  positionKey = '';
  resting = null;
  percent = null;
  cutoff = 0;
  title = '';
}

// ---- Opening

async function openBook() {
  /** @type {any} */
  let book;
  /** @type {any} */
  let opened;
  try {
    opened = await invoke('reader_load');
    book = await new EPUB(makeLoader(opened.entries)).init();
  } catch (e) {
    console.error('Could not open the book:', e);
    appWindow.destroy();
    return;
  }
  title = titleOf(book.metadata) || opened.file_name;
  appWindow.setTitle(title).catch(() => {});
  positionKey = keyFor(book, opened.path);

  book.transformTarget?.addEventListener('load', (/** @type {any} */ e) => {
    if (e.detail.isScript) e.detail.allow = false;
  });
  book.transformTarget?.addEventListener('data', (/** @type {any} */ e) => {
    if (PAGE_TYPES.includes(e.detail.type)) e.detail.data = Promise.resolve(e.detail.data).then(blockScripts);
  });

  const [saved] = await Promise.all([
    invoke('reader_position', { key: positionKey }).catch(() => null),
    invoke('reader_settings').then((s) => (settings = /** @type {any} */ (s))).catch(() => {}),
  ]);
  appliedStep = settings.font_step;

  view = document.createElement('foliate-view');
  view.addEventListener('load', (/** @type {any} */ e) => {
    const doc = e.detail.doc;
    applyFont(doc);
    layoutFigures(doc);
    // The book's own fonts can arrive later and move every line.
    doc.fonts?.ready.then(() => {
      layoutFigures(doc);
      rerender();
    });
    doc.addEventListener('keydown', onKey);
    doc.addEventListener('wheel', onWheel, { passive: false });
    watchChapter(doc);
    // This chapter's text size is known now, which can change whether two columns fit.
    requestAnimationFrame(syncLayout);
  });
  view.addEventListener('relocate', (/** @type {any} */ e) => {
    const { fraction, cfi, reason } = e.detail;
    // A link followed inside the book is a new place to read from, not a scroll to undo.
    if (reason === 'navigation' && !changingChapter) resting = null;
    if (typeof fraction === 'number') percent = Math.round(fraction * 100);
    if (ready && cfi) savePositionSoon(cfi, fraction ?? 0);
  });
  readingEl.append(view);
  await view.open(book);
  shownTwoUp = wantTwo();
  applyLayout();
  try {
    await view.init({ lastLocation: /** @type {any} */ (saved)?.cfi, showTextStart: false });
  } catch (e) {
    console.error('Could not restore the reading position:', e);
    await view.init({ showTextStart: false });
  }
  await whenSized(currentDoc());
  // Now that a chapter is showing, the parts of the layout set from inside foliate can be set too.
  applyLayout();
  settle();
  ready = true;
}

onMount(() => {
  window.addEventListener('keydown', onKey);
  window.addEventListener('wheel', onWheel, { passive: false });

  const observer = new ResizeObserver(([entry]) => {
    const box = entry.borderBoxSize[0];
    area = { width: box.inlineSize, height: box.blockSize };
  });
  observer.observe(readingEl);

  /** @type {(() => void)[]} */
  const unlisten = [];
  listen('reader-settings', (/** @type {any} */ e) => {
    if (e.payload.from !== appWindow.label) settings = e.payload.settings;
  }).then((fn) => unlisten.push(fn));
  // The window closes only once the position is written.
  appWindow.onCloseRequested(() => savePosition()).then((fn) => unlisten.push(fn));

  openBook();
  return () => {
    window.removeEventListener('keydown', onKey);
    window.removeEventListener('wheel', onWheel);
    observer.disconnect();
    unlisten.forEach((fn) => fn());
    view?.close();
  };
});
</script>

<div class="frame">
  {#if !fullscreen}<TopBar {title} />{/if}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    class="reading"
    class:fullscreen
    tabindex="-1"
    bind:this={readingEl}
    class:two={shownTwoUp}
    style:padding="{inset.top}px {shownTwoUp ? 0 : inset.right}px {inset.bottom}px {shownTwoUp ? 0 : inset.left}px"
    style:--pull-left="{inset.right}px"
    style:--pull-right="{inset.left}px"
  >
    {#if pickerOpen}
      <ReaderPicker
        error={pickerError}
        onPick={(/** @type {{ id: number }} */ book) => replaceBook({ bookId: book.id })}
        onBrowse={browseDisk}
        onClose={closePicker}
      />
    {/if}
    {#if cutoff > 0}
      <div class="cutoff" style:left="{inset.left}px" style:right="{inset.right}px" style:bottom="{inset.bottom}px" style:height="{cutoff}px"></div>
    {/if}
    {#if percent != null}
      <div class="percent" style:left="{inset.left}px" style:height="{inset.bottom}px">{percent}%</div>
    {/if}
  </div>
  <!-- Opens and closes by growing and shrinking, the same glide as the edit dialog's side panels. -->
  <div class="panel-slot" style:height="{panelOpen ? panelHeight : 0}px" inert={!panelOpen}>
    <div bind:clientHeight={panelHeight}>
      <ReaderSettings open={panelOpen} padding={settings.padding} twoColumns={settings.two_columns} onLayout={setLayout} onChange={(/** @type {any} */ padding) => saveSettings({ ...settings, padding })} />
    </div>
  </div>
</div>

<style>
.frame {
  width: 100vw; height: 100vh; box-sizing: border-box;
  background: var(--zen-bg);
  display: flex; flex-direction: column; overflow: hidden;
}
.reading {
  position: relative;
  flex: 1; min-height: 0;
  box-sizing: border-box;
  background: var(--zen-panel);
  border-top: 1px solid var(--zen-border);
  outline: none;
}
.reading.fullscreen { border-top: none; }
/* Two columns: foliate leaves the same space at the outer edges as between its pages, so its
   area reaches past the edges by the difference (see applyLayout). */
.reading.two :global(foliate-view) {
  width: auto;
  margin: 0 calc(-1 * var(--pull-right)) 0 calc(-1 * var(--pull-left));
}
.reading :global(foliate-view) {
  display: block; width: 100%; height: 100%;
}
.panel-slot {
  flex: none;
  overflow: hidden;
  transition: height 70ms ease;
}
.cutoff {
  position: absolute; z-index: 1;
  background: var(--zen-panel);
  pointer-events: none;
}
.percent {
  position: absolute; bottom: 0;
  display: flex; align-items: center;
  font-family: var(--zen-font-sans); font-size: 11px;
  color: var(--zen-text-faint);
  user-select: none; pointer-events: none;
}
</style>
