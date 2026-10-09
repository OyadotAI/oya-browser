/** Pointer translation targets the owned native view with bounded flags and page-relative coordinates. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { dispatchNativePointer } from '../../../../src/main/native/pointer-input.ts';
import { validateParams } from '../../../../src/main/native-front-door/validation.ts';
/** Record only native input calls; no page script or debugger exists at this seam. */
function fixture(zoom = 2) {
  const sent: any[] = [];
  const page = {
    webContents: {
      isDestroyed: () => false,
      getZoomFactor: () => zoom,
      sendInputEvent: (event: unknown) => sent.push(event),
    },
  } as any;
  return { page, sent };
}
test('pointer presses and releases preserve CSS zoom, button masks and modifier state', () => {
  const f = fixture();
  dispatchNativePointer(f.page, { type: 'mousePressed', x: 12.5, y: 20, button: 'left', clickCount: 2, modifiers: 9 });
  assert.deepEqual(f.sent[0], {
    type: 'mouseDown',
    x: 25,
    y: 40,
    button: 'left',
    clickCount: 2,
    modifiers: ['alt', 'shift', 'leftbuttondown'],
  });
  dispatchNativePointer(f.page, { type: 'mouseReleased', x: 12.5, y: 20, button: 'left', clickCount: 2 });
  assert.deepEqual(f.sent[1].modifiers, []);
  dispatchNativePointer(f.page, { type: 'mouseMoved', x: 20, y: 30, buttons: 5 });
  assert.deepEqual(f.sent[2].modifiers, ['leftbuttondown', 'middlebuttondown']);
});
test('wheel translation preserves precise CSS distance and direction at page zoom', () => {
  const f = fixture();
  dispatchNativePointer(f.page, { type: 'mouseWheel', x: 10, y: 20, deltaX: -5, deltaY: 40 });
  assert.equal(f.sent[0].deltaX, 10);
  assert.equal(f.sent[0].deltaY, -80);
  assert.equal(f.sent[0].hasPreciseScrollingDeltas, true);
});
test('malformed pointer requests cannot dispatch even a partial native event', () => {
  const f = fixture();
  const base = { type: 'mousePressed', x: 10, y: 20, button: 'left' };
  for (const patch of [
    { type: 'toString' },
    { x: NaN },
    { x: '1' },
    { y: Infinity },
    { x: 1000001 },
    { modifiers: -1 },
    { modifiers: 16 },
    { buttons: 8 },
    { buttons: 0 },
    { button: null },
    { button: 'back' },
    { clickCount: 1.5 },
    { clickCount: 4 },
    { deltaX: 1 },
    { type: 'mouseWheel' },
    { type: 'mouseReleased', buttons: 1 },
  ])
    assert.throws(() => dispatchNativePointer(f.page, { ...base, ...patch }));
  assert.deepEqual(f.sent, []);
});
test('unsupported pointer semantics are rejected by the protocol contract', () => {
  for (const extra of [
    { timestamp: 1 },
    { pointerType: 'pen' },
    { force: 0.5 },
    { tiltX: 1 },
    { tangentialPressure: 0 },
  ])
    assert.throws(() =>
      validateParams({
        id: 1,
        method: 'Input.dispatchMouseEvent',
        params: { type: 'mouseMoved', x: 1, y: 1, ...extra },
      }),
    );
});
test('destroyed targets and unusable zoom never receive native input', () => {
  const f = fixture();
  f.page.webContents.isDestroyed = () => true;
  assert.throws(() => dispatchNativePointer(f.page, { type: 'mouseMoved', x: 1, y: 1 }), /destroyed/);
  for (const zoom of [NaN, 0, -1, Infinity]) {
    const bad = fixture(zoom);
    assert.throws(() => dispatchNativePointer(bad.page, { type: 'mouseMoved', x: 1, y: 1 }), /zoom/);
    assert.deepEqual(bad.sent, []);
  }
});

test('fractional external coordinates are not rounded by the human mouse-path helper', () => {
  const f = fixture(1);
  dispatchNativePointer(f.page, { type: 'mouseMoved', x: 12.25, y: 17.5 });
  assert.equal(f.sent[0].x, 12.25);
  assert.equal(f.sent[0].y, 17.5);
});
