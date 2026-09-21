/**
 * Unit tests for the page scripts: values are inserted exactly where the
 * script text expects them, and actions without a handler map to a script.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const s = require('../../../../main/actions/scripts.cjs');

describe('page scripts', () => {
  it('findElementJs quotes the selector as code and escapes it in the message', () => {
    const js = s.findElementJs("a[title='x']");
    assert.ok(js.includes('const el = f("a[title=\'x\']");'));
    assert.ok(js.includes("error: 'Element not found: a[title=\\'x\\']'"));
  });

  it('findElementJs keeps a backslash or line break in the selector from breaking the script', () => {
    for (const selector of ['a\\', "a\\'b", 'a\nb']) {
      assert.doesNotThrow(() => new Function(s.findElementJs(selector)), JSON.stringify(selector));
    }
  });

  it('inserted values are taken literally, replacement patterns included', () => {
    const js = s.selectOptionJs('#s', '$& $1');
    assert.ok(js.includes('el.value = "$& $1";'));
    assert.ok(js.includes('const el = f("#s");'));
  });

  it('every script is valid JavaScript', () => {
    for (const js of [
      s.findElementJs('a'),
      s.iframeClickJs('a'),
      s.selectFieldJs('a'),
      s.selectOptionJs('a', 'b'),
      s.devWaitJs({ selector: 'a' }),
      s.scrollResultJs({ direction: 'up' }, 5),
      s.DROPDOWN_JS,
      s.DEV_ANALYZE_JS,
    ]) {
      assert.doesNotThrow(() => new Function(js), js.slice(0, 60));
    }
  });

  it('the wait script defaults its timeout', () => {
    assert.ok(s.actionScript('wait', { selector: 'a' }).includes('const maxWait = 10000;'));
    assert.ok(s.actionScript('wait', { selector: 'a', timeout: 5 }).includes('const maxWait = 5;'));
  });

  it('a wait timeout is only ever a number, so caller text cannot become code', () => {
    const hostile = { selector: 'a', timeout: '1;globalThis.pwned=1' };
    for (const js of [s.actionScript('wait', hostile), s.devWaitJs(hostile)]) {
      assert.ok(js.includes('const maxWait = 10000;'), js.slice(0, 120));
      assert.ok(!js.includes('pwned'));
    }
  });

  it('a wait timeout given as digits, zero, a negative or infinity is a number or the default', () => {
    const maxWait = (timeout) => /const maxWait = ([^;]+);/.exec(s.actionScript('wait', { selector: 'a', timeout }))[1];
    assert.deepEqual(['250', 0, -5, Infinity, null].map(maxWait), ['250', '10000', '10000', '10000', '10000']);
  });

  it('the scroll script writes its amount as a number whatever it is handed', () => {
    const js = s.scrollResultJs({ direction: 'up' }, '1}}),(globalThis.pwned=1),({a:{b:1');
    assert.ok(js.endsWith('amount: 0 } }'), js.slice(-60));
    assert.ok(s.scrollResultJs({}, 250).endsWith('amount: 250 } }'));
  });

  it('analyze passes its params to the analyzer', () => {
    assert.ok(s.actionScript('analyze', { a: 1 }).includes('analyzePage({"a":1})'));
  });

  it('an unknown action, even a prototype name, has no script at all', () => {
    assert.equal(s.actionScript('toString'), null);
  });

  it('the name of an unknown action never reaches a script, whatever it contains', () => {
    assert.equal(s.actionScript("x'}),(globalThis.pwned=1),({a:'", {}), null);
  });
});
