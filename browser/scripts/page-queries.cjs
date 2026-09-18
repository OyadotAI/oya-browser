/**
 * Page-side queries the agent's wait and read_elements tools run, shared by the
 * desktop driver (main/actions) and the server's CDP driver. Both look through
 * same-origin iframes as well as the page: portals such as Availity render their
 * whole form inside one, where a plain document.querySelector never looks.
 */
const { QUERIES } = require('./constants.cjs');

/**
 * An analyzer element reference: its number alone, or the attribute selector
 * the tools build from it ([data-ac-id="12"]). Anything else is CSS.
 */
const ANALYZER_REF = '/^\\d+$|^\\[data-[\\w-]+="\\d+"\\]$/';

/** What read_elements lists when no selector is given: things a person can act on. */
const INTERACTIVE =
  'a[href], button, input:not([type=hidden]), select, textarea, [role=button], [role=link], [contenteditable=true]';

/** Script text: the first match for `s` in a document or any same-origin iframe inside it. */
const FIND_DEEP = `const findDeep = (doc, s) => {
    let hit = null;
    try { hit = doc.querySelector(s); } catch { return null; }
    if (hit) return hit;
    for (const f of doc.querySelectorAll('iframe, frame')) {
      try { const inner = f.contentDocument && findDeep(f.contentDocument, s); if (inner) return inner; } catch {}
    }
    return null;
  };`;

/** An expression: whether `selector` (an analyzer reference or CSS) matches anything on the page. */
const presentJs = (selector) => `((s) => {
  ${FIND_DEEP}
  if (${ANALYZER_REF}.test(s) && window.__acFindElement) return !!window.__acFindElement(s);
  return !!findDeep(document, s);
})(${JSON.stringify(String(selector || ''))})`;

/** Script text: how one element is listed; an input's typed value is never read. */
const DESCRIBE = `const describe = (el) => ({
    tag: el.tagName.toLowerCase(),
    id: el.id || '',
    text: (el.innerText || (/^(submit|button|reset)$/.test(el.type) ? el.value : '') || el.placeholder || '').replace(/\\s+/g, ' ').trim().slice(0, 80),
    aria_label: el.getAttribute('aria-label') || '',
  });`;

/** Script text: the function read_elements runs, given a selector and a limit. */
const READ_ELEMENTS = `(s, limit) => {
  ${DESCRIBE}
  const out = [];
  const visit = (doc) => {
    let found = [];
    try { found = doc.querySelectorAll(s); } catch { return; }
    for (const el of found) {
      if (out.length >= limit) return;
      const r = el.getBoundingClientRect();
      if (r.width || r.height) out.push(describe(el));
    }
    for (const f of doc.querySelectorAll('iframe, frame')) { try { if (f.contentDocument) visit(f.contentDocument); } catch {} }
  };
  visit(document);
  return { ok: true, data: { url: location.href, title: document.title, elements: out } };
}`;

/** An expression: up to `limit` visible elements matching `selector` (interactive ones by default), with the page's URL and title. */
function readElementsJs(selector, limit) {
  const max = Number(limit) > 0 ? Math.floor(Number(limit)) : QUERIES.DEFAULT_LIMIT;
  return `(${READ_ELEMENTS})(${JSON.stringify(String(selector || INTERACTIVE))}, ${max})`;
}

module.exports = { presentJs, readElementsJs, INTERACTIVE };
