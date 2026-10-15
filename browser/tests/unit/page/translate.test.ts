/**
 * Unit tests for src/page/translate.ts: page translation's scripts, run against
 * a small fake page. What is collected and what is skipped, how translations are
 * written back, and that "Show original" restores only what is still ours.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { applyJs, collectJs, detectJs, restoreJs } from '../../../src/page/translate.ts';

/** A fake text node. */
class Text {
  /** Element (1) or text (3). */
  nodeType = 3;
  /** Its parent, while attached. */
  parentNode: El | null = null;
  /** Its text. */
  data: string;
  /** Holds `data`. */
  constructor(data: string) {
    this.data = data;
  }
  /** Whether it is still in the page. */
  get isConnected(): boolean {
    return !!this.parentNode?.isConnected;
  }
}

/** A fake element: a tag, attributes, children, and whether it is visible or editable. */
class El {
  /** Element (1). */
  nodeType = 1;
  /** Its parent, while attached. */
  parentNode: El | null = null;
  /** Whether it is the page's root. */
  root = false;
  /** Its tag, upper case. */
  tagName: string;
  /** Its attributes. */
  attrs: Record<string, string>;
  /** Its children. */
  childNodes: Array<El | Text>;
  /** As the DOM's. */
  isContentEditable = false;
  /** Its computed display. */
  display = 'block';
  /** Builds `<tag attrs>children</tag>`. */
  constructor(tag: string, attrs: Record<string, string>, children: Array<El | Text>) {
    this.tagName = tag.toUpperCase();
    this.attrs = attrs;
    this.childNodes = children;
    for (const c of children) c.parentNode = this;
  }
  /** As the DOM's. */
  getAttribute(name: string): string | null {
    return this.attrs[name] ?? null;
  }
  /** As the DOM's. */
  get classList() {
    return { contains: (c: string) => (this.attrs.class || '').split(' ').includes(c) };
  }
  /** Whether it is in the page. */
  get isConnected(): boolean {
    return this.root || !!this.parentNode?.isConnected;
  }
  /** Detaches a child. */
  remove(child: El | Text): void {
    this.childNodes = this.childNodes.filter((c) => c !== child);
    child.parentNode = null;
  }
}

/** `<tag>` with attributes and children (strings become text nodes). */
const el = (tag: string, attrs: Record<string, string> = {}, ...children: Array<El | Text | string>) =>
  new El(
    tag,
    attrs,
    children.map((c) => (typeof c === 'string' ? new Text(c) : c)),
  );

/** Installs a page with `body`, `<html>` attributes, and its metas by selector, as the script's globals. */
function page(body: El, lang: string | null = null, meta: string | null = null, htmlAttrs = {}, metas = {}) {
  body.root = true;
  const html = el('html', { ...(lang === null ? {} : { lang }), ...htmlAttrs });
  const found = { 'meta[http-equiv="content-language" i]': meta, ...metas };
  const querySelector = (s: string) => (found[s] == null ? null : { getAttribute: () => found[s] });
  const getComputedStyle = (e: El) => ({ display: e.display });
  Object.assign(globalThis, {
    getComputedStyle,
    MutationObserver: FakeObserver,
    document: { body, documentElement: html, querySelector },
  });
  return body;
}

/** The page's MutationObserver: a test calls `added` to say the page inserted nodes. */
class FakeObserver {
  /** The live observer, if one is watching. */
  static live: FakeObserver | null = null;
  /** What it calls with mutation records. */
  callback: (records: unknown[]) => void;
  /** Keeps `callback`. */
  constructor(callback: (records: unknown[]) => void) {
    this.callback = callback;
  }
  /** Starts watching. */
  observe(): void {
    FakeObserver.live = this;
  }
  /** Stops watching. */
  disconnect(): void {
    FakeObserver.live = null;
  }
  /** The page inserted `nodes` under `parent`. */
  static added(parent: El, ...nodes: Array<El | Text>): void {
    for (const n of nodes) {
      parent.childNodes.push(n);
      n.parentNode = parent;
    }
    FakeObserver.live?.callback([{ addedNodes: nodes }]);
  }
}

