/**
 * Unit tests for src/main/native/page.ts's capture: a frame the compositor is
 * still catching up on (UnknownVizError under Xvfb) is tried again, anything
 * else is reported at once, and a lag that never ends still fails.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { capturePage } from '../../../../src/main/native/page.ts';
import { CAPTURE_VIZ_TRIES } from '../../../../src/main/native/constants.ts';

/** A page whose captures fail with `errors` in turn, then return a frame. */
function page(errors: string[]) {
  const state = { attempts: 0 };
  const image = { isEmpty: () => false, toPNG: () => Buffer.from('png'), toJPEG: () => Buffer.from('jpg') };
  const webContents = {
    isDestroyed: () => false,
    capturePage: async () => {
      const error = errors[state.attempts++];
      if (error) throw new Error(error);
      return image;
    },
  };
  return { view: { webContents } as any, state };
}

it('a capture the compositor is catching up on is tried again', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const { view, state } = page(['UnknownVizError', 'UnknownVizError']);
  const shot = capturePage(view);
  for (let i = 0; i < 2; i++) {
    await new Promise((r) => setImmediate(r));
    t.mock.timers.runAll();
  }
  assert.equal(await shot, `data:image/png;base64,${Buffer.from('png').toString('base64')}`);
  assert.equal(state.attempts, 3);
});

it('any other capture failure is reported at once', async () => {
  const { view, state } = page(['Current display surface not available for capture']);
  await assert.rejects(capturePage(view), /display surface/);
  assert.equal(state.attempts, 1);
});

it('a compositor that never catches up still fails', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const { view, state } = page(Array(CAPTURE_VIZ_TRIES).fill('UnknownVizError'));
  const shot = capturePage(view);
  const failed = assert.rejects(shot, /UnknownVizError/);
  for (let i = 0; i < CAPTURE_VIZ_TRIES; i++) {
    await new Promise((r) => setImmediate(r));
    t.mock.timers.runAll();
  }
  await failed;
  assert.equal(state.attempts, CAPTURE_VIZ_TRIES);
});
