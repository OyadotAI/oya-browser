/**
 * DOM helpers every renderer script uses: lookup by id, HTML escaping, and
 * building small elements.
 */
/* exported Dom */

/** Small DOM helpers. */
const Dom = {
  /** The element with this id. */
  byId: (id) => document.getElementById(id),

  /** `s` escaped for use inside HTML, by letting the DOM serialize it as text. */
  esc(s) {
    const d = document.createElement('div');
    d.textContent = s;
    return d.innerHTML;
  },

  /** A new `tag` element with optional text and class. */
  node(tag, text, cls) {
    const el = document.createElement(tag);
    if (text != null) el.textContent = text;
    if (cls) el.className = cls;
    return el;
  },
};
