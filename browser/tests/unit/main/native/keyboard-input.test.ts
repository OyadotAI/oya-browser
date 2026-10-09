/** External keyboard events retain native shortcut provenance and never fake unsupported physical keys. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { dispatchNativeKey } from '../../../../src/main/native/keyboard-input.ts';
import { isNativeKeyDispatch } from '../../../../src/main/input/index.ts';
import { validateParams } from '../../../../src/main/native-front-door/validation.ts';
/** The native seam records whether the synchronous shortcut grant belongs to the exact view. */
function fixture() {
  const events: any[] = [];
  const page = {
    webContents: {
      isDestroyed: () => false,
      sendInputEvent(event: unknown) {
        assert.equal(isNativeKeyDispatch(this), true);
        events.push(event);
      },
    },
  } as any;
  return { page, events };
}
test('keyDown text, raw keys, characters and releases preserve their distinct semantics', () => {
  const f = fixture();
  dispatchNativeKey(f.page, {
    type: 'keyDown',
    key: 'A',
    code: 'KeyA',
    windowsVirtualKeyCode: 65,
    text: 'A',
    modifiers: 8,
  });
  assert.deepEqual(f.events, [
    { type: 'keyDown', keyCode: 'A', modifiers: ['shift'] },
    { type: 'char', keyCode: 'A', modifiers: ['shift'] },
  ]);
  dispatchNativeKey(f.page, { type: 'rawKeyDown', key: 'ArrowDown', code: 'ArrowDown' });
  assert.equal(f.events.at(-1).keyCode, 'Down');
  dispatchNativeKey(f.page, { type: 'char', text: '!' });
  assert.equal(f.events.at(-1).keyCode, '!');
  dispatchNativeKey(f.page, { type: 'keyUp', key: 'ArrowDown' });
  assert.equal(f.events.at(-1).type, 'keyUp');
  assert.equal(isNativeKeyDispatch(f.page.webContents), false);
});
test('bad metadata or text cannot partially press a key before rejection', () => {
  const f = fixture();
  for (const patch of [
    { type: 'keydown' },
    { key: null },
    { key: 'F24' },
    { key: '🚀' },
    { code: 'KeyB' },
    { windowsVirtualKeyCode: 66 },
    { modifiers: 16 },
    { text: 'two' },
    { text: '日本語' },
    { type: 'rawKeyDown', text: 'a' },
    { type: 'keyUp', text: 'a' },
    { type: 'char' },
  ])
    assert.throws(() => dispatchNativeKey(f.page, { type: 'keyDown', key: 'a', ...patch }));
  assert.deepEqual(f.events, []);
});
test('unsupported physical and system options never disappear at the external boundary', () => {
  for (const extra of [
    { nativeVirtualKeyCode: 4 },
    { location: 2 },
    { autoRepeat: true },
    { isSystemKey: true },
    { commands: ['selectAll'] },
    { timestamp: 1 },
  ])
    assert.throws(() =>
      validateParams({ id: 1, method: 'Input.dispatchKeyEvent', params: { type: 'keyDown', key: 'a', ...extra } }),
    );
});
test('native dispatch failures always revoke the exact-view shortcut grant', () => {
  const f = fixture();
  f.page.webContents.sendInputEvent = () => {
    throw Error('native failure');
  };
  assert.throws(() => dispatchNativeKey(f.page, { type: 'keyDown', key: 'Enter' }), /native failure/);
  assert.equal(isNativeKeyDispatch(f.page.webContents), false);
  f.page.webContents.isDestroyed = () => true;
  assert.throws(() => dispatchNativeKey(f.page, { type: 'keyDown', key: 'Enter' }), /destroyed/);
});

test('space and Enter metadata match their real physical key definitions', () => {
  const f = fixture();
  dispatchNativeKey(f.page, { type: 'keyDown', key: ' ', code: 'Space', windowsVirtualKeyCode: 32, text: ' ' });
  dispatchNativeKey(f.page, { type: 'char', text: '\r', code: 'Enter', windowsVirtualKeyCode: 13 });
  assert.deepEqual(
    f.events.map((e) => e.keyCode),
    ['Space', ' ', '\r'],
  );
});
