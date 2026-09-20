/**
 * Unit tests for scripts/page-queries.cjs: the wait and read_elements page
 * scripts, run against a small fake page with a same-origin iframe.
 */
const { describe, it, afterEach } = require('node:test');
const assert = require('node:assert/strict');
const { presentJs, readElementsJs } = require('../../../scripts/page-queries.cjs');

/** A fake element: a tag, what it says, and whether it takes up space. */
const element = (tag, props = {}) => ({
  tagName: tag.toUpperCase(),
  id: '',
  innerText: '',
  placeholder: '',
  getAttribute: (name) => props.attrs?.[name] ?? null,
  getBoundingClientRect: () => (props.hidden ? { width: 0, height: 0 } : { width: 10, height: 10 }),
  ...props,
});

/** A fake document: `matches` answers each selector, `frames` are its iframes. */
const page = (matches, frames = []) => ({
  querySelector(s) {
    if (s === 'bad[') throw new SyntaxError('bad selector');
    return (matches[s] || [])[0] || null;
  },
  querySelectorAll(s) {
    if (s === 'iframe, frame') return frames.map((doc) => ({ contentDocument: doc }));
    if (s === 'bad[') throw new SyntaxError('bad selector');
    return matches[s] || [];
  },
});

/** Runs a page script against `doc` as the top document. */
function run(js, doc, acFind) {
  Object.assign(globalThis, {
    document: doc,
    location: { href: 'https://p.test/' },
    window: { __acFindElement: acFind },
  });
  doc.title = 'Page';
  return eval(js);
}

describe('presentJs', () => {
  afterEach(() => ['document', 'location', 'window'].forEach((k) => delete globalThis[k]));

  it('finds a CSS selector in the page', () => {
    assert.equal(run(presentJs('button'), page({ button: [element('button')] })), true);
  });

  it('finds a CSS selector inside a same-origin iframe', () => {
    const inner = page({ '#from': [element('input')] });
    assert.equal(run(presentJs('#from'), page({}, [inner])), true);
  });

  it('answers false when nothing matches, or the selector is not valid CSS', () => {
    assert.equal(run(presentJs('.missing'), page({})), false);
    assert.equal(run(presentJs('bad['), page({})), false);
  });

  it('looks up an analyzer id through the analyzer, not as CSS', () => {
    const asked = [];
    const acFind = (s) => (asked.push(s), s === '12' ? {} : null);
    assert.equal(run(presentJs('12'), page({}), acFind), true);
    assert.equal(run(presentJs('[data-ac-id="3"]'), page({}), acFind), false);
    assert.deepEqual(asked, ['12', '[data-ac-id="3"]']);
  });

  it('never treats a CSS selector with a number in it as an analyzer id', () => {
    const acFind = () => assert.fail('the analyzer was asked');
    assert.equal(run(presentJs('#field2'), page({ '#field2': [element('input')] }), acFind), true);
  });
});

describe('readElementsJs', () => {
  afterEach(() => ['document', 'location', 'window'].forEach((k) => delete globalThis[k]));

  it('lists visible matches from the page and its iframes, with the page URL and title', () => {
    const inner = page({ button: [element('button', { innerText: ' Next\n step ' })] });
    const top = page(
      { button: [element('button', { id: 'go', innerText: 'Go' }), element('button', { hidden: true })] },
      [inner],
    );
    assert.deepEqual(run(readElementsJs('button', 10), top), {
      ok: true,
      data: {
        url: 'https://p.test/',
        title: 'Page',
        elements: [
          { tag: 'button', id: 'go', text: 'Go', aria_label: '' },
          { tag: 'button', id: '', text: 'Next step', aria_label: '' },
        ],
      },
    });
  });

  it('stops at the limit', () => {
    const many = Array.from({ length: 5 }, () => element('a'));
    assert.equal(run(readElementsJs('a', 2), page({ a: many })).data.elements.length, 2);
  });

  it('never reads what was typed into an input, only a button value or placeholder', () => {
    const fields = [
      element('input', { type: 'password', value: 'hunter2', placeholder: 'Password' }),
      element('input', { type: 'submit', value: 'Sign in' }),
    ];
    const listed = run(readElementsJs('input', 5), page({ input: fields })).data.elements.map((e) => e.text);
    assert.deepEqual(listed, ['Password', 'Sign in']);
  });

  it('lists interactive elements when no selector is given', () => {
    const js = readElementsJs(undefined, 5);
    assert.ok(js.includes('a[href], button'));
  });

  it('answers an empty list for a selector that is not valid CSS', () => {
    assert.deepEqual(run(readElementsJs('bad[', 5), page({})).data.elements, []);
  });
});
