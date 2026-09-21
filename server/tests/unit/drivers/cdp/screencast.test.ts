/**
 * Unit tests for the live view of a CDP browser through a fake connection:
 * frames forwarded and acknowledged, idle gaps filled with screenshots, and
 * stopping cleanly.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  IDLE_FILL_INTERVAL_MS,
  SCREENCAST_EVERY_NTH_FRAME,
  SCREENCAST_MAX_WIDTH,
  SCREENCAST_QUALITY,
} from '../../../../src/drivers/cdp/constants.ts';
import { SESSION, fakeDriver } from '../../support/cdp.ts';
import { advance } from '../../support/http.ts';

describe('screencast', () => {
  beforeEach(() => mock.timers.enable({ apis: ['setInterval', 'Date'], now: 10_000 }));
  afterEach(() => mock.timers.reset());

  it('starts with the default settings, once', async () => {
    const { driver, conn } = fakeDriver();
    await driver.startScreencast(() => {});
    await driver.startScreencast(() => {});
    assert.deepEqual(
      conn.sent('Page.startScreencast').map((c) => c.params),
      [
        {
          format: 'jpeg',
          quality: SCREENCAST_QUALITY,
          maxWidth: SCREENCAST_MAX_WIDTH,
          everyNthFrame: SCREENCAST_EVERY_NTH_FRAME,
        },
      ],
    );
    await driver.stopScreencast();
  });

  it('forwards each frame of its own session as a data URL and acknowledges it', async () => {
    const { driver, conn } = fakeDriver();
    const frames: string[] = [];
    await driver.startScreencast((f: string) => frames.push(f), { quality: 70 });
    conn.emit('Page.screencastFrame', { data: 'AAA', sessionId: 42 });
    conn.emit('Page.screencastFrame', { data: 'BBB', sessionId: 43 }, 'other-session');
    assert.deepEqual(frames, ['data:image/jpeg;base64,AAA']);
    assert.deepEqual(conn.sent('Page.screencastFrameAck')[0], {
      method: 'Page.screencastFrameAck',
      params: { sessionId: 42 },
      sessionId: SESSION,
    });
    await driver.stopScreencast();
  });

  it('acknowledges a frame even when the consumer throws', async () => {
    const { driver, conn } = fakeDriver();
    await driver.startScreencast(() => {
      throw new Error('consumer gone');
    });
    conn.emit('Page.screencastFrame', { data: 'A', sessionId: 1 });
    assert.equal(conn.sent('Page.screencastFrameAck').length, 1);
    await driver.stopScreencast();
  });

  it('fills a quiet live view with a screenshot', async () => {
    const { driver, conn } = fakeDriver();
    conn.replies['Page.captureScreenshot'] = { data: 'IDLE' };
    const frames: string[] = [];
    await driver.startScreencast((f: string) => frames.push(f));
    await advance(IDLE_FILL_INTERVAL_MS);
    assert.deepEqual(frames, ['data:image/jpeg;base64,IDLE']);
    assert.equal(conn.sent('Page.captureScreenshot')[0].params.optimizeForSpeed, true);
    await driver.stopScreencast();
  });

  it('sends no fill while frames are arriving', async () => {
    const { driver, conn } = fakeDriver();
    const frames: string[] = [];
    await driver.startScreencast((f: string) => frames.push(f));
    conn.emit('Page.screencastFrame', { data: 'LIVE', sessionId: 1 });
    await advance(IDLE_FILL_INTERVAL_MS);
    assert.deepEqual(frames, ['data:image/jpeg;base64,LIVE']);
    await driver.stopScreencast();
  });

  it('tries again on the next tick when a fill screenshot fails', async () => {
    const { driver, conn } = fakeDriver();
    let attempt = 0;
    conn.replies['Page.captureScreenshot'] = () => (++attempt === 1 ? new Error('busy') : { data: 'OK' });
    const frames: string[] = [];
    await driver.startScreencast((f: string) => frames.push(f));
    await advance(IDLE_FILL_INTERVAL_MS, 2);
    assert.deepEqual(frames, ['data:image/jpeg;base64,OK']);
    await driver.stopScreencast();
  });

  it('stops the screencast, its listener and its fill', async () => {
    const { driver, conn } = fakeDriver();
    const frames: string[] = [];
    await driver.startScreencast((f: string) => frames.push(f));
    await driver.stopScreencast();
    await driver.stopScreencast();
    conn.emit('Page.screencastFrame', { data: 'LATE', sessionId: 1 });
    await advance(IDLE_FILL_INTERVAL_MS);
    assert.deepEqual(frames, []);
    assert.equal(conn.sent('Page.stopScreencast').length, 1);
  });
});
