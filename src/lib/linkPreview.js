import DOMPurify from 'dompurify';

// What a link in a book points at, and what to show when the pointer rests on it. The footnote
// detection follows foliate's own rules: a link marked as a note reference, or one that looks like
// a superscript marker, is a footnote. Any other link inside the book gets a light peek.

const OPS = 'http://www.idpf.org/2007/ops';
const types = (el) => new Set(el?.getAttributeNS?.(OPS, 'type')?.split(/\s+/));
const roles = (el) => new Set(el?.getAttribute?.('role')?.split(/\s+/));

const isSuper = (el) => {
  if (!el || el.matches?.('body, html')) return false;
  if (el.matches('sup')) return true;
  const { verticalAlign } = getComputedStyle(el);
  return verticalAlign === 'super' || verticalAlign === 'top' || verticalAlign === 'text-top' || /^\d/.test(verticalAlign);
};

const INLINE = 'a, span, sup, sub, em, strong, i, b, small, big';

/** True when the link sits inside a superscript, however many small wrappers lie between them. */
const inSuper = (/** @type {Element} */ a) => {
  let n = a.parentElement;
  for (let depth = 0; n && depth < 4 && n.matches?.(INLINE); depth++, n = n.parentElement) if (isSuper(n)) return true;
  return false;
};

const isBacklink = (a) => types(a).has('backlink') || roles(a).has('doc-backlink');

/** 'footnote' for a note reference, 'peek' for any other link that stays inside the book. */
export function kindOf(/** @type {Element} */ a) {
  const t = types(a);
  const r = roles(a);
  if (['biblioref', 'glossref', 'noteref'].some((x) => t.has(x)) || ['doc-biblioref', 'doc-glossref', 'doc-noteref'].some((x) => r.has(x))) return 'footnote';
  if (isBacklink(a)) return 'peek';
  if (isSuper(a) || (a.children.length === 1 && isSuper(a.children[0])) || inSuper(a)) return 'footnote';
  return 'peek';
}

/** The ids a marker may be found by: its own, or one on an empty anchor or a wrapper right beside it. */
export function idsOf(/** @type {Element} */ a) {
  const prev = a.previousElementSibling;
  const wrapper = a.parentElement?.matches?.(INLINE) ? a.parentElement : null;
  return [a.id, prev && !textOf(prev) ? prev.id : '', wrapper?.id ?? ''].filter(Boolean);
}

/** What to show for a link that leaves the book: the address, or just the mail address. */
export function addressOf(/** @type {string} */ raw) {
  return raw.replace(/^mailto:/i, '').slice(0, 300);
}

// Parsed chapters, kept for a while so hovering several links to the same place is quick. Each
// book has its own, so a book opened later never shows another book's text.
/** @type {WeakMap<object, Map<number, Promise<Document>>>} */
const caches = new WeakMap();
function chapterDoc(book, index) {
  let docs = caches.get(book);
  if (!docs) caches.set(book, (docs = new Map()));
  let found = docs.get(index);
  if (!found) {
    found = Promise.resolve(book.sections[index].createDocument());
    docs.set(index, found);
    if (docs.size > 6) docs.delete(docs.keys().next().value);
  }
  return found;
}

/** The most of a note that is shown in the box. A longer one is cut and links to the notes page. */
const NOTE_LIMIT = 700;
/** A note longer than this is more likely a whole section than a note, so its link just jumps. */
const NOTE_MAX = 6000;

/** Cuts html after about `max` characters of text, at a word, keeping the tags valid. */
function limitHtml(/** @type {string} */ html, max = NOTE_LIMIT) {
  const holder = document.createElement('template');
  holder.innerHTML = html;
  const walker = document.createTreeWalker(holder.content, NodeFilter.SHOW_TEXT);
  let used = 0;
  /** @type {Text | null} */
  let last = null;
  for (let node = /** @type {Text | null} */ (walker.nextNode()); node; node = /** @type {Text | null} */ (walker.nextNode())) {
    if (used + node.data.length > max) {
      node.data = node.data.slice(0, Math.max(0, max - used)).replace(/\s+\S*$/, '').replace(/[\s,;:\u2013-]+$/, '') + '...';
      last = node;
      break;
    }
    used += node.data.length;
  }
  if (!last) return { html, cut: false };
  for (let node = /** @type {Node | null} */ (last); node && node !== holder.content; node = node.parentNode) {
    while (node.nextSibling) node.nextSibling.remove();
  }
  return { html: holder.innerHTML, cut: true };
}

