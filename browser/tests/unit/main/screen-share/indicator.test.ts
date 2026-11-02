/** Tab shares are tracked only while a call captures the tab, and named by host only. */
import { it, type TestContext } from 'node:test';
import assert from 'node:assert/strict';
import { ShareIndicator } from '../../../../src/main/screen-share/indicator.ts';
import { SHARE_POLL_MS } from '../../../../src/main/screen-share/constants.ts';

/** A tab whose capture count the test sets, counting the reads. */
function tab(id: number, url = `https://site${id}.test/path?secret=1`) {
  const contents = {
    mainFrame: { id },
    captured: true,
    destroyed: false,
    reads: 0,
    isDestroyed: () => contents.destroyed,
    isBeingCaptured: () => (contents.reads++, contents.captured),
  };
  return { id, url, view: { webContents: contents } };
}

/** A share indicator over three tabs, with recorded sends, presents and activations. */
function fixture(t: TestContext) {
  t.mock.timers.enable({ apis: ['setInterval'] });
  const tabs = [tab(1), tab(2), tab(3, 'oya://home')];
  const calls: string[] = [];
  const sent: unknown[] = [];
  const scope = { tabs: { activateTab: (id: number) => calls.push(`activate ${id}`) } };
  const deps = {
    tabs: { list: tabs, sendTabList: () => calls.push('tab list') },
    shell: { send: (_channel: string, chip: unknown) => sent.push(chip) },
    windows: {
      owner: (id: number) => (tabs.some((x) => x.id === id) ? scope : undefined),
      present: () => calls.push('present'),
    },
  };
  const indicator = new ShareIndicator(deps as any);
  const share = (shared: number, call: number) =>
    indicator.started(tabs[shared - 1].view.webContents.mainFrame as any, tabs[call - 1].view.webContents as any);
  const tick = () => t.mock.timers.tick(SHARE_POLL_MS);
  return { indicator, tabs, deps, calls, sent, share, tick };
}

it('no capture is read until a tab share starts, and none after the last one ends', (t) => {
  const f = fixture(t);
  f.tick();
  assert.equal(f.tabs[0].view.webContents.reads, 0);
  f.share(1, 2);
  f.tick();
  assert.equal(f.tabs[0].view.webContents.reads, 1);
  f.tabs[0].view.webContents.captured = false;
  f.tick();
  f.tick();
  assert.equal(f.tabs[0].view.webContents.reads, 2);
});

it('a window or screen source is never tracked', (t) => {
  const f = fixture(t);
  f.indicator.started({ id: 'screen:0:0' } as any, f.tabs[1].view.webContents as any);
  assert.equal(f.indicator.chip(), null);
  assert.deepEqual(f.calls, []);
});

it('the first false reading ends the share, and the strip is redrawn', (t) => {
  const f = fixture(t);
  f.share(1, 2);
  f.calls.length = 0;
  f.tabs[0].view.webContents.captured = false;
  f.tick();
  assert.equal(f.indicator.has(1), false);
  assert.deepEqual(f.calls, ['tab list']);
});

it('a true reading keeps the share', (t) => {
  const f = fixture(t);
  f.share(1, 2);
  f.tick();
  f.tick();
  assert.equal(f.indicator.has(1), true);
});

it('a destroyed shared page ends the share without reading its capture count', (t) => {
  const f = fixture(t);
  f.share(1, 2);
  f.tabs[0].view.webContents.destroyed = true;
  f.tick();
  assert.equal(f.indicator.has(1), false);
  assert.equal(f.tabs[0].view.webContents.reads, 0);
});

it('closing the shared tab drops it on the tab-list change, before the next poll', (t) => {
  const f = fixture(t);
  f.share(1, 2);
  f.tabs.splice(0, 1);
  f.indicator.tabsChanged();
  assert.equal(f.indicator.has(1), false);
});

it('the chip names the most recent share, and re-sharing a tab makes it most recent', (t) => {
  const f = fixture(t);
  f.share(1, 2);
  f.share(3, 2);
  assert.equal(f.indicator.chip()?.sharedTabId, 3);
  f.share(1, 2);
  assert.equal(f.indicator.chip()?.sharedTabId, 1);
});

it('the chip is null while the calling tab is gone', (t) => {
  const f = fixture(t);
  f.share(1, 2);
  f.tabs.splice(1, 1);
  assert.equal(f.indicator.chip(), null);
  assert.equal(f.indicator.has(1), true);
});

it('a closed call tab falls back to the newest share whose call is still open', (t) => {
  const f = fixture(t);
  f.share(3, 1);
  f.share(1, 2);
  f.tabs.splice(1, 1);
  assert.equal(f.indicator.chip()?.sharedTabId, 3);
});

it('the chip carries hosts only, and null for a non-web page', (t) => {
  const f = fixture(t);
  f.share(3, 1);
  assert.deepEqual(f.indicator.chip(), { sharedTabId: 3, callTabId: 1, shared: null, call: 'site1.test' });
});

it('the chip is re-sent on every tab-list change while sharing, and null once after', (t) => {
  const f = fixture(t);
  f.indicator.tabsChanged();
  f.share(1, 2);
  f.indicator.tabsChanged();
  f.indicator.tabsChanged();
  f.tabs[0].view.webContents.captured = false;
  f.tick();
  f.indicator.tabsChanged();
  f.indicator.tabsChanged();
  assert.equal(f.sent.length, 3);
  assert.equal(f.sent[2], null);
});

it('tabsChanged never re-sends the tab list', (t) => {
  const f = fixture(t);
  f.share(1, 2);
  f.calls.length = 0;
  f.tabs.splice(0, 1);
  f.indicator.tabsChanged();
  assert.deepEqual(f.calls, []);
});

it("go to call presents the call tab's window, then activates the tab", (t) => {
  const f = fixture(t);
  f.share(1, 2);
  f.calls.length = 0;
  f.indicator.goToCall();
  assert.deepEqual(f.calls, ['present', 'activate 2']);
});

it('go to call moves nothing when control is refused, or the call is gone', (t) => {
  const f = fixture(t);
  f.share(1, 2);
  f.calls.length = 0;
  f.deps.windows.present = () => {
    throw new Error('An agent is driving the browser');
  };
  assert.throws(() => f.indicator.goToCall());
  f.tabs.splice(1, 1);
  f.indicator.goToCall();
  assert.deepEqual(f.calls, []);
});

it('a share after one ended runs one poll, not two', (t) => {
  const f = fixture(t);
  f.share(1, 2);
  f.tabs[0].view.webContents.captured = false;
  f.tick();
  f.share(1, 2);
  f.tabs[0].view.webContents.captured = true;
  f.tick();
  assert.equal(f.tabs[0].view.webContents.reads, 2);
});
