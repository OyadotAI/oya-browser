/**
 * Unit tests for src/main/shell/page-zoom.ts: the percentage a zoom level reads as,
 * zooming only a page the person may use, telling every window, and the shell's
 * own pages reading 100.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { sendZoom, zoomPercent, zoomShownPage } from '../../../../src/main/shell/page-zoom.ts';

/** A page's contents at `level`. */
function contents(level = 0) {
  const page: any = { level, isDestroyed: () => false };
  page.getZoomLevel = () => page.level;
  page.setZoomLevel = (next) => (page.level = next);
  return page;
}

/** One window showing tab 1 with `page`, with what its shell was sent. */
function windowWith(page, tab: any = {}) {
  const sent = [];
  const shown = { id: 1, view: { webContents: page }, ...tab };
  const scope: any = {
    tabs: { activeTabId: 1, find: (id) => (id === 1 ? shown : undefined), getActiveView: () => shown.view },
    shell: { send: (channel, data) => sent.push([channel, data]), window: { webContents: contents() } },
    agentTab: { humanMay: () => true },
  };
  return { scope, sent };
}

describe('page zoom', () => {
  it('percent is round(100 x 1.2^level), as Chrome reads it', () => {
    assert.deepEqual([0, 0.5, 1.5, -0.5, -1].map(zoomPercent), [100, 110, 131, 91, 83]);
  });

  it('zoom refuses while an agent holds the shown tab', () => {
    const page = contents();
    const { scope, sent } = windowWith(page);
    scope.agentTab.humanMay = (id) => id !== 1;
    zoomShownPage(scope, (level) => level + 0.5);
    assert.deepEqual([page.level, sent], [0, []]);
  });

  it('zoom sends the new percent to every window, announced', () => {
    const page = contents();
    const first = windowWith(page);
    const second = windowWith(page);
    first.scope.windows = { each: (run) => [first.scope, second.scope].forEach(run) };
    zoomShownPage(first.scope, (level) => level + 0.5);
    for (const { sent } of [first, second]) assert.deepEqual(sent, [['page-zoom', { percent: 110, announce: true }]]);
  });

  it('the start page and popup tabs send 100 even when their view is zoomed', () => {
    for (const tab of [{ home: true }, { window: {} }]) {
      const { scope, sent } = windowWith(contents(2), tab);
      sendZoom(scope, false);
      assert.deepEqual(sent, [['page-zoom', { percent: 100, announce: false }]]);
    }
  });

  it('zooming on the start page or a popup changes nothing and announces nothing', () => {
    for (const tab of [{ home: true }, { window: {} }]) {
      const page = contents();
      const { scope, sent } = windowWith(page, tab);
      zoomShownPage(scope, (level) => level + 0.5);
      assert.deepEqual([page.level, sent], [0, []]);
    }
  });

  it("zooming never touches the shell's own contents", () => {
    const { scope } = windowWith(contents());
    zoomShownPage(scope, (level) => level + 1);
    assert.equal(scope.shell.window.webContents.level, 0);
  });

  it('a closed page reads 100 and is never zoomed', () => {
    const page = contents(1);
    page.isDestroyed = () => true;
    const { scope, sent } = windowWith(page);
    zoomShownPage(scope, () => 2);
    sendZoom(scope, false);
    assert.deepEqual([page.level, sent], [1, [['page-zoom', { percent: 100, announce: false }]]]);
  });
});
