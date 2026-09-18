/**
 * Unit tests for scripts/workflow/locators.cjs: which locators a recorded
 * element gets, in which order, and the Playwright code for each.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { candidates, locatorCode } = require('../../../../scripts/workflow/locators.cjs');

describe('candidates', () => {
  it('puts a field’s id and name before its text', () => {
    assert.deepEqual(candidates({ type: 'input', text: 'Hint', domId: 'first', name: 'f' }), [
      { kind: 'css', value: '[id="first"]' },
      { kind: 'css', value: '[name="f"]' },
      { kind: 'label', value: 'Hint' },
    ]);
  });

  it('puts a button’s accessible name before its attributes', () => {
    assert.deepEqual(candidates({ type: 'button', role: 'button', text: 'Save', domId: 'b1' }), [
      { kind: 'role', role: 'button', value: 'Save' },
      { kind: 'text', value: 'Save' },
      { kind: 'css', value: '[id="b1"]' },
    ]);
  });

  it('always leads with a test id', () => {
    assert.deepEqual(candidates({ type: 'input', testId: 't', domId: 'd' })[0], { kind: 'testId', value: 't' });
  });

  it('prefers the aria label as the role name and adds a placeholder and link target', () => {
    const out = candidates({ type: 'link', role: 'link', ariaLabel: 'Home', text: 'H', placeholder: 'p', href: '/h' });
    assert.deepEqual(out, [
      { kind: 'role', role: 'link', value: 'Home' },
      { kind: 'label', value: 'Home' },
      { kind: 'text', value: 'H' },
      { kind: 'placeholder', value: 'p' },
      { kind: 'css', value: 'a[href="/h"]' },
    ]);
  });

  it('gives nothing for an element with no handles', () => {
    assert.deepEqual(candidates(), []);
  });
});

describe('locatorCode', () => {
  it('writes each kind as its Playwright call', () => {
    assert.equal(locatorCode({ kind: 'css', value: '#a' }), 'p.locator("#a")');
    assert.equal(
      locatorCode({ kind: 'role', role: 'button', value: 'Go' }),
      'p.getByRole("button", {name: "Go", exact:true})',
    );
    assert.equal(locatorCode({ kind: 'testId', value: 't' }), 'p.getByTestId("t")');
    assert.equal(locatorCode({ kind: 'label', value: 'L' }, 'q', 'v'), 'q.getByLabel(v, {exact:true})');
  });

  it('refuses an unknown or missing locator', () => {
    assert.throws(() => locatorCode({ kind: 'xpath', value: '//a' }), /supported locator/);
    assert.throws(() => locatorCode(null), /supported locator/);
  });
});
