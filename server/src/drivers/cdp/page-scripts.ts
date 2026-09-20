/**
 * Page-side JavaScript the driver evaluates, kept as source builders. Every
 * caller-supplied value goes in through JSON.stringify, never raw.
 */

/** FIND_ELEMENT_JS's source; __SELECTOR__ stands for the selector. */
const FIND_ELEMENT_SOURCE = `(() => {
  {
    const f = window.__acFindElement || ((s) => document.querySelector(s));
    const el = f(__SELECTOR__);
    if (!el) return { ok: false, error: 'Element not found' };
    el.scrollIntoView({ behavior: 'instant', block: 'center' });
    let r = el.getBoundingClientRect();
    if (r.top < 80) { window.scrollBy(0, r.top - 100); r = el.getBoundingClientRect(); }
    return { ok: true, data: { x: r.left + r.width / 2, y: r.top + r.height / 2 } };
  }
})()`;

/** Scrolls an element into view (clear of a sticky header) and returns its centre point. */
export const FIND_ELEMENT_JS = (selector) =>
  FIND_ELEMENT_SOURCE.replace('__SELECTOR__', () => JSON.stringify(selector));

/** Selects an element's existing contents, so typed text replaces them. */
export const SELECT_CONTENTS_JS = (selector) =>
  `(() => { const el = (window.__acFindElement || ((s) => document.querySelector(s)))(${JSON.stringify(selector)}); if (el?.isContentEditable) { const range = document.createRange(); range.selectNodeContents(el); const selection = getSelection(); selection.removeAllRanges(); selection.addRange(range); } else el?.select?.(); })()`;

/** An element's input type, or null when it is not an <input>. */
export const INPUT_TYPE_JS = (selector) =>
  `(() => { const el = (window.__acFindElement || ((s) => document.querySelector(s)))(${JSON.stringify(selector)}); return el && el.tagName === 'INPUT' ? el.type : null; })()`;

/** Sets a date or time input's value as its picker would, firing input and change; answers the value it kept. */
export const SET_DATE_VALUE_JS = (selector, value) => `(() => {
          const el = (window.__acFindElement || ((s) => document.querySelector(s)))(${JSON.stringify(selector)});
          if (!el) return null;
          el.focus();
          el.value = ${JSON.stringify(value)};
          el.dispatchEvent(new Event('input', { bubbles: true }));
          el.dispatchEvent(new Event('change', { bubbles: true }));
          return el.value;
        })()`;

/** Sets a form control's value and fires change; answers whether the element was found. */
export const SET_VALUE_JS = (selector, value) => `(() => {
          const f = window.__acFindElement || ((s) => document.querySelector(s));
          const el = f(${JSON.stringify(selector)});
          if (!el) return false;
          el.value = ${JSON.stringify(value)};
          el.dispatchEvent(new Event('change', { bubbles: true }));
          return true;
        })()`;

/** Runs the analyzer with the caller's options. */
export const ANALYZE_JS = (params) =>
  `(typeof analyzePage === 'function') ? analyzePage(${JSON.stringify(params || {})}) : { ok: false, error: 'Analyzer not loaded' }`;

/** Scrolls the window to its top or bottom edge. */
export const SCROLL_TO_JS = (top) => `window.scrollTo({ top: ${top} })`;
