/** Native drag dispatch is explicit, acknowledged and never replaced with another transport. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { nativeDrag } from '../../../../src/main/input/index.ts';

/** A browser-owned capability with fatal debugger access catches fallback attempts. */
function fixture(handler?, destroyed = false) {
  return {
    webContents: {
      isDestroyed: () => destroyed,
      _dragOya: handler,
      get debugger() {
        throw new Error('Internal CDP forbidden');
      },
      sendInputEvent() {
        throw new Error('Unacknowledged pointer fallback forbidden');
      },
    },
  };
}
const from = { x: 10, y: 20 },
  to = { x: 50, y: 60 };
it('preserves the native owner and exact endpoints', async () => {
  const view = fixture(async function (a, b) {
    assert.equal(this, view.webContents);
    assert.deepEqual([a, b], [from, to]);
    return 'html';
  });
  assert.equal(await nativeDrag(view, from, to), 'html');
});
it('ordinary pointer drags remain distinguishable from accepted HTML drops', async () => {
  assert.equal(
    await nativeDrag(
      fixture(async () => 'pointer'),
      from,
      to,
    ),
    'pointer',
  );
});
it('missing engine capability fails closed', async () => {
  await assert.rejects(nativeDrag(fixture(), from, to), /Unsupported native capability/);
});
it('rejects destroyed contents before contacting the engine', async () => {
  await assert.rejects(
    nativeDrag(
      fixture(() => assert.fail('must not dispatch'), true),
      from,
      to,
    ),
    /destroyed/,
  );
});
it('rejects nonfinite coordinates before contacting the engine', async () => {
  for (const value of [NaN, Infinity, -Infinity])
    await assert.rejects(
      nativeDrag(
        fixture(() => assert.fail('must not dispatch')),
        { x: value, y: 1 },
        to,
      ),
      /finite/,
    );
});
it('propagates native cancellation without fallback or a false success', async () => {
  await assert.rejects(
    nativeDrag(
      fixture(async () => {
        throw new Error('document changed');
      }),
      from,
      to,
    ),
    /document changed/,
  );
});
it('rejects a malformed native acknowledgement', async () => {
  await assert.rejects(
    nativeDrag(
      fixture(async () => true),
      from,
      to,
    ),
    /Invalid native drag acknowledgement/,
  );
});
