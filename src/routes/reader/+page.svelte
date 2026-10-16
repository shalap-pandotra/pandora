<script>
import '../../lib/theme.css';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { onMount, tick } from 'svelte';
import TopBar from '../../lib/TopBar.svelte';
import ReaderSettings from '../../lib/ReaderSettings.svelte';
import ReaderPicker from '../../lib/ReaderPicker.svelte';
import LinkPreview from '../../lib/LinkPreview.svelte';
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
let bookUuid = '';
let percent = $state(/** @type {number | null} */ (null));
/** The book fraction foliate last reported. It is saved beside the place, for information only. */
let lastFraction = 0;
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
let backSpot = $state(/** @type {string | null} */ (null));
let holdSaves = false;
let outside = $state(/** @type {{ href: string, host: string, mail: boolean } | null} */ (null));
let notice = $state('');
/** @type {ReturnType<typeof setTimeout> | undefined} */
let noticeTimer;
let links = $state(/** @type {any} */ (null));

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

// A book can colour its own selection, with !important, a more specific selector, or by also setting
// the selected text's colour, and would then win over the app's pale pink. Its selection rules are
// taken out of its styles before the page is built, so the app's colour is the only one. A rule that
// also styles other things (a, ::selection { ... }) keeps those other selectors.
const SELECTION_RULE = /([^{}]*::(?:-moz-)?selection[^{}]*)\{([^{}]*)\}/gi;

function withoutSelectionRules(/** @type {string} */ css) {
  return css.replace(SELECTION_RULE, (_all, selectors, body) => {
    const kept = selectors.split(',').filter((/** @type {string} */ s) => !/::(?:-moz-)?selection/i.test(s));
    return kept.length ? `${kept.join(',')}{${body}}` : '';
  });
}

