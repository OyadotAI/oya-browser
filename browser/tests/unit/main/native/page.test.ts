/**
 * Unit tests for src/main/native/page.ts's capture: a frame the compositor is
 * still catching up on (UnknownVizError under Xvfb) is tried again, anything
 * else is reported at once, and a lag that never ends still fails.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { capturePage } from '../../../../src/main/native/page.ts';
import {
  CAPTURE_VIZ_TRIES,
  CAPTURE_VIZ_BACKOFF_MS,
  NATIVE_CAPTURE_TIMEOUT_MS,
} from '../../../../src/main/native/constants.ts';

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
    t.mock.timers.tick(CAPTURE_VIZ_BACKOFF_MS * (i + 1));
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
    t.mock.timers.tick(CAPTURE_VIZ_BACKOFF_MS * (i + 1));
  }
  await failed;
  assert.equal(state.attempts, CAPTURE_VIZ_TRIES);
});

it('fixed viewport capture crops native pixels without including unused window space', async () => {
  let rectangle: unknown;
  const image = { isEmpty: () => false, getSize: () => ({ width: 375, height: 812 }), toPNG: () => Buffer.from('png') };
  const view: any = {
    webContents: {
      isDestroyed: () => false,
      capturePage: async (rect: unknown) => {
        rectangle = rect;
        return image;
      },
    },
  };
  assert.equal(await capturePage(view, undefined, { width: 375, height: 812 }), 'data:image/png;base64,cG5n');
  assert.deepEqual(rectangle, { x: 0, y: 0, width: 375, height: 812 });
});
it('a native surface smaller than the requested viewport returns an error instead of a clipped image', async () => {
  const image = {
    isEmpty: () => false,
    getSize: () => ({ width: 400, height: 700 }),
    toPNG: () => {
      throw Error('must not encode clipped pixels');
    },
  };
  const view: any = { webContents: { isDestroyed: () => false, capturePage: async () => image } };
  await assert.rejects(capturePage(view, undefined, { width: 1280, height: 812 }), /does not fit/);
});

it('a suspended compositor times out and never encodes its late result', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  let resume: (value: unknown) => void = () => {},
    encoded = 0;
  const view: any = {
    webContents: {
      isDestroyed: () => false,
      capturePage: () =>
        new Promise((resolve) => {
          resume = resolve;
        }),
    },
  };
  const pending = capturePage(view);
  const rejected = assert.rejects(pending, /Native screenshot timed out/);
  t.mock.timers.tick(NATIVE_CAPTURE_TIMEOUT_MS);
  await rejected;
  resume({
    isEmpty: () => false,
    toPNG: () => {
      encoded++;
      return Buffer.from('png');
    },
  });
  await Promise.resolve();
  assert.equal(encoded, 0);
});