const clean = (html) =>
  DOMPurify.sanitize(html, { ALLOWED_TAGS: ['p', 'div', 'span', 'em', 'i', 'strong', 'b', 'sub', 'sup', 'br', 'small', 'cite', 'q', 'blockquote', 'ul', 'ol', 'li', 'dl', 'dt', 'dd'], ALLOWED_ATTR: [] });
const escapeHtml = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
const textOf = (el) => (el?.textContent ?? '').replace(/\s+/g, ' ').trim();
const cut = (s, n) => (s.length > n ? s.slice(0, n).replace(/\s+\S*$/, '') + '\u2026' : s);

/** The block a note lives in, found from the element its link points at. */
function noteBlock(doc, el) {
  let node = el;
  while (node.matches?.(INLINE)) {
    if (!node.parentElement) break;
    node = node.parentElement;
  }
  if (node === doc.body) {
    const next = el.nextElementSibling;
    return next && !next.matches(INLINE) ? next : null;
  }
  return node;
}

/** The text in a block before an element. A marker sits in running text, a note starts with its label. */
function textBefore(block, el) {
  if (el === block || !block.contains(el)) return '';
  const range = block.ownerDocument.createRange();
  range.setStart(block, 0);
  range.setEndBefore(el);
  return range.toString().replace(/\s+/g, ' ').trim();
}

/** True for a link that opens its note, like the number in front of it: nothing comes before it. */
export function labelAt(/** @type {Element} */ a) {
  const block = noteBlock(a.ownerDocument, a);
  return !!block && block !== a && textBefore(block, a) === '';
}

/** True when a block opens with a link, as the number that starts a note often does. */
function startsWithLink(block) {
  const link = block.querySelector('a[href]');
  return !!link && textBefore(block, link) === '';
}

/** True for a note marker. A link that points at one is a link back from a note, which should jump. */
const isMarker = (/** @type {Element} */ el) => !!el.closest?.('a[href]') || (!!el.matches?.(INLINE) && !!el.querySelector?.('a[href]'));

function noteHtml(block, ids, label = true) {
  const copy = /** @type {Element} */ (block.cloneNode(true));
  for (const a of [...copy.querySelectorAll('a')]) {
    const fragment = (a.getAttribute('href') ?? '').split('#')[1];
    const arrow = /^[\s\u21a9\u21b5\u2191^]*$|^back$/i.test(textOf(a));
    if (isBacklink(a) || arrow || (fragment && ids.includes(fragment))) a.remove();
  }
  const html = copy.matches('li, aside') ? copy.innerHTML : copy.outerHTML;
  const safe = clean(html);
  return label ? safe.replace(/^(\s|<[^>]+>)*?([.:)\]]\s*)/, (m, _a, lead) => m.replace(lead, '')) : safe;
}

/** True when a paragraph opens like a new note: with a link, an id of its own, or a number or symbol. */
const opensNote = (n) => startsWithLink(n) || !!n.id || /^(\[\d+\]|\(\d+\)|\d+[.):]|[*\u2020\u2021\u00a7])\s/.test(textOf(n));

/** A note that opens with its own number goes on into the plain paragraphs after it. */
function continuation(block) {
  if (!/^(p|div)$/i.test(block.localName) || !startsWithLink(block)) return [];
  const rest = [];
  let total = textOf(block).length;
  for (let n = block.nextElementSibling; n && total <= NOTE_LIMIT && rest.length < 8; n = n.nextElementSibling) {
    if (!/^(p|div|blockquote)$/i.test(n.localName) || opensNote(n)) break; // a heading, or the next note
    const length = textOf(n).length;
    if (!length) continue;
    rest.push(n);
    total += length;
  }
  return rest;
}

const noteBody = (block, ids) => [noteHtml(block, ids), ...continuation(block).map((n) => noteHtml(n, ids, false))].join('');

