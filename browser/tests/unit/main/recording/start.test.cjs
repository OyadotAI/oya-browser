/**
 * Unit tests for starting a recording (start.cjs): a tab that closes while
 * the recording arms every tab does not abort the whole recording.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { Recorder } = require('../../../../main/recording/recorder.cjs');
const { mainCtx, FakeBrowserView } = require('../../support/main-ctx.cjs');

describe('startRecording', () => {
  let ctx, live, closing;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setInterval', 'Date'], now: 1000 });
    ctx = mainCtx({ recorder: Recorder });
    live = new FakeBrowserView();
    live.webContents.url = 'https://start.test/';
    closing = new FakeBrowserView();
    ctx.tabs = {
      list: [
        { id: 1, view: live },
        { id: 2, view: closing },
      ],
      activeTabId: 1,
      getActiveView: () => live,
    };
    ctx.recorder.channels.stopAll = async () => {};
  });
  afterEach(() => mock.timers.reset());

  it('keeps recording the other tabs when one closes while they are armed', async () => {
    ctx.recorder.channels.armRecordingView = async (view) => {
      if (view !== closing) return;
      view.webContents.destroyed = true;
      throw new Error('View is destroyed');
    };
    const result = await ctx.recorder.startRecording();
    assert.equal(result.recording, true);
  });

  it('stops and reports a failure on the tab the person is looking at', async () => {
    ctx.recorder.channels.armRecordingView = async (view) => {
      if (view === live) throw new Error('page refused');
    };
    await assert.rejects(ctx.recorder.startRecording(), /page refused/);
    assert.equal(ctx.recorder.recording, false);
  });

  it('keeps recording when a background tab cannot be armed', async () => {
    ctx.recorder.channels.armRecordingView = async (view) => {
      if (view === closing) throw new Error('page refused');
    };
    const logged = mock.method(console, 'error', () => {});
    assert.equal((await ctx.recorder.startRecording()).recording, true);
    assert.match(logged.mock.calls[0].arguments.join(' '), /page refused/);
  });
});
