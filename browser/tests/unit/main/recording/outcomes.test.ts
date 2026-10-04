/**
 * Unit tests for src/main/recording/outcomes.ts: the page checks a recording adds
 * when a person's action moves a tab, and the one it ends on.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { Recorder } from '../../../../src/main/recording/recorder.ts';
import { pageOf } from '../../../../src/main/recording/outcomes.ts';
import { PAGE_CHECK_SETTLE_MS } from '../../../../src/main/recording/constants.ts';
import { mainCtx, FakeBrowserView } from '../../support/main-ctx.cjs';

describe('page checks', () => {
  let ctx: any, view: any, recorder: any;
  beforeEach(async () => {
    mock.timers.enable({ apis: ['setTimeout', 'setInterval', 'Date'], now: 1000 });
    ctx = mainCtx({ recorder: Recorder });
    view = new FakeBrowserView();
    view.webContents.url = 'https://shop.test/login';
    ctx.tabs = { list: [{ id: 1, view }], activeTabId: 1, getActiveView: () => view };
    ctx.recorder.channels.armRecordingView = async () => {};
    ctx.recorder.channels.stopAll = async () => {};
    recorder = ctx.recorder;
    await recorder.startRecording();
  });
  afterEach(() => mock.timers.reset());

  /** The tab moves to `url` now, and the check has had time to run. */
  const moveTo = (url: string) => {
    view.webContents.url = url;
    recorder.pageReached(1, url);
    mock.timers.tick(PAGE_CHECK_SETTLE_MS);
  };

  /** The steps as [action, url or expected]. */
  const steps = () => recorder.recordedSteps.map((s: any) => [s.action, s.url || s.expected || '']);

  it('checks the page a click took the tab to', () => {
    recorder.pushRecordedStep({ action: 'click', el: { text: 'Log in', role: 'button' } });
    mock.timers.tick(1);
    moveTo('https://shop.test/account?session=abc');
    assert.deepEqual(steps().at(-1), ['assert_page', 'https://shop.test/account?session=abc']);
  });

  it('follows a redirect the page made itself to where it settled', () => {
    recorder.pushRecordedStep({ action: 'click', el: { text: 'Log in', role: 'button' } });
    mock.timers.tick(1);
    moveTo('https://shop.test/redirecting');
    moveTo('https://shop.test/account');
    assert.deepEqual(
      steps().filter(([action]) => action === 'assert_page'),
      [['assert_page', 'https://shop.test/account']],
    );
  });

  it('checks the query parameters a filter or sort changed, and only those', () => {
    recorder.pushRecordedStep({ action: 'click', el: { text: 'Sort', role: 'button' } });
    mock.timers.tick(1);
    moveTo('https://shop.test/login?sort=price&qid=1789940643');
    const check = recorder.recordedSteps.at(-1);
    assert.deepEqual([check.action, check.params], ['assert_page', 'sort']);
  });

  it('holds a new page to its path only, whatever its query', () => {
    recorder.pushRecordedStep({ action: 'click', el: { text: 'Add to cart', role: 'button' } });
    mock.timers.tick(1);
    moveTo('https://shop.test/cart/added?keywords=&newItems=abc');
    assert.equal(recorder.recordedSteps.at(-1).params, undefined);
  });

  it('adds nothing when only a per-visit token or a record id changed', () => {
    recorder.pushRecordedStep({ action: 'click', el: { text: 'Next job', role: 'button' } });
    moveTo('https://shop.test/login?currentJobId=4011223344&trackingId=abc&ds=v1%3A1pkTEjUUgKtHU4xZbq9sQwRtY');
    assert.ok(!steps().some(([action]) => action === 'assert_page'));
  });

  it('checks where Back landed, and where the recording ended after it', async () => {
    await recorder.recordHistory('go_back');
    mock.timers.tick(1);
    moveTo('https://shop.test/home');
    assert.deepEqual(steps().at(-1), ['assert_page', 'https://shop.test/home']);
  });

  it('reads an Amazon product path the same whatever its /ref= segment', () => {
    assert.equal(
      pageOf('https://a.test/Cable/dp/B08/ref=sr_1_3?k=x'),
      pageOf('https://a.test/Cable/dp/B08/ref=sr_1_1'),
    );
  });

  it('adds nothing after a navigation the person typed, which already says where the tab is', async () => {
    await recorder.recordNavigation('https://shop.test/cart');
    moveTo('https://shop.test/cart');
    assert.ok(!steps().some(([action]) => action === 'assert_page'));
  });

  it('checks the page the recording ended on when the steps never said so', () => {
    recorder.pushRecordedStep({ action: 'click', el: { text: 'Next', role: 'button' } });
    view.webContents.url = 'https://shop.test/step-2';
    recorder.checks.finalPageCheck();
    assert.deepEqual(steps().at(-1), ['assert_page', 'https://shop.test/step-2']);
  });

  it('adds no final check when the tab is still where the steps say it is', () => {
    recorder.pushRecordedStep({ action: 'click', el: { text: 'Next', role: 'button' } });
    recorder.checks.finalPageCheck();
    assert.notEqual(steps().at(-1)[0], 'assert_page');
  });

  it('compares pages by origin, path and a #/ route, not by query or a plain fragment', () => {
    assert.equal(pageOf('https://a.test/x?q=1#top'), pageOf('https://a.test/x'));
    assert.notEqual(pageOf('https://a.test/#/active'), pageOf('https://a.test/#/done'));
  });
});
