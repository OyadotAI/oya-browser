/**
 * Unit tests for scripts/workflow/locators.cjs: which locators a recorded
 * element gets, in which order, and the Playwright code for each.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { stableId, withoutLiveCount, candidates, locatorCode, roleName } = require('../../../../scripts/workflow/locators.cjs');
const { generate, normalizeDraft } = require('../../../../scripts/workflow.cjs');

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

  it('locates a link by its href as written, since CSS matches the attribute and not the resolved URL', () => {
    const out = candidates({ type: 'link', href: 'https://shop.test/a/b', rawHref: '/a/b' });
    assert.deepEqual(out, [{ kind: 'css', value: 'a[href="/a/b"]' }]);
  });

  it('skips a link target of "#" or a script, which every such link shares', () => {
    assert.deepEqual(candidates({ type: 'link', href: 'https://shop.test/#', rawHref: '#' }), []);
    assert.deepEqual(candidates({ type: 'link', rawHref: 'javascript:void(0)' }), []);
  });

  it('falls back to the recorded position when nothing else identifies the element', () => {
    assert.deepEqual(candidates({ type: 'button', tag: 'div', path: 'body > div:nth-of-type(2)' }), [
      { kind: 'css', value: 'body > div:nth-of-type(2)' },
    ]);
  });

  it('adds a repeated name scoped to its container after the name, and the position last', () => {
    const el = {
      type: 'button',
      text: 'Content',
      scoped: '[data-index="content"] strong:text-is("Content")',
      path: 'body > div',
    };
    assert.deepEqual(candidates(el), [
      { kind: 'text', value: 'Content' },
      { kind: 'css', value: '[data-index="content"] strong:text-is("Content")' },
      { kind: 'css', value: 'body > div' },
    ]);
  });

  it('finds a checkbox in a group by its value first, since grids make up its id', () => {
    assert.deepEqual(candidates({ type: 'checkbox', domId: 'id_912', name: 'reviews', choice: '344' }).slice(0, 2), [
      { kind: 'css', value: 'input[name="reviews"][value="344"]' },
      { kind: 'css', value: '[id="id_912"]' },
    ]);
  });

  it('leads with the link and its stable name when the recorded name carries a live count', () => {
    const el = {
      type: 'link',
      role: 'link',
      text: '1 1 new notification Notifications',
      stableText: 'Notifications',
      rawHref: 'https://in.test/notifications/?',
    };
    assert.deepEqual(candidates(el), [
      { kind: 'css', value: 'a[href="https://in.test/notifications/?"]' },
      { kind: 'css', value: 'a[href^="https://in.test/notifications/"]' },
      { kind: 'text', value: 'Notifications' },
    ]);
  });

  it('puts a link whose name repeats on the page behind its unique target', () => {
    const el = {
      type: 'link',
      role: 'link',
      text: 'View Order',
      repeats: 'true',
      rawHref: '/order/view/order_id/189/',
    };
    assert.deepEqual(candidates(el)[0], { kind: 'css', value: 'a[href="/order/view/order_id/189/"]' });
    assert.equal(candidates(el).at(-1).value, 'View Order');
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
      'p.getByRole("button", {name: named("Go")})',
    );
    assert.equal(locatorCode({ kind: 'testId', value: 't' }), 'p.getByTestId("t")');
    assert.equal(locatorCode({ kind: 'label', value: 'L' }, 'q', 'v'), 'q.getByLabel(v, {exact:true})');
  });

  it('matches a role name exactly, give or take the icons and punctuation a page draws around it', () => {
    const name = roleName('Reports (2)');
    for (const shown of ['Reports (2)', '\ue60a Reports (2)', 'Reports (2) ›']) assert.ok(name.test(shown), shown);
    for (const other of ['My Reports (2)', 'Reports (22)', 'reports (2)']) assert.ok(!name.test(other), other);
  });

  it('gives the generated module the same role-name rule validation uses', () => {
    const code = generate(
      normalizeDraft({ steps: [{ action: 'click', candidates: [{ kind: 'role', role: 'link', value: 'a.b' }] }] }),
    ).code;
    const line = code.split('\n').find((l) => l.includes('const named'));
    const named = new Function(line + '; return named;')();
    for (const shown of ['a.b', '\ue60a a.b', 'axb', 'a.bc'])
      assert.equal(named('a.b').test(shown), roleName('a.b').test(shown), shown);
  });

  it('acts only on visible matches, except a file upload whose input is hidden', () => {
    const code = (action) =>
      generate(normalizeDraft({ steps: [{ action, file: 'f', candidates: [{ kind: 'css', value: '#x' }] }] })).code;
    assert.match(code('click'), /p\.locator\(value\("#x"\)\)\.filter\(\{visible:true\}\)\.click/);
    assert.match(code('upload_file'), /p\.locator\(value\("#x"\)\)\.setInputFiles/);
  });

  it('refuses an unknown or missing locator', () => {
    assert.throws(() => locatorCode({ kind: 'xpath', value: '//a' }), /supported locator/);
    assert.throws(() => locatorCode(null), /supported locator/);
  });
});
describe('handles that only look stable', () => {
  it('keeps an id a person wrote', () => {
    for (const id of ['search-input', 'pagination-next', 'main-content', 'authWizardNextButton'])
      assert.equal(stableId(id), true, id);
  });

  it('demotes an id a framework made up for this render', () => {
    // Parsoid numbers every Wikipedia node; React, Ember, ExtJS and Radix all do their own.
    for (const id of ['mwCg', 'mwAQ', ':r3:', 'ember1204', 'ext-gen1023', 'radix-:r1:', 'user_1234567890'])
      assert.equal(stableId(id), false, id);
  });

  it("prefers a link's target over an id invented for this render", () => {
    const first = candidates({ tag: 'a', domId: 'mwCg', rawHref: '/wiki/Managed_care' })[0];
    assert.deepEqual(first, { kind: 'css', value: 'a[href="/wiki/Managed_care"]' });
  });

  it('still offers the generated id last, when nothing else can find the element', () => {
    const all = candidates({ tag: 'div', domId: 'mwCg' });
    assert.deepEqual(all.at(-1), { kind: 'css', value: '[id="mwCg"]' });
  });

  it('tries where the element sits before an id this render invented', () => {
    // Wikipedia renumbers every node per render; its place in the article does not move.
    const all = candidates({ tag: 'a', domId: 'mwDQ', path: 'div.mw-parser-output > p:nth-child(4) > a' });
    assert.deepEqual(all, [
      { kind: 'css', value: 'div.mw-parser-output > p:nth-child(4) > a' },
      { kind: 'css', value: '[id="mwDQ"]' },
    ]);
  });

  it('strips a live count from a name so a nav link survives a notification', () => {
    assert.equal(withoutLiveCount('Home, 1 new notification'), 'Home');
    assert.equal(withoutLiveCount('Messaging, 0 new notifications'), 'Messaging');
    assert.equal(withoutLiveCount('Inbox (12)'), 'Inbox');
    assert.equal(withoutLiveCount('Page 2'), 'Page 2');
  });
});