/** A stylesheet, or the style blocks inside a page. */
function dropSelectionStyles(/** @type {any} */ data, /** @type {string} */ type) {
  if (typeof data !== 'string') return data;
  if (type === 'text/css') return withoutSelectionRules(data);
  return data.replace(/(<style\b[^>]*>)([\s\S]*?)(<\/style>)/gi, (_m, open, css, close) => open + withoutSelectionRules(css) + close);
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

const baseSizes = new WeakMap();

// Many books give their text a fixed size: small, medium, large, 12pt, 16px. A fixed size ignores the
// root size that Ctrl + and Ctrl - change, so such a book would not respond. Each fixed font size in
// a page's styles is rewritten once as the same size in rem, which follows the root.
const KEYWORD_PX = { 'xx-small': 9, 'x-small': 10, small: 13, medium: 16, large: 18, 'x-large': 24, 'xx-large': 32, 'xxx-large': 48 };
const UNIT_PX = { px: 1, pt: 4 / 3, pc: 16, in: 96, cm: 96 / 2.54, mm: 96 / 25.4, q: 96 / 101.6 };
const sizesRelative = new WeakSet();

function remFor(/** @type {string} */ value, /** @type {number} */ base) {
  const v = value.trim().toLowerCase();
  const unit = /^(\d*\.?\d+)(px|pt|pc|in|cm|mm|q)$/.exec(v);
  const px = v in KEYWORD_PX ? KEYWORD_PX[v] : unit ? parseFloat(unit[1]) * UNIT_PX[unit[2]] : null;
  return px === null ? null : `${+(px / base).toFixed(4)}rem`;
}

function makeSizesRelative(/** @type {Document} */ doc, /** @type {number} */ base) {
  if (sizesRelative.has(doc)) return;
  sizesRelative.add(doc);
  const fix = (/** @type {CSSStyleDeclaration} */ style) => {
    const rem = remFor(style.getPropertyValue('font-size'), base);
    if (rem) style.setProperty('font-size', rem, style.getPropertyPriority('font-size'));
  };
  const walk = (/** @type {CSSRuleList} */ rules) => {
    for (const rule of /** @type {any[]} */ ([...rules])) {
      if (rule.style) fix(rule.style);
      if (rule.cssRules) walk(rule.cssRules);
      try { if (rule.styleSheet) walk(rule.styleSheet.cssRules); } catch {}
    }
  };
  for (const sheet of [...doc.styleSheets]) {
    try { walk(sheet.cssRules); } catch {}
  }
  for (const el of /** @type {NodeListOf<HTMLElement>} */ (doc.querySelectorAll('[style*="font-size"]'))) fix(el.style);
}

function applyFont(/** @type {Document} */ doc) {
  const root = doc.documentElement;
  if (!root || !doc.defaultView) return;
  if (!baseSizes.has(doc)) baseSizes.set(doc, parseFloat(doc.defaultView.getComputedStyle(root).fontSize) || 16);
  makeSizesRelative(doc, baseSizes.get(doc));
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

/** @type {ReturnType<typeof setTimeout> | undefined} */
let settingsTimer;
let settingsSeq = 0;

/**
 * Applies new settings here at once and saves them. A burst of changes, like typing in a padding
 * box, is saved once when it pauses (pass a wait), so it is not written to disk on every key.
 */
function saveSettings(/** @type {typeof settings} */ next, wait = 0) {
  settings = next;
  settingsSeq++;
  clearTimeout(settingsTimer);
  if (wait) settingsTimer = setTimeout(flushSettings, wait);
    else flushSettings();
}

function flushSettings() {
  clearTimeout(settingsTimer);
  settingsTimer = undefined;
  const seq = settingsSeq;
  return invoke('set_reader_settings', { settings: $state.snapshot(settings) })
    .then((stored) => {
      // Only the reply to the newest change may replace what is shown here.
      if (seq === settingsSeq && JSON.stringify(stored) !== JSON.stringify(settings)) settings = /** @type {any} */ (stored);
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
  // The book's own fonts count too: they reflow every line once they arrive.
  const pending = [...doc.body.querySelectorAll('img')].map((img) => waiting.get(img));
  const all = Promise.all([doc.fonts?.ready, ...pending].filter(Boolean));
  /** @type {ReturnType<typeof setTimeout> | undefined} */
  let timer;
  const limit = new Promise((done) => (timer = setTimeout(done, 4000)));
  return Promise.race([all, limit]).finally(() => clearTimeout(timer));
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
  pinnedEnd = null;
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
  textStops(from, Infinity, from === 0, true);
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
/** The chapter the reader was sent to the end of. Until the reader moves on its own, settling
 * keeps it on the last stop, even if a late font or picture makes the chapter longer. */
/** @type {Document | null} */
let pinnedEnd = null;
/** For each chapter left by going on: how many stops before its last one the reader left it
 * from. Coming back lands on that same screen. */
/** @type {Map<number, number>} */
const exits = new Map();
/** The pin's resting stop, counted back from the last one. Null is the last stop with whole text. */
/** @type {number | null} */
let endBack = null;

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

/**
 * The place the reader rests on, as a position foliate can restore: the first character of the
 * top line, or the picture. Foliate's own idea of the place depends on exactly where the top
 * edge falls, so a book would creep down a line each time it was closed and opened.
 */
function restingCfi() {
  const doc = resting?.doc;
  const index = view?.renderer?.getContents()[0]?.index;
  if (!resting || !doc || doc !== currentDoc() || index == null) return null;
  try {
    const range = doc.createRange();
    if (resting.start) range.setStart(doc.body, 0);
      else if (resting.picture) range.setStart(resting.picture, 0);
        else if (resting.node?.isConnected) range.setStart(resting.node, resting.offset ?? 0);
          else return null;
    range.collapse(true);
    return view.getCFI(index, range);
  } catch {
    return null;
  }
}

/** Puts the screen on a stop. Every move is instant: a line, a screen, or a picture. */
function moveTo(/** @type {Document} */ doc, /** @type {Stop} */ stop, /** @type {{ settling?: boolean }} */ { settling = false } = {}) {
  const el = scrollerOf(doc);
  if (!el) return;
  if (!settling) {
    pinnedEnd = null;
    holdSaves = false;
  }
  showPictures(doc, stop.at);
  el.scrollTop = stop.at;
  written = el.scrollTop;
  coverCutLine(doc, stop.at);
  remember(doc, stop);
  if (ready && !holdSaves) {
    const own = restingCfi();
    if (own) savePositionSoon(own, lastFraction);
  }
}

/** Brings the screen back onto a stop: the remembered spot's, or the chapter's last. */
function settle(/** @type {{ toEnd?: boolean }} */ { toEnd = false } = {}) {
  // Pages have no stops: foliate keeps them aligned itself.
  if (twoUp()) return;
  // While a book is still opening, foliate's own anchoring keeps the saved place as the layout
  // grows. Settling on a half-built layout would pick the wrong text to rest on.
  if (!ready) return;
  const doc = currentDoc();
  const el = doc && scrollerOf(doc);
  if (!doc?.body || !el) return;
  const { stops } = stopsOf(doc);
  if (toEnd) pinnedEnd = doc;
  const atEnd = pinnedEnd === doc;
  const remembered = restingAt(doc);
  const y = remembered ?? el.scrollTop;
  const rest = endBack == null ? stops.findLast((st) => !st.tail) : stops[Math.max(0, stops.length - 1 - endBack)];
  // With nothing remembered (a book just opened), the stop nearest to where foliate left the
  // screen is the one it meant, whatever the line spacing.
  const nearest = stops.reduce((best, st) => (Math.abs(st.at - y) < Math.abs(best.at - y) ? st : best), stops[0]);
  const lastStop = stops.findLast((/** @type {Stop} */ st) => !st.tail);
  const pastEnd = remembered != null && !!lastStop && y > lastStop.at + 0.5;
  const stop = atEnd ? rest : pastEnd ? nearestLine(doc, y) : remembered == null ? nearest : stops[stopIndex(stops, y)];
  if (stop) moveTo(doc, stop, { settling: true });
}

let settleFrame = 0;

function nearestLine(/** @type {Document} */ doc, /** @type {number} */ y) {
  const { lines } = stopsOf(doc);
  const line = lines.reduce((/** @type {any} */ best, /** @type {any} */ l) => (!best || Math.abs(l.top - y) < Math.abs(best.top - y) ? l : best), undefined);
  return line ? { at: line.top } : undefined;
}

function land() {
  const doc = currentDoc();
  const el = doc && scrollerOf(doc);
  if (!doc?.body || !el) return;
  resting = null;
  pinnedEnd = null;
  const y = el.scrollTop;
  const picture = stopsOf(doc).stops.find((/** @type {Stop} */ st) => st.picture && Math.abs(st.at - y) < 12);
  const stop = picture ?? nearestLine(doc, y);
  if (stop) moveTo(doc, stop, { settling: true });
    else settle();
}

function hereCfi() {
  return twoUp() ? (view?.lastLocation?.cfi ?? null) : restingCfi();
}

function say(/** @type {string} */ message) {
  notice = message;
  clearTimeout(noticeTimer);
  noticeTimer = setTimeout(() => (notice = ''), 2500);
}

async function followLink(/** @type {string} */ href) {
  if (!view || changingChapter) return;
  links?.hide();
  const from = hereCfi();
  const held = holdSaves;
  holdSaves = true;
  changingChapter = true;
  let landed = false;
  try {
    landed = Boolean(await view.goTo(href));
    if (landed && !twoUp()) {
      await whenSized(currentDoc());
      land();
    }
  } finally {
    changingChapter = false;
  }
  if (!landed) {
    holdSaves = held;
    say('That link goes nowhere.');
    return;
  }
  if (from) backSpot = from;
  syncLayout();
}
/** With two columns, a note reached by a link shows alone: nothing before it, nothing after it. */
async function focusNoteAt(/** @type {string} */ href) {
  const doc = currentDoc();
  const el = doc && view?.book.resolveHref?.(href)?.anchor?.(doc);
  if (!el) return;
  const undo = focusNote(doc, el);
  if (!undo) return;
  undoFocus = undo;
  focusHref = href;
  view.renderer.render();
  await view.goTo(href);
}

/** Brings back the page around a note that was shown alone. */
async function unfocusNote(/** @type {boolean} */ backToNote = true) {
  const undo = undoFocus;
  if (!undo) return false;
  undoFocus = null;
  try {
    undo();
    view?.renderer.render();
    if (backToNote && focusHref) await view?.goTo(focusHref);
  } catch (e) {
    console.error('Could not bring the page back:', e);
  }
  return true;
}


async function goBack() {
  if (!backSpot || !view || changingChapter) return;
  const spot = backSpot;
  backSpot = null;
  holdSaves = false;
  changingChapter = true;
  try {
    await view.goTo(spot);
    if (!twoUp()) {
      await whenSized(currentDoc());
      land();
    }
  } finally {
    changingChapter = false;
  }
  syncLayout();
}

function askOutside(/** @type {string} */ raw) {
  links?.hide();
  /** @type {URL} */
  let url;
  try {
    url = new URL(raw);
  } catch {
    return;
  }
  if (!['http:', 'https:', 'mailto:'].includes(url.protocol)) return;
  const mail = url.protocol === 'mailto:';
  outside = { href: url.href, host: mail ? url.pathname : url.host, mail };
}

function goOutside() {
  const target = outside;
  outside = null;
  if (!target) return;
  invoke('plugin:opener|open_url', { url: target.href }).catch((e) => {
    console.error('Could not open the link:', e);
    say('That link could not be opened.');
  });
}

function focusIt(/** @type {HTMLElement} */ node) {
  node.focus();
}

function settleSoon() {
  if (settleFrame) return;
  settleFrame = requestAnimationFrame(() => {
    settleFrame = 0;
    if (!changingChapter && ready) settle();
  });
}

/** Scrolling the reader did not do itself is undone, back onto the remembered stop. */
function onScrollGuard(/** @type {Event} */ e) {
  if (twoUp()) return;
  const el = /** @type {HTMLElement} */ (e.currentTarget);
  if (!ready || changingChapter || Math.abs(el.scrollTop - written) < 1.5) return;
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
  // Going on, note which screen the chapter is left from, so coming back lands on it.
  const leaving = currentDoc();
  const leavingEl = leaving && scrollerOf(leaving);
  if (dir > 0 && leaving && leavingEl) {
    const { stops } = stopsOf(leaving);
    exits.set(current, stops.length - 1 - stopIndex(stops, leavingEl.scrollTop));
  }
  holdSaves = false;
  changingChapter = true;
  resting = null;
  pinnedEnd = null;
  try {
    await r.goTo({ index, anchor: dir > 0 ? 0 : 1 });
    await whenSized(currentDoc());
    endBack = exits.get(index) ?? null;
    settle({ toEnd: dir < 0 });
  } finally {
    changingChapter = false;
    wheelLocked = true;
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

/** The screens stepped through in each chapter, so Space and Left retrace exactly the same
 * ones, even after leaving the chapter and coming back to it. A chapter's trail starts over
 * only when its layout changes, or the reader rests somewhere the trail does not know. */
/** @type {Map<number, { key: string, at: number[], i: number }>} */
const trails = new Map();

function trailHere(/** @type {Document} */ doc, /** @type {number} */ s) {
  const index = view.renderer.getContents()[0]?.index ?? -1;
  const key = `${Math.round(view.renderer.size)}|${Math.round(doc.body.getBoundingClientRect().height)}`;
  let t = trails.get(index);
  const i = t && t.key === key ? t.at.findIndex((a) => Math.abs(a - s) < 1) : -1;
  if (!t || i < 0) {
    t = { key, at: [s], i: 0 };
    trails.set(index, t);
  } else t.i = i;
  return t;
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
  // A screen already stepped through is returned to exactly, whichever way you go.
  const t = trailHere(doc, s);
  const again = t.at[t.i + dir];
  const replay = again == null ? undefined : stops.find((st) => Math.abs(st.at - again) < 0.5);
  if (replay) {
    t.i += dir;
    return moveTo(doc, replay);
  }
  if (dir > 0) {
    const ahead = stops.filter((st) => st.at > s + 0.5);
    if (!ahead.length) return changeChapter(1);
    // The first line not fully on screen goes to the top.
    const cut = lines.find((l) => l.bottom > s + H + 0.5);
    // All the text left is already on screen and no picture is ahead: on to the next chapter.
    if (!cut && !ahead.some((st) => st.picture)) return changeChapter(1);
    // The last line fully on screen stays, at the top, so the eye has something to land on.
    // If that would not move at all (a screen of one line), the old step is used.
    // Only text that is not on screen yet and comes before the next picture gets the overlap.
    // When all of it is already showing, the next step is the picture itself.
    const nextPicture = ahead.find((st) => st.picture);
    const unseen = cut && (!nextPicture || cut.top < nextPicture.at - 0.5) ? cut : undefined;
    const kept = unseen ? lines[lines.indexOf(unseen) - 1] : undefined;
    const keep = kept ? ahead.findLast((st) => st.at <= kept.top + 0.5) : undefined;
    const target = keep ?? (cut ? ahead.findLast((st) => st.at <= cut.top + 0.5) : ahead.at(-1)) ?? ahead[0];
    const picture = ahead.find((st) => st.picture && st.at <= target.at);
    const dest = picture ?? target;
    moveTo(doc, dest);
    t.at.splice(t.i + 1, t.at.length, dest.at);
    t.i++;
  } else {
    const behind = stops.filter((st) => st.at < s - 0.5);
    if (!behind.length) return changeChapter(-1);
    // Mirrors going on: the line at the top now was the last one fully on screen before.
    const topLine = lines.find((l) => l.top >= s - 0.5);
    const need = topLine ? topLine.bottom - H - 0.5 : s - H - 0.5;
    // From a picture, back to the text before it as a whole screen (its last line at the
    // bottom), never the one-line screens the forward tail steps pass through.
    const onPicture = stops.some((st) => st.picture && Math.abs(st.at - s) < 1);
    const target = (onPicture ? behind.findLast((st) => !st.tail) : behind.find((st) => st.at >= need)) ?? /** @type {Stop} */ (behind.at(-1));
    const picture = behind.findLast((st) => st.picture && st.at >= target.at);
    const dest = picture ?? target;
    moveTo(doc, dest);
    t.at.splice(t.i, 0, dest.at);
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
  // A pinned note stays until it is dismissed. Esc closes it, and nothing moves under it.
  if (links?.isPinned()) {
    if (e.key === 'Escape') {
      e.preventDefault();
      links.hide();
      return;
    }
    if (!e.ctrlKey && !e.altKey && !e.metaKey && ['c', ' ', 'ArrowRight', 'ArrowLeft', 'ArrowUp', 'ArrowDown', 'PageUp', 'PageDown'].includes(e.key)) {
      e.preventDefault();
      return;
    }
  } else links?.hide();
  if (outside) {
    if (e.key === 'Escape') {
      e.preventDefault();
      outside = null;
    }
    return;
  }
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
    c: () => goBack(),
  };
  const move = moves[e.key];
  if (move) {
    e.preventDefault();
    if (e.key !== 'c') backSpot = null;
    move();
  }
}

const WHEEL_NOTCH = 50; // px: a single event this large is a mouse wheel notch
const TOUCHPAD_LINE = 30; // px of touchpad travel per line
let wheelTravel = 0;
let wheelLocked = false;
let lastWheel = 0;

function onWheel(/** @type {WheelEvent} */ e) {
  // A pinned note stays put, and nothing scrolls under it.
  if (links?.isPinned()) {
    e.preventDefault();
    return;
  }
  links?.hide();
  // The picker's list scrolls as usual.
  if (pickerOpen && !e.ctrlKey) return;
  e.preventDefault();
  if (e.ctrlKey || !view) return;
  const delta = e.deltaMode === 1 ? e.deltaY * 40 : e.deltaY;
  if (!delta) return;
  const now = performance.now();
  const quiet = now - lastWheel > 150;
  lastWheel = now;
  if (wheelLocked) {
    if (!quiet) return;
    wheelLocked = false;
  }
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
  holdSaves = false;
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
  if (!(await openBook())) {
    // The new book would not open. Go back to the one that was open, and say so in the picker.
    pickerError = 'That book could not be opened.';
    try {
      await invoke('reader_revert');
    } catch (e) {
      console.error('Could not go back to the earlier book:', e);
    }
    if (!(await openBook())) {
      showOpenFailure();
      return;
    }
    pickerOpen = true;
    return;

  }
  readingEl?.focus();
}

async function browseDisk() {
  const path = await openFileDialog({ multiple: false, directory: false, filters: [{ name: 'EPUB', extensions: ['epub'] }] });
  if (typeof path === 'string') replaceBook({ path });
}

/** Lets go of the book being shown, ready for another. */
function closeView() {
  links?.hide();
  view?.close();
  view?.remove();
  view = undefined;
  ready = false;
  positionKey = '';
  resting = null;
  percent = null;
  cutoff = 0;
  title = '';
  bookUuid = '';
  backSpot = null;
  holdSaves = false;
  outside = null;
}

// ---- Opening

/** What went wrong with the last book that would not open, as one short line. */
let openFailure = '';

/** A window with no book to show keeps its picker open, with the reason, so there is a way on. */
function showOpenFailure() {
  pickerError = `Book could not be opened${openFailure ? ` (${openFailure})` : ''}. Choose another book.`;
  pickerOpen = true;
}

/**
 * Opens the book this window was given and says whether it is showing. A book that will not
 * open leaves the window empty and the caller to decide what happens next, so the window is
 * never closed from in here.
 */
async function openBook() {
  try {
    await showBook();
    return true;
  } catch (e) {
    console.error('Could not open the book:', e);
    openFailure = String(/** @type {any} */ (e)?.message ?? e).split('\n')[0].slice(0, 140);
    try {
      closeView();
    } catch {}
    return false;
  }
}

async function showBook() {
  /** @type {any} */
  const opened = await invoke('reader_load');
  /** @type {any} */
  const book = await new EPUB(makeLoader(opened.entries)).init();
  title = opened.book_title || titleOf(book.metadata) || opened.file_name;
  bookUuid = opened.book_uuid ?? '';
  appWindow.setTitle(title).catch(() => {});
  // A library book is known by calibre's own uuid for it. Any other file keeps the old key.
  const legacyKey = keyFor(book, opened.path);
  positionKey = opened.book_uuid ? `uuid:${opened.book_uuid}` : legacyKey;

  book.transformTarget?.addEventListener('load', (/** @type {any} */ e) => {
    if (e.detail.isScript) e.detail.allow = false;
  });
  book.transformTarget?.addEventListener('data', (/** @type {any} */ e) => {
    const type = e.detail.type;
    if (PAGE_TYPES.includes(type)) {
      e.detail.data = Promise.resolve(e.detail.data).then(blockScripts).then((html) => dropSelectionStyles(html, type));
    } else if (type === 'text/css') {
      e.detail.data = Promise.resolve(e.detail.data).then((css) => dropSelectionStyles(css, type));
    }
  });

  const [saved] = await Promise.all([
    // A place saved under the old key is picked up once, and saved under the new key from then on.
    invoke('reader_position', { key: positionKey })
    .then((p) => p ?? (positionKey === legacyKey ? null : invoke('reader_position', { key: legacyKey })))
    .catch(() => null),
    invoke('reader_settings').then((s) => (settings = /** @type {any} */ (s))).catch(() => {}),
  ]);
  appliedStep = settings.font_step;

  exits.clear();
  trails.clear();
  view = document.createElement('foliate-view');
  view.addEventListener('link', async (/** @type {any} */ e) => {
    e.preventDefault();
    if (await links?.pinNote(e.detail.a, e.detail.href)?.catch(() => false)) return;
    followLink(e.detail.href);
  });
  view.addEventListener('external-link', (/** @type {any} */ e) => {
    e.preventDefault();
    askOutside(e.detail.href_);
  });
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
    links?.attach(doc, e.detail.index);
    watchChapter(doc);
    // This chapter's text size is known now, which can change whether two columns fit.
    requestAnimationFrame(syncLayout);
  });
  view.addEventListener('relocate', (/** @type {any} */ e) => {
    const { fraction, cfi, reason } = e.detail;
    // A link followed inside the book is a new place to read from, not a scroll to undo.
    if (reason === 'navigation' && !changingChapter) {
      resting = null;
      pinnedEnd = null;
    }
    if (typeof fraction === 'number') {
      percent = Math.round(fraction * 100);
      lastFraction = fraction;
    }

    if (ready && cfi && twoUp() && !holdSaves) savePositionSoon(cfi, fraction ?? 0);
  });
  readingEl.append(view);
  await view.open(book);
  view.renderer.setStyles?.('::selection { background: #F7C7CE !important; }');
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
  ready = true;
  settle();
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
  // A title changed in the library shows here at once.
  listen('book-title-changed', (/** @type {any} */ e) => {
    if (!bookUuid || e.payload.uuid !== bookUuid || e.payload.title === title) return;
    title = e.payload.title;
    appWindow.setTitle(title).catch(() => {});
  }).then((fn) => unlisten.push(fn));
  appWindow
    .onCloseRequested(() => Promise.all([settingsTimer ? flushSettings() : undefined, savePosition()]).then(() => {}))
    .then((fn) => unlisten.push(fn));

  // A window opened for one book that cannot show it has nothing else to show.
  openBook().then((ok) => {
    if (!ok) showOpenFailure();
  });
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
    {#if backSpot}
      <button class="back-chip" onclick={goBack}>Back</button>
    {/if}
    {#if notice}
      <div class="notice">{notice}</div>
    {/if}
    <LinkPreview bind:this={links} getBook={() => view?.book} onJump={(href) => followLink(href)} />
    {#if outside}
      <div class="ask" role="dialog" aria-label="Open link">
        <div class="ask-box">
          <p>{outside.mail ? 'This will open your email app to write to' : 'This will open'} <b>{outside.host}</b>{outside.mail ? '.' : ' in your browser.'}</p>
          <div class="ask-actions">
            <button onclick={() => (outside = null)}>Cancel</button>
            <button class="go" use:focusIt onclick={goOutside}>Go ahead</button>
          </div>
        </div>
      </div>
    {/if}
  </div>
  <!-- Opens and closes by growing and shrinking, the same glide as the edit dialog's side panels. -->
  <div class="panel-slot" style:height="{panelOpen ? panelHeight : 0}px" inert={!panelOpen}>
    <div bind:clientHeight={panelHeight}>
      <ReaderSettings open={panelOpen} padding={settings.padding} twoColumns={settings.two_columns} onLayout={setLayout} onChange={(/** @type {any} */ padding) => saveSettings({ ...settings, padding }, 300)} />
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
.back-chip {
  position: absolute; top: 8px; left: 14px; z-index: 2;
  font: 12px var(--zen-font-sans); color: var(--zen-panel);
  background: var(--zen-ink); border: 1px solid var(--zen-border); border-radius: 999px;
  padding: 3px 12px; cursor: pointer;
}
.back-chip:hover { color: var(--zen-ink); border-color: var(--zen-text-faint); }
.notice {
  position: absolute; z-index: 3; left: 50%; bottom: 36px; transform: translateX(-50%);
  font: 13px var(--zen-font-sans); color: var(--zen-text-secondary);
  background: var(--zen-panel); border: 1px solid var(--zen-border); border-radius: 6px;
  padding: 6px 14px; pointer-events: none;
}
.ask {
  position: absolute; inset: 0; z-index: 4; display: grid; place-items: center;
  background: color-mix(in srgb, var(--zen-ink) 18%, transparent);
}
.ask-box {
  max-width: 320px; box-sizing: border-box;
  background: var(--zen-panel); border: 1px solid var(--zen-border); border-radius: 6px;
  padding: 16px 18px; font: 14px/1.45 var(--zen-font-sans); color: var(--zen-ink);
}
.ask-box p { margin: 0; overflow-wrap: anywhere; }
.ask-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 14px; }
.ask-actions button {
  font: inherit; padding: 5px 12px; border-radius: 4px; cursor: pointer;
  color: var(--zen-text-secondary); background: transparent; border: 1px solid var(--zen-border);
}
.ask-actions .go { color: #fff; background: var(--zen-red); border-color: var(--zen-red); }
</style>
