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

  it('still stops and reports a failure on a live tab', async () => {
    ctx.recorder.channels.armRecordingView = async (view) => {
      if (view === closing) throw new Error('page refused');
    };
    await assert.rejects(ctx.recorder.startRecording(), /page refused/);
    assert.equal(ctx.recorder.recording, false);
  });
});
