/**
 * Unit tests for src/main/tabs/tab-lifecycle.ts's zoom reports: a shown tab's
 * zoom reaches the toolbar when it mounts and when its page goes to another site,
 * read live, since tabs on one site share one zoom and are not told when it moves.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mountTab, updateTabUrl } from '../../../../src/main/tabs/tab-lifecycle.ts';

/** Chromium's zoom per site, shared by every page on it. */
const zoomBySite = new Map<string, number>();

/** A tab whose page's zoom is its site's. */
function tabOn(id: number, url: string): any {
  const webContents = {
    getZoomLevel: () => zoomBySite.get(new URL(webContents.url).origin) ?? 0,
    isDestroyed: () => false,
    url,
  };
  return { id, url, title: '', view: { webContents } };
}

/** A window over `tabs`, showing `active`, with the zoom reports its shell was sent. */
function windowOver(tabs: any[], active: number) {
  const sent = [];
  const deps: any = {
    overlays: { names: new Set(['legacy']) },
    shell: { window: { webContents: { focus() {} } }, send: (ch, data) => ch === 'page-zoom' && sent.push(data) },
    layout: { layoutActiveTab() {} },
    findBar: { follow() {} },
    tabs: { activeTabId: active, find: (id) => tabs.find((t) => t.id === id), sendTabList() {} },
  };
  return { deps, sent };
}

describe('tab zoom reports', () => {
  it('two tabs on one site share one zoom: switching back shows the shared value', () => {
    zoomBySite.clear();
    const [a, b] = [tabOn(1, 'https://same.test/a'), tabOn(2, 'https://same.test/b')];
    const { deps, sent } = windowOver([a, b], 2);
    zoomBySite.set('https://same.test', 1.5); // zoomed from tab 2; tab 1 is told nothing
    deps.tabs.activeTabId = 1;
    mountTab(deps, a);
    assert.deepEqual(sent, [{ percent: 131, announce: false }]);
  });

  it("a site change in the shown tab sends the new site's zoom, unannounced", () => {
    zoomBySite.clear();
    zoomBySite.set('https://zoomed.test', 0.5);
    const tab = tabOn(1, 'https://zoomed.test/');
    const { deps, sent } = windowOver([tab], 1);
    tab.view.webContents.url = 'https://plain.test/';
    updateTabUrl(deps.tabs, deps.shell, tab, 'https://plain.test/');
    assert.deepEqual(sent, [{ percent: 100, announce: false }]);
  });

  it('a navigation in a background tab sends no zoom', () => {
    const tab = tabOn(2, 'https://a.test/');
    const { deps, sent } = windowOver([tab], 1);
    updateTabUrl(deps.tabs, deps.shell, tab, 'https://b.test/');
    assert.deepEqual(sent, []);
  });
});
