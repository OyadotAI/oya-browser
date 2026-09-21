/**
 * Unit tests for RecordingChannels: one channel per view, a failed arm
 * forgotten for retry, a closed tab's stop failure tolerated.
 */
const { describe, it, beforeEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { RecordingChannels } = require('../../../../main/recording/channels.cjs');
const { RecordingChannel } = require('../../../../scripts/recording.cjs');
const { mainCtx, FakeBrowserView } = require('../../support/main-ctx.cjs');

describe('RecordingChannels', () => {
  let ctx, channels, view;
  beforeEach(() => {
    ctx = mainCtx();
    channels = new RecordingChannels(ctx);
    view = new FakeBrowserView();
  });

  it('arms a view once, with a fresh analyzer tag and recording off', async () => {
    const start = mock.method(RecordingChannel.prototype, 'start', async () => {});
    await channels.armRecordingView(view);
    await channels.armRecordingView(view);
    assert.equal(start.mock.callCount(), 1);
    const channel = channels.channels.get(view);
    assert.match(channel.options?.analyzer ?? channels.analyzer(), /^analyzer\(data-[0-9a-f]{8}, false\)$/);
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

  it('listens for protocol events through the view debugger and can unsubscribe', () => {
    const options = channels.channelOptions(view, 'https://a.test/');
    const got = [];
    const off = options.on('Runtime.bindingCalled', (p) => got.push(p));
    view.webContents.debugger.event('Runtime.bindingCalled', { n: 1 });
    view.webContents.debugger.event('Other', {});
    off();
    view.webContents.debugger.event('Runtime.bindingCalled', { n: 2 });
    assert.deepEqual(got, [{ n: 1 }]);
    assert.equal(options.worldName, 'w-test');
  });
});
