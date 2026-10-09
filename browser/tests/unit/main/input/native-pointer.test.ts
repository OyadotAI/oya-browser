/** Native input remains usable with debugger access made fatal. */
import { readFileSync } from 'node:fs';
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { nativePointer, nativeWheel } from '../../../../src/main/input/index.ts';
/** No debugger transport is supplied or permitted. */
function fixture(destroyed = false) {
  const events = [];
  const webContents = {
    isDestroyed: () => destroyed,
    sendInputEvent: (event) => events.push(event),
    get debugger() {
      throw new Error('Internal CDP is forbidden');
    },
  };
  return { view: { webContents }, events };
}
it('dispatches native pointer input without consulting a debugger', () => {
  const { view, events } = fixture();
  nativePointer(view, { type: 'mouseDown', x: 10.4, y: 20.6, button: 'left' });
  assert.deepEqual(events, [{ type: 'mouseDown', x: 10, y: 21, button: 'left' }]);
});
it('maps action scroll direction to native wheel direction', () => {
  const { view, events } = fixture();
  nativeWheel(view, 10, 20, 30, 40);
  assert.deepEqual(events, [{ type: 'mouseWheel', x: 10, y: 20, deltaX: -30, deltaY: -40 }]);
});
it('refuses destroyed targets and nonfinite pointer or wheel values', () => {
  assert.throws(() => nativePointer(fixture(true).view, { type: 'mouseMove', x: 1, y: 2 }), /destroyed/);
  assert.throws(() => nativePointer(fixture().view, { type: 'mouseMove', x: NaN, y: 2 }), /finite/);
  assert.throws(() => nativeWheel(fixture().view, 1, 2, Infinity, 0), /finite/);
});

it('native pointer modules cannot import the debugger adapter or dispatch protocol commands', () => {
  for (const file of [
    'input/native-pointer.ts',
    'input/native-drag.ts',
    'input/mouse.ts',
    'actions/pointer-commands.ts',
    'input/keyboard.ts',
    'input/native-keyboard.ts',
    'native/page.ts',
    'native/world.ts',
    'native/frames.ts',
    'native/frame-path.ts',
    'native/dialogs.ts',
    'native/recording.ts',
    'native/recording-document.ts',
    'native/document-recorder.ts',
    'native/recorder-scripts.ts',
    'dialogs/service.ts',
    'dialogs/native.ts',
    'shell/hold-still.ts',
    'workflow/target-picker.ts',
  ]) {
    const source = readFileSync(new URL(`../../../../src/main/${file}`, import.meta.url), 'utf8');
    assert.doesNotMatch(source, /from ['"][^'"]*\/cdp\//);
    assert.doesNotMatch(source, /\.debugger\b|\bsendCommand\s*\(|Input\.dispatch/);
  }
});
