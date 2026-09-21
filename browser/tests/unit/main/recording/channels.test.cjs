/**
 * Unit tests for RecordingChannels: one channel per view, a failed arm
 * forgotten for retry, a closed tab's stop failure tolerated.
 */
const { describe, it, beforeEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { RecordingChannels } = require('../../../../main/recording/channels.cjs');
const { RecordingChannel } = require('../../../../scripts/recording.cjs');
const { mainCtx, FakeBrowserView } = require('../../support/main-ctx.cjs');
const { flush } = require('../../support/fakes.cjs');
const { RECORDING_CDP_MS } = require('../../../../main/recording/constants.cjs');

describe('RecordingChannels', () => {
  let ctx, channels, view;
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

  it('forgets every channel and stops the rest even when a live tab refuses to stop', async () => {
    const stopped = [];
    const refusing = { ready: Promise.resolve(), stop: async () => Promise.reject(new Error('busy')) };
    const next = { ready: Promise.resolve(), stop: async () => stopped.push('next') };
    channels.channels.set(view, refusing);
    channels.channels.set(new FakeBrowserView(), next);
    await assert.rejects(channels.stopAll(), /busy/);
    assert.equal(channels.channels.size, 0);
    assert.deepEqual(stopped, ['next']);
  });

  it('gives up on a page that never answers a recording command', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    view.webContents.debugger.sendCommand = () => new Promise(() => {});
    const answer = channels.channelOptions(view, 'https://a.test/').send('Runtime.evaluate', {});
    mock.timers.tick(RECORDING_CDP_MS);
    await assert.rejects(answer, /did not answer/);
    mock.timers.reset();
  });

  it("hands a channel's steps on with the tab it was made for", () => {
    const got = [];
    ctx.recorder = { receive: (...args) => got.push(args) };
    channels.channelOptions(view, 'https://a.test/', 7).receive({ steps: [] });
    assert.deepEqual(got[0].slice(1), ['https://a.test/', { steps: [] }, 7]);
  });

  it("forgets a closed tab's channel", async () => {
    let stopped = false;
    channels.channels.set(view, { ready: Promise.resolve(), stop: async () => (stopped = true) });
    channels.forget(view);
    await flush();
    assert.equal(channels.channels.has(view), false);
    assert.equal(stopped, true);
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
