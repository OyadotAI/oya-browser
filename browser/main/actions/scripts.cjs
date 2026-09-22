/**
 * The scripts page commands evaluate, in the analyzer's isolated world unless
 * a command says otherwise. Their text is kept exactly as it was: each is the
 * CDP payload a command sends.
 */
const { ELEMENT_WAIT_MS } = require('./constants.cjs');
const { presentJs, readElementsJs } = require('../../scripts/page-queries.cjs');

/**
 * Marks where a value goes in a script's text. A NUL never appears in the
 * scripts themselves, so splitting on it finds exactly the holes.
 */
const HOLE = '\u0000';

/** The script text with each hole replaced, in order, by one of `values`. */
const fill = (template, ...values) => template.split(HOLE).reduce((out, part, i) => out + values[i - 1] + part);

/** Finds an element through the injected analyzer, scrolls it clear of sticky headers, and returns its centre and metadata. */
const FIND_ELEMENT_JS = `(() => {
  window.__oyaInternalCall = true;
  try {
  const f = window.__acFindElement || ((s) => document.querySelector(s));
  const el = f(${HOLE});
  if (!el) return { ok: false, error: 'Element not found: ${HOLE}' };
  el.scrollIntoView({ behavior: 'instant', block: 'center' });
  // Check if element is behind a sticky header and adjust scroll
  let rect = el.getBoundingClientRect();
  if (rect.top < 80) {
    // Likely behind a sticky nav (LinkedIn, Reddit, HN all have sticky headers ~52-80px)
    window.scrollBy(0, rect.top - 100);
    rect = el.getBoundingClientRect();
  }
  let offsetX = 0, offsetY = 0;
  // If element is inside an iframe, offset by the iframe's position in the parent page
  const ownerDoc = el.ownerDocument;
  if (ownerDoc !== document) {
    for (const iframe of document.querySelectorAll('iframe')) {
      try { if (iframe.contentDocument === ownerDoc) {
        const iframeRect = iframe.getBoundingClientRect();
        offsetX = iframeRect.x;
        offsetY = iframeRect.y;
        break;
      }} catch {}
    }
  }
  // The handles a replay finds this element by again, read here because here is
  // the only place the element is unambiguous: an id from an earlier analysis may
  // name nothing by the time the action is recorded, and a step recorded without
  // handles replays as a click on the body.
  const text = (el.innerText || el.textContent || '').replace(/\\s+/g, ' ').trim().slice(0, 200);
  return {
    ok: true,
    data: {
      x: rect.x + rect.width / 2 + offsetX,
      y: rect.y + rect.height / 2 + offsetY,
      tag: el.tagName,
      editable: el.isContentEditable,
      inIframe: ownerDoc !== document,
      handle: {
        tag: (el.tagName || '').toLowerCase(),
        text,
        domId: el.id || undefined,
        name: el.getAttribute('name') || undefined,
        ariaLabel: el.getAttribute('aria-label') || undefined,
        testId: el.getAttribute('data-testid') || undefined,
        placeholder: el.getAttribute('placeholder') || undefined,
        // As written, which is what a CSS locator matches.
        rawHref: el.getAttribute('href') || undefined,
        role: el.getAttribute('role') || undefined,
        // Where it sits: the only durable handle for an element with no name and
        // no target of its own, such as a citation link on a wiki page.
        path: (window.__acCssPath && window.__acCssPath(el)) || undefined,
      },
    },
  };
  } finally { window.__oyaInternalCall = false; }
})()`;
/** Fills FIND_ELEMENT_JS for one call. */
const findElementJs = (selector) =>
  fill(FIND_ELEMENT_JS, JSON.stringify(selector), JSON.stringify(selector).slice(1, -1).replace(/'/g, "\\'"));

/** Replays a full pointer and mouse click sequence on an element inside an iframe, where CDP mouse events may not reach framework handlers. */
const IFRAME_CLICK_JS = `(() => {
          const f = window.__acFindElement || ((s) => document.querySelector(s));
          const el = f(${HOLE});
          if (!el) return;
          const rect = el.getBoundingClientRect();
          const x = rect.x + rect.width / 2;
          const y = rect.y + rect.height / 2;
          const w = el.ownerDocument.defaultView;
          const opts = { bubbles: true, cancelable: true, view: w, clientX: x, clientY: y, screenX: x, screenY: y };
          el.dispatchEvent(new PointerEvent('pointerdown', { ...opts, pointerId: 1, pointerType: 'mouse' }));
          el.dispatchEvent(new MouseEvent('mousedown', { ...opts, button: 0, buttons: 1 }));
          el.dispatchEvent(new PointerEvent('pointerup', { ...opts, pointerId: 1, pointerType: 'mouse' }));
          el.dispatchEvent(new MouseEvent('mouseup', { ...opts, button: 0 }));
          el.dispatchEvent(new MouseEvent('click', { ...opts, button: 0 }));
        })()`;
/** Fills IFRAME_CLICK_JS for one call. */
const iframeClickJs = (selector) => fill(IFRAME_CLICK_JS, JSON.stringify(selector));

/** Selects a field's content so Backspace clears just that field; answers 'select' or false. */
const SELECT_FIELD_JS = `(() => {
          const f = window.__acFindElement || ((s) => document.querySelector(s));
          const el = f(${HOLE});
          if (!el) return false;
          el.focus();
          if (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA') {
            el.select();
            return 'select';
          } else if (el.isContentEditable) {
            const sel = window.getSelection();
            sel.selectAllChildren(el);
            return 'select';
          }
          return false;
        })()`;
/** Fills SELECT_FIELD_JS for one call. */
const selectFieldJs = (selector) => fill(SELECT_FIELD_JS, JSON.stringify(selector));

/** An element's input type, or null when it is not an <input>. */
const INPUT_TYPE_JS = `(() => {
          const f = window.__acFindElement || ((s) => document.querySelector(s));
          const el = f(${HOLE});
          return el && el.tagName === 'INPUT' ? el.type : null;
        })()`;
/** Fills INPUT_TYPE_JS for one call. */
const inputTypeJs = (selector) => fill(INPUT_TYPE_JS, JSON.stringify(selector));

/** Sets a date or time input's value as its picker would, firing input and change; answers the value it kept. */
const SET_INPUT_VALUE_JS = `(() => {
          const f = window.__acFindElement || ((s) => document.querySelector(s));
          const el = f(${HOLE});
          if (!el) return null;
          el.focus();
          el.value = ${HOLE};
          el.dispatchEvent(new Event('input', { bubbles: true }));
          el.dispatchEvent(new Event('change', { bubbles: true }));
          return el.value;
        })()`;
/** Fills SET_INPUT_VALUE_JS for one call. */
const setInputValueJs = (selector, value) => fill(SET_INPUT_VALUE_JS, JSON.stringify(selector), JSON.stringify(value));

/** A field's current text: an input's value or an editable element's text; null when it is gone. */
const FIELD_VALUE_JS = `(() => {
          const f = window.__acFindElement || ((s) => document.querySelector(s));
          const el = f(${HOLE});
          if (!el) return null;
          return el.isContentEditable ? el.innerText : (el.value ?? null);
        })()`;
/** Fills FIELD_VALUE_JS for one call. */
const fieldValueJs = (selector) => fill(FIELD_VALUE_JS, JSON.stringify(selector));

/** Focuses a field with the caret at its start, where an unfilled mask (__/__/____) takes its first character. */
const CARET_START_JS = `(() => {
          const f = window.__acFindElement || ((s) => document.querySelector(s));
          const el = f(${HOLE});
          if (!el) return;
          el.focus();
          try { el.setSelectionRange(0, 0); } catch {}
        })()`;
/** Fills CARET_START_JS for one call. */
const caretStartJs = (selector) => fill(CARET_START_JS, JSON.stringify(selector));

/** Whether an autocomplete or suggestion list is showing. */
const DROPDOWN_JS = `(() => {
        const lists = document.querySelectorAll('[role="listbox"], [role="menu"], [role="list"], .pac-container, [class*="suggest"], [class*="autocomplete"], [class*="dropdown"], [id*="suggest"], [id*="autocomplete"], ul[class*="result"]');
        for (const l of lists) {
          const r = l.getBoundingClientRect();
          if (r.width > 0 && r.height > 0) return true;
        }
        return false;
      })()`;

/** The page's viewport size. */
const VIEWPORT_JS = '({ w: window.innerWidth, h: window.innerHeight })';

/**
 * Analyses the page after a scroll, or reports the scroll when the analyzer is
 * missing. The amount is written as a number whatever is passed in: it becomes
 * code here.
 */
const scrollResultJs = (params, amount) =>
  `(typeof analyzePage === 'function') ? analyzePage(${JSON.stringify(params?.analyze || {})}) : { ok: true, data: { direction: ${JSON.stringify(String(params?.direction || 'down'))}, amount: ${Number(amount) || 0} } }`;

/** Sets a <select>'s value and fires change and input. */
const SELECT_OPTION_JS = `(() => {
        const f = window.__acFindElement || ((s) => document.querySelector(s));
        const el = f(${HOLE});
        if (!el || el.tagName !== 'SELECT') return { ok: false, error: 'Select element not found' };
        el.value = ${HOLE};
        el.dispatchEvent(new Event('change', { bubbles: true }));
        el.dispatchEvent(new Event('input', { bubbles: true }));
        return { ok: true, data: { selected: el.value } };
      })()`;
/** Fills SELECT_OPTION_JS for one call. */
const selectOptionJs = (selector, value) => fill(SELECT_OPTION_JS, JSON.stringify(selector), JSON.stringify(value));

/** The dev panel's analyze: the analyzer's full read of the page. */
const DEV_ANALYZE_JS =
  '(typeof analyzePage === "function") ? analyzePage({}) : { ok: false, error: "Analyzer not loaded" }';

/**
 * How long a wait lasts: the caller's timeout when it is a positive number, else
 * the default. It is written into a script as code, so it is never the caller's
 * own text: a string would run in the page.
 */
function waitMs(params) {
  const ms = Number(params?.timeout);
  return Number.isFinite(ms) && ms > 0 ? ms : ELEMENT_WAIT_MS;
}

/** The dev panel's wait: polls a plain CSS selector until it matches or times out. */
const devWaitJs = (params) =>
  `(async () => { const maxWait = ${waitMs(params)}; const start = Date.now(); while (Date.now() - start < maxWait) { if (document.querySelector(${JSON.stringify(params.selector)})) return { ok: true, data: { found: true } }; await new Promise(r => setTimeout(r, 250)); } return { ok: false, error: 'Timeout' }; })()`;

/** Scripts for the actions that have no handler of their own, by action. */
const ACTION_SCRIPTS = {
  /** The analyzer's read of the page. */
  analyze: (params) =>
    `(typeof analyzePage === 'function') ? analyzePage(${JSON.stringify(params || {})}) : { ok: false, error: 'Analyzer not loaded' }`,
  /** Polls for an element (an analyzer id or CSS, in the page or its iframes) until it appears or the wait runs out. */
  wait: (params) =>
    `(async () => { const present = () => ${presentJs(params?.selector)}; const maxWait = ${waitMs(params)}; const start = Date.now(); while (Date.now() - start < maxWait) { if (present()) return { ok: true, data: { found: true } }; await new Promise(r => setTimeout(r, 250)); } return { ok: false, error: 'Timeout' }; })()`,
  /** The visible elements matching a selector, in the page and its iframes. */
  read_page: (params) => readElementsJs(params?.selector, params?.limit),
};

/**
 * The script for an action with no handler of its own, or null for an action
 * this browser does not know. The action's name is the caller's text, so it
 * never becomes part of a script.
 */
function actionScript(action, params) {
  return Object.hasOwn(ACTION_SCRIPTS, action) ? ACTION_SCRIPTS[action](params) : null;
}

module.exports = {
  findElementJs,
  iframeClickJs,
  selectFieldJs,
  inputTypeJs,
  fieldValueJs,
  caretStartJs,
  setInputValueJs,
  DROPDOWN_JS,
  VIEWPORT_JS,
  scrollResultJs,
  selectOptionJs,
  DEV_ANALYZE_JS,
  devWaitJs,
  actionScript,
  ACTION_SCRIPTS,
};
