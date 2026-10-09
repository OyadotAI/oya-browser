/**
 * Unit tests for src/main/recording/channels.ts (RecordingChannels): one channel per view, a failed arm
 * forgotten for retry, a closed tab's stop failure tolerated.
 */
import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { RecordingChannels } from '../../../../src/main/recording/channels.ts';
import { NativeRecordingChannel as RecordingChannel } from '../../../../src/main/recording/native-channel.ts';
import { mainCtx, FakeBrowserView } from '../../support/main-ctx.cjs';
import { flush } from '../../support/fakes.cjs';

describe('RecordingChannels', () => {
  let ctx: any, channels: any, view: any;
  beforeEach(() => {
    ctx = mainCtx();
    channels = new RecordingChannels(ctx);
    view = new FakeBrowserView();
    ctx.tabs = { list: [{ id: 3, view }] };
  });

  it('arms a view once, with a fresh analyzer tag and recording off', async () => {
    const start = mock.method(RecordingChannel.prototype, 'start', async () => {});
    await channels.armRecordingView(view);
    await channels.armRecordingView(view);
    assert.equal(start.mock.callCount(), 1);
    const channel = channels.channels.get(view);
    assert.match(channel.analyzer, /^analyzer\(data-[0-9a-f]{8}, false\)$/);
    start.mock.restore();
  });

  it('forgets a view whose recorder failed to start, so the next arm retries', async () => {
    const start = mock.method(RecordingChannel.prototype, 'start', async () => {
      throw new Error('no page');
    });
    await assert.rejects(channels.armRecordingView(view), /no page/);
    assert.equal(channels.channels.size, 0);
    start.mock.restore();
  });

  it('a failed old start cannot erase a replacement channel', async () => {
    let rejectOld: (error: Error) => void;
    const first = new Promise<void>((_resolve, reject) => {
      rejectOld = reject;
    });
    const start = mock.method(RecordingChannel.prototype, 'start', () => first);
    mock.method(RecordingChannel.prototype, 'stop', async () => {});
    const old = assert.rejects(channels.armRecordingView(view), /old start/);
    channels.forget(view);
    start.mock.mockImplementation(async () => {});
    await channels.armRecordingView(view);
    const replacement = channels.channels.get(view);
    rejectOld!(Error('old start'));
    await old;
    assert.equal(channels.channels.get(view), replacement);
    await channels.stopAll();
  });

  it('tolerates a stop failing on a closed tab, but not on a live one', async () => {
    const closed = {
      ready: Promise.resolve(),
      stop: async () => {
        throw new Error('gone');
      },
    };
    view.webContents.destroyed = true;
    channels.channels.set(view, closed);
    await channels.stopAll();
    assert.equal(channels.channels.size, 0);
    const live = new FakeBrowserView();
    channels.channels.set(live, closed);
    await assert.rejects(channels.stopAll(), /gone/);
  });

  it('forgets every channel and stops the rest even when a live tab refuses to stop', async () => {
    const stopped: any[] = [];
    const refusing = { ready: Promise.resolve(), stop: async () => Promise.reject(new Error('busy')) };
    const next = { ready: Promise.resolve(), stop: async () => stopped.push('next') };
    channels.channels.set(view, refusing);
    channels.channels.set(new FakeBrowserView(), next);
    await assert.rejects(channels.stopAll(), /busy/);
    assert.equal(channels.channels.size, 0);
    assert.deepEqual(stopped, ['next']);
  });

  it("forgets a closed tab's channel", async () => {
    let stopped = false;
    channels.channels.set(view, { ready: Promise.resolve(), stop: async () => (stopped = true) });
    channels.forget(view);
    await flush();
    assert.equal(channels.channels.has(view), false);
    assert.equal(stopped, true);
  });

  it('arming and disposal never access a debugger transport', async () => {
    Object.defineProperty(view.webContents, 'debugger', {
      get() {
        assert.fail('Internal CDP forbidden');
      },
    });
    const start = mock.method(RecordingChannel.prototype, 'start', async () => {});
    const stop = mock.method(RecordingChannel.prototype, 'stop', async () => {});
    await channels.armRecordingView(view);
    await channels.stopAll();
    assert.equal(start.mock.callCount(), 1);
    assert.equal(stop.mock.callCount(), 1);
  });
});
