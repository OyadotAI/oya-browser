/**
 * Unit tests for Recorder: step intake (limits, duplicates, late steps),
 * navigation steps, start/resume/stop, the serialized task queue, and the
 * hand-off when an agent takes control.
 */
const { describe, it, beforeEach, afterEach, mock } = require('node:test');
const assert = require('node:assert/strict');
const { Recorder } = require('../../../../main/recording/recorder.cjs');
const { mainCtx, FakeBrowserView } = require('../../support/main-ctx.cjs');
const { flush } = require('../../support/fakes.cjs');

/** A fake workspace that remembers what it was handed. */
function fakeWorkspace(draft = { steps: [], secrets: [] }) {
  return {
    draft,
    captures: [],
    edits: [],
    busy: () => false,
    capture(...args) {
      this.captures.push(args);
    },
    edit(c) {
      this.edits.push(c);
    },
  };
}

describe('Recorder', () => {
  let ctx, view;
  beforeEach(() => {
    mock.timers.enable({ apis: ['setInterval', 'Date'], now: 1000 });
    ctx = mainCtx({ recorder: Recorder });
    view = new FakeBrowserView();
    view.webContents.url = 'https://start.test/';
    ctx.tabs = { list: [{ id: 1, view }], activeTabId: 1, getActiveView: () => view };
    ctx.recorder.channels.armRecordingView = async () => {};
    ctx.recorder.channels.stopAll = async () => {};
    ctx.workspace = fakeWorkspace();
  });
  afterEach(() => mock.timers.reset());

  it('starts where the person is, on a fresh draft', async () => {
    const result = await ctx.recorder.startRecording();
    assert.equal(result.recording, true);
    assert.deepEqual(ctx.workspace.edits, [{ type: 'new' }]);
    assert.equal(ctx.recorder.recordedSteps[0].action, 'navigate');
    assert.equal(ctx.recorder.recordedSteps[0].url, 'https://start.test/');
    assert.equal(ctx.recorder.recordedSteps[0].tab, 'main');
  });

  it('refuses to record during a validation', async () => {
    ctx.workspace.busy = () => true;
    await assert.rejects(ctx.recorder.startRecording(), /Stop validation before recording/);
  });

  it('records a move made while paused when it resumes', async () => {
    await ctx.recorder.startRecording();
    await ctx.recorder.stopRecording();
    ctx.workspace.draft = { steps: structuredClone(ctx.recorder.recordedSteps), secrets: [] };
    view.webContents.url = 'https://elsewhere.test/';
    await ctx.recorder.startRecording(true);
    assert.deepEqual(
      ctx.recorder.recordedSteps.map((s) => s.url),
      ['https://start.test/', 'https://elsewhere.test/'],
    );
  });

  it('drops duplicate steps and steps after the cutoff', async () => {
    await ctx.recorder.startRecording();
    ctx.recorder.pushRecordedStep({ id: 'a', action: 'click' });
    ctx.recorder.pushRecordedStep({ id: 'a', action: 'click' });
    ctx.recorder.recordingCutoff = 500;
    ctx.recorder.pushRecordedStep({ id: 'b', action: 'click', t: 900 });
    assert.deepEqual(ctx.recorder.recordedSteps.map((s) => s.id).slice(1), ['a']);
  });

  it('drops a malformed step without losing the ones after it', async () => {
    await ctx.recorder.startRecording();
    const logged = mock.method(console, 'error', () => {});
    ctx.recorder.pushRecordedStep({ id: 'bad', action: 'click', url: 5 });
    ctx.recorder.pushRecordedStep({ id: 'good', action: 'click' });
    assert.deepEqual(ctx.recorder.recordedSteps.map((s) => s.id).slice(1), ['good']);
    assert.equal(logged.mock.callCount(), 1);
  });

  it('keeps a step whose element has a role no locator can use', async () => {
    await ctx.recorder.startRecording();
    ctx.recorder.pushRecordedStep({ action: 'click', el: { tag: 'button', role: 'none presentation', text: 'Go' } });
    assert.equal(ctx.recorder.recordedSteps.at(-1).action, 'click');
  });

  it('finds an element with no name or handle by its recorded position', async () => {
    await ctx.recorder.startRecording();
    ctx.recorder.pushRecordedStep({
      action: 'click',
      el: { tag: 'div', type: 'button', path: '[id="bar"] > div:nth-of-type(2)' },
    });
    const step = ctx.recorder.recordedSteps.at(-1);
    assert.deepEqual(step.candidates, [{ kind: 'css', value: '[id="bar"] > div:nth-of-type(2)' }]);
    assert.equal(step.enabled, true);
  });

  it('turns off a step nothing identifies, saying why, so the workflow still saves', async () => {
    await ctx.recorder.startRecording();
    ctx.recorder.pushRecordedStep({ action: 'click', el: { tag: 'div', type: 'button' } });
    const step = ctx.recorder.recordedSteps.at(-1);
    assert.equal(step.enabled, false);
    assert.match(step.captureIssue, /Nothing identifies this element/);
  });

  it('turns off the click that opened a file picker once the upload is recorded', async () => {
    await ctx.recorder.startRecording();
    ctx.recorder.pushRecordedStep({ action: 'click', el: { tag: 'button', text: 'Upload' }, t: 2000 });
    ctx.recorder.pushRecordedStep({
      action: 'upload_file',
      el: { tag: 'input', name: 'cv' },
      file: '{{upload_file}}',
      t: 5000,
    });
    const [click, upload] = ctx.recorder.recordedSteps.slice(-2);
    assert.equal(click.enabled, false);
    assert.equal(upload.enabled, true);
  });

  it('stops at the step limit and says so on the last step', async () => {
    await ctx.recorder.startRecording();
    for (let i = 0; i < 510; i++) ctx.recorder.pushRecordedStep({ action: 'click', id: 's' + i });
    assert.equal(ctx.recorder.recordedSteps.length, 500);
    assert.match(ctx.recorder.recordedSteps[499].captureIssue, /500-step capture limit/);
    await ctx.recorder.recordingTask;
    assert.equal(ctx.recorder.recording, false);
  });

  it('records only typed web addresses, once', async () => {
    await ctx.recorder.startRecording();
    ctx.recorder.recordNavigation('https://b.test/');
    ctx.recorder.recordNavigation('https://b.test/');
    ctx.recorder.recordNavigation('about:blank');
    assert.deepEqual(
      ctx.recorder.recordedSteps.map((s) => s.url),
      ['https://start.test/', 'https://b.test/'],
    );
  });

  it("starts a new tab's steps with the page it began on", async () => {
    await ctx.recorder.startRecording();
    const other = new FakeBrowserView();
    ctx.tabs.list.push({ id: 2, view: other });
    ctx.recorder.receive(other, 'https://other.test/', { steps: [{ action: 'click', t: 2000 }], secrets: ['pw'] });
    const tabSteps = ctx.recorder.recordedSteps.filter((s) => s.tab === 'tab-1');
    assert.deepEqual(
      tabSteps.map((s) => [s.action, s.t]),
      [
        ['navigate', 1999],
        ['click', 2000],
      ],
    );
    assert.ok(ctx.recorder.recordedSecrets.has('pw'));
  });

  it('starts a tab opened blank with the page its first steps happened on', async () => {
    await ctx.recorder.startRecording();
    const other = new FakeBrowserView();
    other.webContents.url = 'https://landed.test/';
    ctx.tabs.list.push({ id: 2, view: other });
    ctx.recorder.receive(other, 'about:blank', { steps: [{ action: 'click', t: 2000 }] });
    const first = ctx.recorder.recordedSteps.find((s) => s.tab === 'tab-1');
    assert.deepEqual([first.action, first.url], ['navigate', 'https://landed.test/']);
  });

  it('ignores page output when not recording', () => {
    ctx.recorder.receive(view, 'https://x.test/', { steps: [{ action: 'click' }], secrets: ['pw'] });
    assert.equal(ctx.recorder.recordedSteps.length, 0);
  });

  it('stops a failed start and reports why', async () => {
    ctx.recorder.channels.armRecordingView = async () => {
      throw new Error('no debugger');
    };
    await assert.rejects(ctx.recorder.startRecording(), /no debugger/);
    assert.equal(ctx.recorder.recording, false);
  });

  it('hands a changed recording to the workspace once, and nothing when it is unchanged', async () => {
    await ctx.recorder.startRecording();
    mock.timers.tick(400);
    mock.timers.tick(400);
    assert.equal(ctx.workspace.captures.length, 1);
  });

  it('stops a desktop recording when an agent takes control', async () => {
    await ctx.recorder.startRecording();
    ctx.recorder.controlLost({ interactive: false });
    await ctx.recorder.recordingTask;
    assert.equal(ctx.recorder.recording, false);
    assert.equal(ctx.recorder.recordingCutoff, 1000);
  });

  it('keeps a server-started recording going through a handoff', async () => {
    await ctx.recorder.remote('start');
    ctx.recorder.controlLost({ interactive: false });
    assert.equal(ctx.recorder.recording, true);
    assert.equal((await ctx.recorder.remote('stop')).recording, false);
  });

  it('runs recording tasks one at a time, handing each nothing from the last', async () => {
    const seen = [];
    ctx.recorder.queueRecording(async () => 'first');
    const failed = ctx.recorder.queueRecording(async () => {
      throw new Error('x');
    });
    await assert.rejects(failed);
    await ctx.recorder.queueRecording(async () => 'third');
    await ctx.recorder.queueRecording((...args) => seen.push(args));
    assert.deepEqual(seen, [[]]);
  });

  it('starts a fresh draft after an earlier stop, never resuming it by accident', async () => {
    await ctx.recorder.queueRecording(() => ctx.recorder.startRecording());
    await ctx.recorder.queueRecording(() => ctx.recorder.stopRecording());
    ctx.workspace.edits = [];
    await ctx.recorder.queueRecording(() => ctx.recorder.startRecording());
    assert.deepEqual(ctx.workspace.edits, [{ type: 'new' }]);
  });

  it("keeps the person's step order when a resumed recording stops", async () => {
    await ctx.recorder.startRecording();
    ctx.recorder.adopt(
      [
        { id: 'late', action: 'click', t: 50, tab: 'main' },
        { id: 'early', action: 'click', t: 10, tab: 'main' },
      ],
      [],
    );
    await ctx.recorder.stopRecording();
    assert.deepEqual(
      ctx.recorder.recordedSteps.map((s) => s.id),
      ['late', 'early'],
    );
  });

  it('orders only the newly drained steps by time, and tolerates steps without a time', async () => {
    await ctx.recorder.startRecording();
    ctx.recorder.adopt([{ id: 'kept', action: 'click', tab: 'main' }], []);
    ctx.recorder.channels.drain = async () => {
      ctx.recorder.pushRecordedStep({ id: 'b', action: 'click', t: 1200 });
      ctx.recorder.pushRecordedStep({ id: 'a', action: 'click', t: 1100 });
    };
    await ctx.recorder.stopRecording();
    assert.deepEqual(
      ctx.recorder.recordedSteps.map((s) => s.id),
      ['kept', 'a', 'b'],
    );
  });

  it('keeps recording when handing steps to the workspace throws', async () => {
    await ctx.recorder.startRecording();
    ctx.workspace.capture = () => {
      throw new Error('duplicate step id');
    };
    ctx.recorder.recordNavigation('https://next.test/');
    assert.doesNotThrow(() => mock.timers.tick(400));
    assert.equal(ctx.recorder.recording, true);
  });

  it("drops a closed tab's late steps instead of inventing a tab name for them", async () => {
    await ctx.recorder.startRecording();
    const gone = new FakeBrowserView();
    ctx.recorder.receive(gone, 'https://gone.test/', { steps: [{ id: 'x', action: 'click', t: 1001 }] });
    assert.equal(
      ctx.recorder.recordedSteps.some((s) => s.id === 'x'),
      false,
    );
  });

  it('names a late step by the tab its channel was made for, even once that tab closed', async () => {
    await ctx.recorder.startRecording();
    const gone = new FakeBrowserView();
    ctx.recorder.receive(gone, 'https://start.test/', { steps: [{ id: 'y', action: 'click', t: 1001 }] }, 1);
    assert.equal(ctx.recorder.recordedSteps.find((s) => s.id === 'y').tab, 'main');
  });

  it('does not arm a tab that opens while the recording is stopping', async () => {
    await ctx.recorder.startRecording();
    let release;
    ctx.recorder.channels.stopAll = () => new Promise((resolve) => (release = resolve));
    const armed = [];
    ctx.recorder.channels.armRecordingView = async (v) => armed.push(v);
    const stopped = ctx.recorder.queueRecording(() => ctx.recorder.stopRecording());
    await flush();
    const joined = ctx.recorder.joinIfRecording(new FakeBrowserView());
    release();
    await Promise.all([stopped, joined]);
    assert.deepEqual(armed, []);
  });

  it('adopts a draft and does not hand it straight back', () => {
    ctx.recorder.adopt([{ action: 'click' }], ['pw']);
    ctx.recorder.emitRecording();
    assert.equal(ctx.workspace.captures.length, 0);
  });

  it('joins a new tab only while recording', async () => {
    const armed = mock.method(ctx.recorder.channels, 'armRecordingView', async () => {});
    ctx.recorder.joinIfRecording(view);
    await ctx.recorder.startRecording();
    ctx.recorder.joinIfRecording(view);
    await flush();
    assert.equal(armed.mock.calls.at(-1).arguments[0], view);
  });
});
