/**
 * Unit tests for native keyboard input: key definitions, key events, typing with
 * Shift, and clearing a field with the platform's select-all.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { keyDef, Keyboard } from '../../../../src/main/input/keyboard.ts';
import { Modifier } from '../../../../src/main/input/constants.ts';
import { instantTimers, fixedRandom, pageView } from '../../support/page.cjs';

/** A keyboard on macOS unless told otherwise. */
const keyboard = (platform: NodeJS.Platform = 'darwin') => new Keyboard(platform);

/** The Input.dispatchKeyEvent params sent, in order. */
const keyEvents = (view: any) =>
  view.webContents.calls
    .filter((c: any) => c[0] === 'input' && ['keyDown', 'keyUp'].includes(c[1].type))
    .map((c: any) => c[1]);

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

describe('native keyboard', () => {
  afterEach(() => mock.restoreAll());

  it('a press is a down then an up, held for a moment', async () => {
    const delays = instantTimers();
    fixedRandom(0);
    const view = pageView();
    await keyboard().press(view, 'Tab', Modifier.CTRL);
    assert.deepEqual(
      keyEvents(view).map((e: any) => [e.type, e.keyCode, e.modifiers]),
      // keyDown, not rawKeyDown: a raw down never became a DOM keydown through
      // Electron's debugger, so no key with no text of its own reached the page.
      [
        ['keyDown', 'Tab', ['control']],
        ['keyUp', 'Tab', ['control']],
      ],
    );
    assert.deepEqual(delays, [20]);
  });

  it('a character key down carries its text', async () => {
    instantTimers();
    const view = pageView();
    await keyboard().press(view, 'x');
    const [down] = keyEvents(view);
    assert.equal(down.type, 'keyDown');
    assert.equal(down.keyCode, 'x');
    assert.deepEqual(view.webContents.calls[1], ['input', { type: 'char', keyCode: 'x', modifiers: [] }]);
  });

  it('typing sends each character with Shift where needed', async () => {
    instantTimers();
    fixedRandom(0.5);
    const view = pageView();
    await keyboard().type(view, 'aB');
    const downs = keyEvents(view).filter((e: any) => e.type === 'keyDown');
    assert.deepEqual(
      downs.map((e: any) => [e.keyCode, e.modifiers]),
      [
        ['a', []],
        ['B', ['shift']],
      ],
    );
  });

  for (const platform of ['darwin', 'win32'] as const) {
    it(`clearing selects all with ${platform}'s modifier, then deletes`, async () => {
      instantTimers();
      const view = pageView();
      await keyboard(platform).clear(view);
      assert.deepEqual(view.webContents.calls[0], ['selectAll']);
      assert.deepEqual(
        keyEvents(view).map((e: any) => [e.type, e.keyCode, e.modifiers]),
        [
          ['keyDown', 'Backspace', []],
          ['keyUp', 'Backspace', []],
        ],
      );
    });
  }

  it('a destroyed view rejects', async () => {
    const view = pageView();
    view.webContents.destroyed = true;
    await assert.rejects(keyboard().press(view, 'a'), /destroyed/);
  });
});
