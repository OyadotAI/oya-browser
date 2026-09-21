/**
 * A minimal DOM for driving the shell's renderer scripts under plain Node:
 * elements with attributes, classes, dataset and style, an HTML parser for
 * index.html and innerHTML, a small selector engine (tag, #id, .class,
 * [attr], [attr=value], :not(:disabled), descendants), and events with
 * capture and bubbling. Only what the renderer uses; no layout.
 */

/** Elements that never have children or a closing tag. */
const VOID = new Set(['meta', 'link', 'input', 'br', 'img', 'hr', 'source']);

/** Named character references the shell's HTML uses. */
const ENTITIES = { amp: '&', lt: '<', gt: '>', quot: '"', apos: "'", nbsp: ' ' };

/** Decodes character references in text or an attribute value. */
function decode(text) {
  return text.replace(/&(#x?[0-9a-f]+|\w+);/gi, (whole, ref) => {
    if (ref[0] !== '#') return ENTITIES[ref] ?? whole;
    const hex = ref[1] === 'x' || ref[1] === 'X';
    return String.fromCodePoint(parseInt(ref.slice(hex ? 2 : 1), hex ? 16 : 10));
  });
}

/** Escapes text for serialization. */
const escapeText = (text) => text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

/** Escapes an attribute value for serialization. */
const escapeAttr = (text) => text.replace(/&/g, '&amp;').replace(/"/g, '&quot;');

/** data-foo-bar ⇄ fooBar. */
const toData = (key) => 'data-' + key.replace(/[A-Z]/g, (c) => '-' + c.toLowerCase());
/** fooBar from data-foo-bar. */
const fromData = (name) => name.slice('data-'.length).replace(/-([a-z])/g, (_, c) => c.toUpperCase());

/** Anything in the tree: an element or a text node. */
class Node {
  /** A detached node owned by `document`. */
  constructor(document) {
    /** The owning document. */
    this.ownerDocument = document;
    /** The parent, when attached. */
    this.parentNode = null;
  }

  /** Whether the node is in the document. */
  get isConnected() {
    let node = this;
    while (node.parentNode) node = node.parentNode;
    return node === this.ownerDocument;
  }

  /** Detaches the node. */
  remove() {
    this.parentNode?.removeChild(this);
  }
}

/** A text node. */
class Text extends Node {
  /** Holds `data`. */
  constructor(document, data) {
    super(document);
    /** The text. */
    this.data = String(data);
  }

  /** As the DOM's. */
  get textContent() {
    return this.data;
  }

  /** As the DOM's. */
  set textContent(value) {
    this.data = String(value);
  }

  /** Serialized form. */
  get outerHTML() {
    return escapeText(this.data);
  }
}

/** A class list backed by the class attribute. */
class ClassList {
  /** For `element`. */
  constructor(element) {
    /** The element. */
    this.element = element;
  }

  /** The current classes. */
  get items() {
    return (this.element.getAttribute('class') || '').split(/\s+/).filter(Boolean);
  }

  /** Replaces the classes. */
  set(items) {
    this.element.setAttribute('class', [...new Set(items)].join(' '));
  }

  /** As the DOM's. */
  add(...names) {
    this.set([...this.items, ...names]);
  }

  /** As the DOM's. */
  remove(...names) {
    this.set(this.items.filter((c) => !names.includes(c)));
  }

  /** As the DOM's. */
  contains(name) {
    return this.items.includes(name);
  }

  /** As the DOM's, with the optional force argument. */
  toggle(name, force) {
    const on = force === undefined ? !this.contains(name) : !!force;
    if (on) this.add(name);
    else this.remove(name);
    return on;
  }
}

/** Properties that reflect a boolean attribute. */
const BOOLEAN_PROPS = ['hidden', 'disabled', 'readOnly', 'checked', 'inert'];
/** Properties that reflect a string attribute. */
const STRING_PROPS = {
  id: 'id',
  className: 'class',
  title: 'title',
  placeholder: 'placeholder',
  type: 'type',
  src: 'src',
};

/** An element. */
class Element extends Node {
  /** A `tag` element. */
  constructor(document, tag) {
    super(document);
    /** Lower-case tag name. */
    this.localName = tag.toLowerCase();
    /** Attributes by name. */
    this.attributes = new Map();
    /** Child nodes. */
    this.childNodes = [];
    /** Listeners by type: [{ fn, capture, once }]. */
    this.listeners = {};
    /** Inline style, with setProperty for custom properties. */
    this.style = { setProperty: (name, value) => (this.style[name] = value) };
    /** As the DOM's; a test may set it. */
    Object.assign(this, { scrollTop: 0, scrollLeft: 0, scrollHeight: 0, clientHeight: 0, clientWidth: 0 });
    Object.assign(this, { offsetLeft: 0, offsetWidth: 0, width: 0 });
  }

  /** Upper-case tag name. */
  get tagName() {
    return this.localName.toUpperCase();
  }

  /** As the DOM's. */
  getAttribute(name) {
    return this.attributes.has(name) ? this.attributes.get(name) : null;
  }

  /** As the DOM's. */
  setAttribute(name, value) {
    this.attributes.set(name, String(value));
  }

  /** As the DOM's. */
  removeAttribute(name) {
    this.attributes.delete(name);
  }

  /** As the DOM's. */
  hasAttribute(name) {
    return this.attributes.has(name);
  }

  /** As the DOM's. */
  toggleAttribute(name, force) {
    const on = force === undefined ? !this.hasAttribute(name) : !!force;
    if (on) this.setAttribute(name, '');
    else this.removeAttribute(name);
    return on;
  }

  /** As the DOM's. */
  get classList() {
    return new ClassList(this);
  }

  /** data-* attributes as camelCase properties. */
  get dataset() {
    const element = this;
    return new Proxy(
      {},
      {
        get: (_, key) => element.getAttribute(toData(String(key))) ?? undefined,
        set: (_, key, value) => (element.setAttribute(toData(String(key)), value), true),
        deleteProperty: (_, key) => (element.removeAttribute(toData(String(key))), true),
        has: (_, key) => element.hasAttribute(toData(String(key))),
        ownKeys: () => [...element.attributes.keys()].filter((n) => n.startsWith('data-')).map(fromData),
        getOwnPropertyDescriptor: () => ({ enumerable: true, configurable: true }),
      },
    );
  }

  /** Child elements only. */
  get children() {
    return this.childNodes.filter((n) => n instanceof Element);
  }

  /** As the DOM's. */
  get firstChild() {
    return this.childNodes[0] ?? null;
  }

  /** As the DOM's. */
  get parentElement() {
    return this.parentNode instanceof Element ? this.parentNode : null;
  }

  /** As the DOM's. */
  appendChild(node) {
    node.remove();
    node.parentNode = this;
    this.childNodes.push(node);
    return node;
  }

  /** As the DOM's; strings become text. */
  append(...nodes) {
    for (const n of nodes) this.appendChild(typeof n === 'string' ? this.ownerDocument.createTextNode(n) : n);
  }

  /** As the DOM's. */
  removeChild(node) {
    this.childNodes = this.childNodes.filter((n) => n !== node);
    node.parentNode = null;
    return node;
  }

  /** As the DOM's. */
  replaceChildren(...nodes) {
    for (const n of [...this.childNodes]) this.removeChild(n);
    this.append(...nodes);
  }

  /** As the DOM's. */
  get textContent() {
    return this.childNodes.map((n) => n.textContent).join('');
  }

  /** As the DOM's. */
  set textContent(value) {
    this.replaceChildren();
    if (value !== '' && value != null) this.appendChild(this.ownerDocument.createTextNode(value));
  }

  /** Serialized children. */
  get innerHTML() {
    return this.childNodes.map((n) => n.outerHTML).join('');
  }

  /** Parses `html` into the children. */
  set innerHTML(html) {
    this.replaceChildren();
    parseInto(this, String(html));
  }

  /** Serialized element. */
  get outerHTML() {
    const attrs = [...this.attributes].map(([k, v]) => (v === '' ? ` ${k}` : ` ${k}="${escapeAttr(v)}"`)).join('');
    if (VOID.has(this.localName)) return `<${this.localName}${attrs}>`;
    return `<${this.localName}${attrs}>${this.innerHTML}</${this.localName}>`;
  }

  /** Every descendant element, in document order. */
  descendants() {
    return this.children.flatMap((c) => [c, ...c.descendants()]);
  }

  /** As the DOM's. */
  querySelectorAll(selector) {
    return this.descendants().filter((el) => matches(el, selector));
  }

  /** As the DOM's. */
  querySelector(selector) {
    return this.descendants().find((el) => matches(el, selector)) ?? null;
  }

  /** As the DOM's. */
  matches(selector) {
    return matches(this, selector);
  }

  /** As the DOM's. */
  closest(selector) {
    for (let el = this; el instanceof Element; el = el.parentNode) if (matches(el, selector)) return el;
    return null;
  }

  /** As the DOM's. */
  addEventListener(type, fn, options) {
    const capture = options === true || !!options?.capture;
    (this.listeners[type] ||= []).push({ fn, capture, once: !!options?.once });
  }

  /** As the DOM's. */
  removeEventListener(type, fn) {
    this.listeners[type] = (this.listeners[type] || []).filter((l) => l.fn !== fn);
  }

  /** Dispatches `event` through capture, target and bubble phases. */
  dispatchEvent(event) {
    return dispatch(this, event);
  }

  /** Sends a click. */
  click() {
    if (!this.disabled) this.dispatchEvent(new Event('click'));
  }

  /** Makes this the active element. */
  focus() {
    this.ownerDocument.activeElement = this;
  }

  /** Gives focus back to the body. */
  blur() {
    if (this.ownerDocument.activeElement === this) this.ownerDocument.activeElement = this.ownerDocument.body;
  }

  /** As the DOM's. */
  select() {}

  /** As the DOM's. */
  scrollIntoView() {}

  /** As the DOM's. */
  setPointerCapture() {}

  /** Resolves at once: images decode instantly here. */
  decode() {
    return Promise.resolve();
  }

  /** A box when the element is shown, none when hidden. */
  getClientRects() {
    return this.hidden ? [] : [{}];
  }

  /** The element's `width`. */
  getBoundingClientRect() {
    return { width: this.width };
  }

  /** The tabindex attribute as a number (-1 when absent). */
  get tabIndex() {
    return Number(this.getAttribute('tabindex') ?? -1);
  }

  /** Sets the tabindex attribute. */
  set tabIndex(value) {
    this.setAttribute('tabindex', value);
  }

  /** A form control's value: set explicitly, else its attribute, else a select's first option. */
  get value() {
    if (this._value !== undefined) return this._value;
    if (this.localName === 'select') return this.querySelector('option')?.value ?? '';
    if (this.localName === 'option') return this.getAttribute('value') ?? this.textContent;
    return this.getAttribute('value') ?? '';
  }

  /** As the DOM's. */
  set value(value) {
    this._value = String(value);
  }
}
for (const prop of BOOLEAN_PROPS) {
  Object.defineProperty(Element.prototype, prop, {
    get() {
      return this.hasAttribute(prop.toLowerCase());
    },
    set(value) {
      this.toggleAttribute(prop.toLowerCase(), !!value);
    },
  });
}
for (const [prop, attr] of Object.entries(STRING_PROPS)) {
  Object.defineProperty(Element.prototype, prop, {
    get() {
      return this.getAttribute(attr) ?? '';
    },
    set(value) {
      this.setAttribute(attr, value);
    },
  });
}

/** A DOM event. */
class Event {
  /** A `type` event; `init` adds fields such as key or bubbles: false. */
  constructor(type, init = {}) {
    Object.assign(this, { bubbles: true, ...init, type, defaultPrevented: false });
    /** Set by stopPropagation. */
    this.stopped = false;
    /** Set by stopImmediatePropagation. */
    this.stoppedNow = false;
  }

  /** As the DOM's. */
  preventDefault() {
    this.defaultPrevented = true;
  }

  /** As the DOM's. */
  stopPropagation() {
    this.stopped = true;
  }

  /** As the DOM's. */
  stopImmediatePropagation() {
    this.stopped = true;
    this.stoppedNow = true;
  }
}

/** Calls the listeners on `node` for one phase. */
function fire(node, event, capture) {
  for (const listener of [...(node.listeners[event.type] || [])]) {
    if (event.stoppedNow) return;
    if (capture !== null && listener.capture !== capture) continue;
    if (listener.once) node.removeEventListener(event.type, listener.fn);
    event.currentTarget = node;
    listener.fn.call(node, event);
  }
}

/** Capture from the top, the target, then bubbling back up. */
function dispatch(target, event) {
  event.target ??= target;
  const path = [];
  for (let n = target.parentNode; n; n = n.parentNode) path.unshift(n);
  for (const node of path) if (!event.stopped) fire(node, event, true);
  if (!event.stopped) fire(target, event, null);
  if (event.bubbles) for (const node of path.reverse()) if (!event.stopped) fire(node, event, false);
  return !event.defaultPrevented;
}

/** One compound selector (no combinators) against one element. */
function matchesCompound(el, compound) {
  const parts = compound.match(/(:not\(:disabled\))|(\[[^\]]+\])|([#.]?[\w-]+)|(\*)/g) || [];
  return parts.every((part) => matchesPart(el, part));
}

/** One simple selector against one element. */
function matchesPart(el, part) {
  if (part === '*') return true;
  if (part === ':not(:disabled)') return !el.disabled;
  if (part[0] === '#') return el.id === part.slice(1);
  if (part[0] === '.') return el.classList.contains(part.slice(1));
  if (part[0] !== '[') return el.localName === part.toLowerCase();
  const [, name, value] = part.match(/^\[([\w-]+)(?:=["']?([^"'\]]*)["']?)?\]$/);
  if (value === undefined) return el.hasAttribute(name);
  return name === 'type' && el.localName === 'input'
    ? (el.getAttribute('type') || 'text') === value
    : el.getAttribute(name) === value;
}

/** A selector with descendant combinators against one element. */
function matchesChain(el, chain) {
  const [last, ...rest] = chain.trim().split(/\s+/).reverse();
  if (!matchesCompound(el, last)) return false;
  let node = el.parentNode;
  for (const compound of rest) {
    while (node instanceof Element && !matchesCompound(node, compound)) node = node.parentNode;
    if (!(node instanceof Element)) return false;
    node = node.parentNode;
  }
  return true;
}

/** A selector list against one element. */
function matches(el, selector) {
  return selector.split(',').some((chain) => matchesChain(el, chain));
}

/** Parses attributes from a start tag's inside. */
function parseAttributes(text, el) {
  for (const [, name, dq, sq, bare] of text.matchAll(/([^\s=/]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g)) {
    el.setAttribute(name.toLowerCase(), decode(dq ?? sq ?? bare ?? ''));
  }
}

/** Parses `html` and appends the result to `parent`. */
function parseInto(parent, html) {
  const document = parent.ownerDocument;
  const stack = [parent];
  const top = () => stack.at(-1);
  const tokens = html.matchAll(/<!--[\s\S]*?-->|<!doctype[^>]*>|<\/([\w-]+)\s*>|<([\w-]+)([^>]*?)(\/?)>|([^<]+|<)/gi);
  for (const [, close, open, attrs, selfClose, text] of tokens) {
    if (text !== undefined) top().appendChild(document.createTextNode(decode(text)));
    else if (close) {
      const index = stack.findLastIndex((el) => el.localName === close.toLowerCase());
      if (index > 0) stack.length = index;
    } else if (open) {
      const el = document.createElement(open);
      parseAttributes(attrs, el);
      top().appendChild(el);
      if (!selfClose && !VOID.has(el.localName)) stack.push(el);
    }
  }
}

/** The document: the root of the tree. */
class Document extends Element {
  /** An empty document with html, head and body. */
  constructor() {
    super(null, '#document');
    this.ownerDocument = this;
    /** The <html> element. */
    this.documentElement = this.createElement('html');
    /** The <body> element. */
    this.body = this.createElement('body');
    this.appendChild(this.documentElement);
    this.documentElement.appendChild(this.body);
    /** The focused element. */
    this.activeElement = this.body;
    /** The window title. */
    this.title = '';
  }

  /** As the DOM's. */
  createElement(tag) {
    return new Element(this, tag);
  }

  /** As the DOM's. */
  createTextNode(text) {
    return new Text(this, text);
  }

  /** As the DOM's. */
  getElementById(id) {
    return this.descendants().find((el) => el.id === id) ?? null;
  }
}

/** A document whose body is the body of `html` (with the body's own attributes). */
function documentFrom(html) {
  const document = new Document();
  const body = html.match(/<body([^>]*)>([\s\S]*)<\/body>/i);
  parseAttributes(body[1], document.body);
  parseInto(document.body, body[2]);
  return document;
}

module.exports = { Document, Element, Event, documentFrom, parseInto };
