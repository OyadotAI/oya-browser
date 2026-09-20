/**
 * Unit tests for CDP keyboard input: key definitions, key events, typing with
 * Shift, and clearing a field with the platform's select-all.
 */
const { describe, it, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { keyDef, cdpPressKey, cdpTypeText, cdpClearField } = require('../../../../main/input/keyboard.cjs');
const { Modifier } = require('../../../../main/input/constants.cjs');
const { instantTimers, fixedRandom, pageView } = require('../../support/page.cjs');

/** The Input.dispatchKeyEvent params sent, in order. */
const keyEvents = (view) =>
  view.webContents.debugger.sent.filter((call) => call.method === 'Input.dispatchKeyEvent').map((call) => call.params);

describe('keyDef', () => {
  it('named keys come from the table, as copies', () => {
    const enter = keyDef('Enter');
    assert.deepEqual(enter, { key: 'Enter', code: 'Enter', keyCode: 13 });
    enter.keyCode = 0;
    assert.equal(keyDef('Enter').keyCode, 13);
  });

  it('a lower-case letter is its Key code without Shift', () => {
    assert.deepEqual(keyDef('a'), { key: 'a', code: 'KeyA', keyCode: 65, text: 'a', shift: false });
  });

  it('an upper-case letter needs Shift', () => {
    assert.equal(keyDef('Q').shift, true);
  });

  it('a digit is its Digit code', () => {
    assert.deepEqual(keyDef('7'), { key: '7', code: 'Digit7', keyCode: 55, text: '7', shift: false });
  });

  it('any other character is typed as text with no code', () => {
    assert.deepEqual(keyDef('%'), { key: '%', code: '', keyCode: 0, text: '%', shift: false });
  });
});

describe('CDP keyboard', () => {
  afterEach(() => mock.restoreAll());

  it('a press is a raw down then an up, held for a moment', async () => {
    const delays = instantTimers();
    fixedRandom(0);
    const view = pageView();
    await cdpPressKey(view, 'Tab', Modifier.CTRL);
    assert.deepEqual(
      keyEvents(view).map((e) => [e.type, e.key, e.modifiers]),
      [
        ['rawKeyDown', 'Tab', Modifier.CTRL],
        ['keyUp', 'Tab', Modifier.CTRL],
      ],
    );
    assert.deepEqual(delays, [20]);
  });

  it('a character key down carries its text', async () => {
    instantTimers();
    const view = pageView();
    await cdpPressKey(view, 'x');
    const [down] = keyEvents(view);
    assert.equal(down.type, 'keyDown');
    assert.equal(down.text, 'x');
    assert.equal(down.unmodifiedText, 'x');
  });

  it('typing sends each character with Shift where needed', async () => {
    instantTimers();
    fixedRandom(0.5);
    const view = pageView();
    await cdpTypeText(view, 'aB');
    const downs = keyEvents(view).filter((e) => e.type === 'keyDown');
    assert.deepEqual(
      downs.map((e) => [e.key, e.modifiers]),
      [
        ['a', 0],
        ['B', Modifier.SHIFT],
      ],
    );
  });

  it('clearing selects all with the platform modifier, then deletes', async () => {
    instantTimers();
    const view = pageView();
    await cdpClearField(view);
    const modifier = process.platform === 'darwin' ? Modifier.META : Modifier.CTRL;
    assert.deepEqual(
      keyEvents(view).map((e) => [e.type, e.key, e.modifiers]),
      [
        ['keyDown', 'a', modifier],
        ['keyUp', 'a', modifier],
        ['rawKeyDown', 'Backspace', 0],
        ['keyUp', 'Backspace', 0],
      ],
    );
  });

  it('a destroyed view rejects', async () => {
    const view = pageView();
    view.webContents.destroyed = true;
    await assert.rejects(cdpPressKey(view, 'a'), /destroyed/);
  });
});
