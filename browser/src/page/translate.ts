/**
 * Page translation's scripts, run in their own isolated world on a person's own
 * tab: the page never sees their source or the state they keep. What they write
 * into the page (text node values, only after the person clicked Translate) is
 * visible to the page's MutationObservers, as Chrome's own translate is. They
 * never run on a tab an agent drives or while a recording runs (main/translate).
 * This text runs in the page, so it changes only on purpose.
 */

/** An expression: the page's declared language tag (`<html lang>`, else the content-language meta); '' when there is none, it is malformed or names no language, or the page opts out of translation. */
export const detectJs = `(() => {
  const html = document.documentElement, body = document.body;
  const optOut = (el) => !!el && (el.getAttribute('translate') === 'no' || el.classList.contains('notranslate'));
  if (optOut(html) || optOut(body) || body?.isContentEditable || document.querySelector('meta[name="google" i][content="notranslate" i]')) return '';
  const meta = document.querySelector('meta[http-equiv="content-language" i]');
  const raw = html?.getAttribute('lang') || meta?.getAttribute('content') || '';
  const tag = raw.split(',')[0].trim();
  // und, zxx, mul, mis name no language a person reads: nothing to offer from.
  return /^[a-z]{2,3}(-[a-z0-9]{2,8})*$/i.test(tag) && !/^(und|zxx|mul|mis)(-|$)/i.test(tag) ? tag : '';
})()`;

/**
 * Script text: what both scripts skip, and a walk over the text nodes under an element.
 * Hidden is display:none on the element itself (its parents were checked on the way
 * down); a display:contents wrapper or an <option> has no box of its own but its text shows.
 */
const WALK = `const SKIP = new Set(['SCRIPT', 'STYLE', 'NOSCRIPT', 'TEXTAREA', 'CODE', 'PRE', 'TEMPLATE', 'svg', 'SVG']);
  const skip = (el) => SKIP.has(el.tagName) || el.getAttribute('translate') === 'no' || el.classList.contains('notranslate')
    || el.isContentEditable || getComputedStyle(el).display === 'none';
  const walk = (el, onText) => {
    for (const node of el.childNodes) {
      if (node.nodeType === 1 && !skip(node)) { if (walk(node, onText) === false) return false; }
      else if (node.nodeType === 3 && onText(node) === false) return false;
    }
  };`;

/** Script text: the collect function, given the page limit and the batch limit. */
const COLLECT = `(maxChars, batchChars) => {
  ${WALK}
  // A page that opts out as a whole (translate="no" or notranslate on <html> or <body>, Google's notranslate meta), or whose body is editable.
  const root = document.body;
  const optedOut = !root || skip(root) || skip(document.documentElement)
    || !!document.querySelector('meta[name="google" i][content="notranslate" i]');
  const texts = [], seen = new Set();
  let total = 0, partial = false;
  // Each distinct text once: a carousel's copies of its slides cost nothing more.
  const take = (node) => {
    const text = node.data.trim();
    if (!/\\p{L}/u.test(text) || seen.has(text)) return;
    if (text.length > batchChars) return void (partial = true);
    if (total + text.length > maxChars) return (partial = true, false);
    seen.add(text); total += text.length; texts.push(text);
  };
  if (!optedOut) walk(root, take);
  globalThis.__oyaTranslation = { texts, dict: new Map(), back: new Map(), done: new WeakSet(), written: [], observer: null };
  return { texts, partial };
}`;

/**
 * An expression: collects the distinct visible texts of the page's body, at most
 * `maxChars` of them, for apply to translate by. A text longer than `batchChars` is
 * left out. Answers the trimmed texts, and whether anything was left out.
 */
export const collectJs = (maxChars: number, batchChars: number): string => `(${COLLECT})(${maxChars}, ${batchChars})`;

/** Script text: the apply function, given the first text's index and the translations. */
const APPLY = `(start, out) => {
  ${WALK}
  const state = globalThis.__oyaTranslation;
  if (!state || !document.body) return 0;
  out.forEach((text, i) => {
    const original = state.texts[start + i];
    if (original === undefined) return;
    state.dict.set(original, String(text).trim());
    state.back.set(String(text).trim(), original);
  });
  // A node with a known text is translated; one that already holds a translation (the page
  // copied a translated node) is only remembered, so Show original puts it back too.
  const write = (node) => {
    if (state.done.has(node)) return;
    const now = node.data.trim(), lead = node.data.match(/^\\s*/)[0], trail = node.data.match(/\\s*$/)[0];
    const text = state.dict.get(now), from = text === undefined ? state.back.get(now) : now;
    if (from === undefined) return;
    if (text !== undefined) node.data = lead + text + trail;
    state.done.add(node);
    state.written.push([node, lead + from + trail, node.data]);
  };
  const before = state.written.length;
  walk(document.body, write);
  // Nodes the page adds later (a carousel rebuilding its slides, a menu redrawn) get any translation already known; nothing new is sent.
  state.observer ??= new MutationObserver((records) => records.forEach((r) => r.addedNodes.forEach((n) =>
    n.nodeType === 3 ? write(n) : n.nodeType === 1 && !skip(n) && walk(n, write))));
  state.observer.observe(document.body, { childList: true, subtree: true });
  return state.written.length - before;
}`;

/**
 * An expression: learns `out` as the translations of the collected texts from
 * `start` on, then writes every text node on the page that holds one of the texts it
 * knows, as text only, keeping each node's own leading and trailing whitespace; and
 * keeps doing so for nodes the page adds later. The model's text enters only through
 * JSON.stringify: it is untrusted.
 */
export const applyJs = (start: number, texts: string[]): string =>
  `(${APPLY})(${JSON.stringify(start)}, ${JSON.stringify(texts)})`;

/** An expression: stops translating added nodes, puts back the original text of every node that still holds what was written into it, then forgets them. */
export const restoreJs = `(() => {
  const state = globalThis.__oyaTranslation;
  if (!state) return 0;
  state.observer?.disconnect();
  let restored = 0;
  state.written.forEach(([node, original, text]) => {
    if (node.data === text) { node.data = original; restored++; }
  });
  delete globalThis.__oyaTranslation;
  return restored;
})()`;