/** Runs a page script. */
const run = <T = any>(js: string): T => (0, eval)(js);

/** The texts of the body's text nodes, in order. */
const texts = (node: El): string[] => node.childNodes.flatMap((c) => (c instanceof Text ? [c.data] : texts(c)));

describe('page translation scripts', () => {
  afterEach(() => {
    delete (globalThis as any).document;
    delete (globalThis as any).getComputedStyle;
    delete (globalThis as any).MutationObserver;
    FakeObserver.live = null;
    delete (globalThis as any).__oyaTranslation;
  });

  it('detects the primary html lang tag, then the content-language meta, and nothing when malformed', () => {
    page(el('body'), 'ja-JP');
    assert.equal(run(detectJs), 'ja-JP');
    page(el('body'), null, 'de, en');
    assert.equal(run(detectJs), 'de');
    page(el('body'), 'not a tag!');
    assert.equal(run(detectJs), '');
    page(el('body'));
    assert.equal(run(detectJs), '');
    page(el('body'), 'ja', null, { translate: 'no' });
    assert.equal(run(detectJs), '');
    page(el('body', { class: 'notranslate' }), 'ja');
    assert.equal(run(detectJs), '');
    const editable = page(el('body'), 'ja');
    editable.isContentEditable = true;
    assert.equal(run(detectJs), '');
    for (const none of ['und', 'zxx', 'mul-Latn']) {
      page(el('body'), none);
      assert.equal(run(detectJs), '', none);
    }
  });

  it('collects text inside a display:contents wrapper', () => {
    const wrapper = el('div', {}, el('span', {}, 'Inhalt'));
    wrapper.display = 'contents';
    page(el('body', {}, wrapper));
    assert.deepEqual(run(collectJs(1000, 100)).texts, ['Inhalt']);
  });

  it('collects nothing from a page that opts out as a whole, or whose body is editable', () => {
    const editable = el('body', {}, 'Entwurf');
    editable.isContentEditable = true;
    const cases: Array<[El, Record<string, string>, Record<string, string>]> = [
      [el('body', {}, 'Text'), { translate: 'no' }, {}],
      [el('body', { class: 'notranslate' }, 'Text'), {}, {}],
      [el('body', {}, 'Text'), {}, { 'meta[name="google" i][content="notranslate" i]': 'notranslate' }],
      [editable, {}, {}],
    ];
    for (const [body, htmlAttrs, metas] of cases) {
      page(body, 'ja', null, htmlAttrs, metas);
      assert.deepEqual(run(collectJs(1000, 100)).texts, []);
    }
  });

  it('collects visible text and skips code, scripts, fields, translate=no, notranslate, editable and hidden text', () => {
    const hidden = el('div', {}, 'Versteckt');
    hidden.display = 'none';
    const editable = el('div', {}, 'Bearbeitbar');
    editable.isContentEditable = true;
    page(
      el(
        'body',
        {},
        el('p', {}, '  Hallo  '),
        el('script', {}, 'var x'),
        el('code', {}, 'Code'),
        el('textarea', {}, 'Feld'),
        el('span', { translate: 'no' }, 'Marke'),
        el('span', { class: 'a notranslate' }, 'Name'),
        editable,
        hidden,
        el('p', {}, '1234 — 56'),
        el('p', {}, 'Welt'),
      ),
    );
    assert.deepEqual(run(collectJs(1000, 100)), { texts: ['Hallo', 'Welt'], partial: false });
  });

  it('stops at the page limit and says the page was translated in part', () => {
    page(el('body', {}, el('p', {}, 'Eins'), el('p', {}, 'Zwei'), el('p', {}, 'Drei')));
    assert.deepEqual(run(collectJs(8, 100)), { texts: ['Eins', 'Zwei'], partial: true });
  });

  it('leaves out a text longer than one batch and says the page was translated in part', () => {
    page(el('body', {}, el('p', {}, 'x'.repeat(20)), el('p', {}, 'Kurz')));
    assert.deepEqual(run(collectJs(1000, 10)), { texts: ['Kurz'], partial: true });
  });

  it('writes translations as text, keeping each node’s own leading and trailing whitespace', () => {
    const body = page(el('body', {}, el('p', {}, '  Hallo\n'), el('p', {}, 'Welt')));
    run(collectJs(1000, 100));
    assert.equal(run(applyJs(0, ['Hello', ' World '])), 2);
    assert.deepEqual(texts(body), ['  Hello\n', 'World']);
  });

  it('writes from the batch’s first index', () => {
    const body = page(el('body', {}, el('p', {}, 'Eins'), el('p', {}, 'Zwei')));
    run(collectJs(1000, 100));
    run(applyJs(1, ['Two']));
    assert.deepEqual(texts(body), ['Eins', 'Two']);
  });

  it('keeps model text that looks like code or markup as plain text', () => {
    const hostile = 'a"b\\c</script><img src=x onerror=alert(1)> ${x}`';
    const body = page(el('body', {}, el('p', {}, 'Hallo')));
    run(collectJs(1000, 100));
    run(applyJs(0, [hostile]));
    assert.deepEqual(texts(body), [hostile]);
  });

  it('skips a node the page changed or removed since it was collected', () => {
    const p = el('p', {}, 'Zwei');
    const body = page(el('body', {}, el('p', {}, 'Eins'), p));
    run(collectJs(1000, 100));
    ((body.childNodes[0] as El).childNodes[0] as Text).data = 'Neu';
    body.remove(p);
    assert.equal(run(applyJs(0, ['One', 'Two'])), 0);
  });

  it('restores the original text, but leaves a node the page changed after translation as the page has it', () => {
    const body = page(el('body', {}, el('p', {}, 'Eins'), el('p', {}, 'Zwei')));
    run(collectJs(1000, 100));
    run(applyJs(0, ['One', 'Two']));
    ((body.childNodes[1] as El).childNodes[0] as Text).data = 'Live';
    assert.equal(run(restoreJs), 1);
    assert.deepEqual(texts(body), ['Eins', 'Live']);
  });

  it('sends a text the page repeats once, and writes every copy of it', () => {
    const body = page(el('body', {}, el('p', {}, 'Nachricht'), el('p', {}, 'Nachricht'), el('p', {}, 'Ende')));
    assert.deepEqual(run(collectJs(1000, 100)), { texts: ['Nachricht', 'Ende'], partial: false });
    assert.equal(run(applyJs(0, ['News', 'End'])), 3);
    assert.deepEqual(texts(body), ['News', 'News', 'End']);
  });

  it('translates nodes the page adds later with a known text, and nothing else', () => {
    const body = page(el('body', {}, el('p', {}, 'Folie')));
    run(collectJs(1000, 100));
    run(applyJs(0, ['Slide']));
    FakeObserver.added(body, el('p', {}, ' Folie '), el('p', {}, 'Neu'));
    assert.deepEqual(texts(body), ['Slide', ' Slide ', 'Neu']);
  });

  it('restores the page’s copies of translated nodes, and stops translating added nodes', () => {
    const body = page(el('body', {}, el('p', {}, 'Folie')));
    run(collectJs(1000, 100));
    run(applyJs(0, ['Slide']));
    FakeObserver.added(body, el('p', {}, 'Slide'));
    assert.equal(run(restoreJs), 2);
    assert.deepEqual(texts(body), ['Folie', 'Folie']);
    assert.equal(FakeObserver.live, null);
  });

  it('restores nothing, and fails nothing, when nothing was collected', () => {
    page(el('body', {}, el('p', {}, 'Eins')));
    assert.equal(run(restoreJs), 0);
    assert.equal(run(applyJs(0, ['One'])), 0);
  });
});
