/**
 * Unit tests for starting a recording (src/main/recording/start.ts): a tab that closes while
 * the recording arms every tab does not abort the whole recording.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { Recorder } from '../../../../src/main/recording/recorder.ts';
import { mainCtx, FakeBrowserView } from '../../support/main-ctx.cjs';

describe('startRecording', () => {
  let ctx: any, live: any, closing: any;
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
