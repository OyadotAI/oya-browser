/**
 * Test doubles for the validation worker: a Playwright browser, page and
 * locators that record what they are asked to do, and the utility-process
 * parent port the worker talks through. Hermetic: nothing launches Chromium.
 */
const { EventEmitter } = require('node:events');

/** A locator: counts from the page's table, records actions. */
class FakeLocator {
  /** `desc` names the locator, e.g. `css:#a` or `role:button:Save`. */
  constructor(page, desc) {
    /** The page it belongs to. */
    this.page = page;
    /** How the page's tables refer to it. */
    this.desc = desc;
  }

  /** As Playwright's; matches still come from the description, but the page notes what was narrowed to visible. */
  filter(options) {
    if (options?.visible) this.page.visibleOnly = [...(this.page.visibleOnly || []), this.desc];
    return this;
  }

  /** Matches, from `page.counts` (1 when not listed). */
  async count() {
    return this.page.counts[this.desc] ?? 1;
  }

  /** As Playwright's, for the identity check: what the page says the found element is (consistent when unset). */
  async evaluate() {
    return this.page.identities?.[this.desc] ?? null;
  }

  /** Resolves at once; rejects when the page says this locator never appears. */
  async waitFor() {
    if (this.page.counts[this.desc] === 0) throw new Error('timeout');
  }

  /** A frame inside this one: the chain is kept in the description. */
  frameLocator(selector) {
    return new FakeScope(this.page, `${this.desc}>${selector}`);
  }

  /** Records a click; fails when the page says so. */
  async click() {
    this.page.act('click', this.desc);
  }

  /** Records a fill with its value. */
  async fill(value) {
    this.page.act('fill', this.desc, value);
  }

  /** Records typing key by key. */
  async pressSequentially(text) {
    this.page.act('keys', this.desc, text);
  }

  /** Records a double-click. */
  async dblclick() {
    this.page.act('dblclick', this.desc);
  }

  /** Records a hover. */
  async hover() {
    this.page.act('hover', this.desc);
  }
}

/** Anything locators can be made from: a page or a frame. */
class FakeScope {
  /** `prefix` is prepended to every locator description made here. */
  constructor(page, prefix = '') {
    /** The page. */
    this.page = page;
    /** Frame chain so far. */
    this.prefix = prefix;
  }

  /** A locator named `kind:value` under this scope. */
  make(kind, value) {
    return new FakeLocator(this.page, `${this.prefix ? this.prefix + '|' : ''}${kind}:${value}`);
  }

  /** As Playwright's. */
  locator(css) {
    return this.make('css', css);
  }

  /** As Playwright's. */
  getByRole(role, { name }) {
    // A name pattern from roleName reads back as the text it was made from.
    const text = name instanceof RegExp ? name.source.slice(4, -4).replace(/\\(.)/g, '$1') : name;
    return this.make('role', `${role}:${text}`);
  }

  /** As Playwright's. */
  getByTestId(value) {
    return this.make('testId', value);
  }

  /** As Playwright's. */
  getByLabel(value) {
    return this.make('label', value);
  }

  /** As Playwright's. */
  getByText(value) {
    return this.make('text', value);
  }

  /** As Playwright's. */
  getByPlaceholder(value) {
    return this.make('placeholder', value);
  }

  /** A frame under this scope. */
  frameLocator(selector) {
    return new FakeScope(this.page, `${this.prefix ? this.prefix + '>' : ''}${selector}`);
  }
}

/** A page: emits Playwright page events, records actions in `log`. */
class FakePage extends EventEmitter {
  /** `url` is what url() returns; `counts` sets how many elements each locator matches. */
  constructor(url, { counts = {}, failing = [], context, identities = {} } = {}) {
    super();
    /** The page address. */
    this.address = url;
    /** Locator description → match count. */
    this.counts = counts;
    /** Locator descriptions whose actions throw. */
    this.failing = failing;
    /** Locator description → what the identity check reads about its element. */
    this.identities = identities;
    /** Every action, as [verb, target, value?]. */
    this.log = [];
    /** Its browser context. */
    this.ctx = context;
    /** Keyboard, as Playwright's. */
    this.keyboard = { press: async (key) => this.log.push(['press', key]) };
    /** Mouse, as Playwright's. */
    this.mouse = { wheel: async (x, y) => this.log.push(['wheel', x, y]) };
    Object.assign(this, { scope: new FakeScope(this) });
  }

  /** Records an action, or throws when its target is failing. */
  act(verb, target, value) {
    if (this.failing.includes(target)) throw new Error(`${verb} failed on ${target}`);
    this.log.push(value === undefined ? [verb, target] : [verb, target, value]);
  }

  /** As Playwright's. */
  url() {
    return this.address;
  }

  /** As Playwright's; notes the grace waits, apart from the action log. */
  async waitForTimeout(ms) {
    this.graces = [...(this.graces || []), ms];
  }

  /** As Playwright's; counts the waits for the page to settle, apart from the action log. */
  async waitForLoadState(state) {
    this.settles = [...(this.settles || []), state];
  }

  /** As Playwright's. */
  context() {
    return this.ctx;
  }

  /** As Playwright's. */
  async goto(url) {
    this.log.push(['goto', url]);
  }

  /** As Playwright's. */
  async goBack() {
    this.log.push(['back']);
  }

  /** As Playwright's. */
  async goForward() {
    this.log.push(['forward']);
  }
}
for (const name of [
  'locator',
  'getByRole',
  'getByTestId',
  'getByLabel',
  'getByText',
  'getByPlaceholder',
  'frameLocator',
]) {
  FakePage.prototype[name] = function (...args) {
    return this.scope[name](...args);
  };
}

/** A browser with one context holding `pages`. */
function fakeBrowser(urls, options = {}) {
  const context = new EventEmitter();
  const pages = urls.map((url) => new FakePage(url, { ...options, context }));
  context.pages = () => pages;
  const browser = { pages, closed: 0, contexts: () => [context], close: async () => browser.closed++ };
  return browser;
}

/** The utility process's parent port: `send()` delivers to the worker, `messages` holds what it said. */
class FakeParentPort extends EventEmitter {
  /** Starts with no messages. */
  constructor() {
    super();
    /** Everything the worker posted. */
    this.messages = [];
  }

  /** As Electron's: records the message and wakes anyone waiting for one like it. */
  postMessage(message) {
    this.messages.push(message);
    this.emit('posted', message);
  }

  /** Sends `data` to the worker. */
  send(data) {
    this.emit('message', { data });
  }

  /** Resolves with the first posted message (past or future) that `match` accepts. */
  waitFor(match) {
    const seen = this.messages.find(match);
    if (seen) return Promise.resolve(seen);
    return new Promise((resolve) => {
      const check = (message) => match(message) && (this.off('posted', check), resolve(message));
      this.on('posted', check);
    });
  }

  /** The run events the worker emitted. */
  events() {
    return this.messages.filter((m) => m.type === 'event').map((m) => m.event);
  }
}

module.exports = { FakeLocator, FakePage, fakeBrowser, FakeParentPort };