/** 'rtl' for a note written right to left. The letters it is written in decide, then dir and lang. */
function dirOf(/** @type {string} */ html, /** @type {Element | null} */ el) {
  const letters = html.replace(/<[^>]+>/g, '').match(/\p{L}/gu) ?? [];
  const rtl = letters.filter((c) => /[\u0590-\u08ff\ufb1d-\ufdff\ufe70-\ufeff]/.test(c)).length;
  if (letters.length && rtl / letters.length >= 0.5) return 'rtl';
  if (letters.length && rtl / letters.length < 0.1) return 'ltr';
  for (let n = el; n; n = n.parentElement) {
    const dir = n.getAttribute?.('dir')?.toLowerCase();
    if (dir === 'rtl' || dir === 'ltr') return dir;
    const lang = (n.getAttribute?.('lang') || n.getAttribute?.('xml:lang') || '').toLowerCase().split('-')[0];
    if (lang) return ['ar', 'he', 'iw', 'fa', 'ur', 'yi', 'ps', 'sd', 'ug', 'dv', 'syr'].includes(lang) ? 'rtl' : 'ltr';
  }
  return 'ltr';
}

/** A note starts at the place a link points to. A link back from a note points into running text. */
function looksLikeNote(block, el, fromLabel) {
  if (!block || /^h[1-6]$/i.test(block.localName) || textOf(block).length > NOTE_MAX) return false;
  const lead = textBefore(block, el).length;
  return !(lead > 12 || (fromLabel && lead > 0));
}

function peek(doc, el) {
  const isHeading = (n) => /^h[1-6]$/i.test(n?.localName ?? '');
  let title = null;
  let from = null; // the element after which the following paragraphs are read
  let first = null; // or the element that is itself the first paragraph
  if (!el || el === doc.body || el === doc.documentElement) {
    const heading = doc.querySelector('h1, h2, h3, h4, h5, h6');
    if (heading) {
      title = textOf(heading);
      from = heading;
    } else first = doc.body.firstElementChild;
  } else if (isHeading(el)) {
    title = textOf(el);
    from = el;
  } else {
    first = noteBlock(doc, el) ?? el;
  }
  const paragraphs = [];
  let total = 0;
  const take = (node) => {
    const t = textOf(node);
    if (!t) return true;
    paragraphs.push(t);
    total += t.length;
    return total < 340;
  };
  if (first && !take(first)) return { title, html: paragraphs.map((p) => `<p>${escapeHtml(cut(p, 420))}</p>`).join('') };
  for (let n = (from ?? first)?.nextElementSibling; n && total < 340 && paragraphs.length < 3; n = n.nextElementSibling) {
    if (isHeading(n)) break;
    take(n);
  }
  return { title, html: paragraphs.map((p) => `<p>${escapeHtml(cut(p, 420))}</p>`).join('') };
}

const idList = (ids) => (Array.isArray(ids) ? ids : ids ? [ids] : []);

/**
 * What to show for an internal link, or null when it goes nowhere.
 * @returns {Promise<{ title: string | null, html: string, cut: boolean, dir: string } | null>}
 */
export async function previewFor(book, /** @type {string} */ href, /** @type {string} */ kind, ids = [], fromLabel = false) {
  const target = book.resolveHref?.(href);
  if (!target) return null;
  const doc = await chapterDoc(book, target.index);
  const el = target.anchor(doc);
  if (kind === 'footnote' && el && !isMarker(el)) {
    const block = noteBlock(doc, el);
    if (looksLikeNote(block, el, fromLabel)) {
      const html = noteBody(block, idList(ids));
      if (html.trim()) return { title: null, ...limitHtml(html), dir: dirOf(html, block) };
    }
  }
  const p = peek(doc, el || null);
  return p.html ? { ...p, cut: false, dir: dirOf(p.html, el || doc.body) } : null;
}

/**
 * The note a link points at, when it looks like one: a block of text that is not a heading, not a
 * whole section, not a marker, and not running text that a note links back to. Null means the link
 * should just jump. `fromLabel` says the link itself opens a note, as the number in front of it does.
 * @returns {Promise<{ title: null, html: string, cut: boolean, dir: string } | null>}
 */
export async function noteFor(book, /** @type {string} */ href, ids = [], fromLabel = false) {
  const target = book.resolveHref?.(href);
  if (!target) return null;
  const doc = await chapterDoc(book, target.index);
  const el = target.anchor(doc);
  if (!el || isMarker(el)) return null;
  const block = noteBlock(doc, el);
  if (!looksLikeNote(block, el, fromLabel)) return null;
  const html = noteBody(block, idList(ids));
  return html.trim() ? { title: null, ...limitHtml(html), dir: dirOf(html, block) } : null;
}
