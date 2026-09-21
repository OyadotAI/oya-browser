/**
 * Unit tests for the navigation handlers against a fake CDP connection:
 * loading a URL, reloading, and walking the session history.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { SESSION, fakeDriver } from '../../../support/cdp.ts';

/** A driver whose page reports its URL and title. */
function page() {
  const made = fakeDriver();
  made.conn.evaluate = (e) =>
    e.includes('document.title') ? { url: 'https://site.example/', title: 'Site' } : undefined;
  return made;
}

describe('navigate', () => {
  it('loads the URL, assuming https, and answers with the page it reached', async () => {
    const { driver, conn } = page();
    const result = await driver.dispatch('navigate', { url: 'site.example' });
    assert.deepEqual(result, { ok: true, data: { url: 'https://site.example/', title: 'Site' } });
    assert.deepEqual(conn.sent('Page.navigate')[0], {
      method: 'Page.navigate',
      params: { url: 'https://site.example' },
      sessionId: SESSION,
    });
  });

  it('keeps an explicit http scheme', async () => {
    const { driver, conn } = page();
    await driver.dispatch('navigate', { url: 'HTTP://site.example' });
    assert.equal(conn.sent('Page.navigate')[0].params.url, 'HTTP://site.example');
  });

  it('requires a URL', async () => {
    const { driver } = page();
    assert.deepEqual(await driver.dispatch('navigate', {}), { ok: false, error: 'URL required' });
  });

  it('fails with the browser’s reason when the navigation does', async () => {
    const { driver, conn } = page();
    conn.replies['Page.navigate'] = { errorText: 'net::ERR_NAME_NOT_RESOLVED' };
    await assert.rejects(driver.dispatch('navigate', { url: 'nowhere.invalid' }), {
      message: 'Navigation failed: net::ERR_NAME_NOT_RESOLVED',
    });
  });
});

describe('reload and history', () => {
  /** A history of three entries, sitting on the middle one. */
  const HISTORY = { currentIndex: 1, entries: [{ id: 10 }, { id: 11 }, { id: 12 }] };

  it('reloads the page', async () => {
    const { driver, conn } = page();
    assert.deepEqual(await driver.dispatch('reload'), { ok: true });
    assert.deepEqual(conn.methods(), ['Page.reload']);
  });

  it('goes back and forward one entry', async () => {
    const { driver, conn } = page();
    conn.replies['Page.getNavigationHistory'] = HISTORY;
    await driver.dispatch('back');
    await driver.dispatch('forward');
    assert.deepEqual(
      conn.sent('Page.navigateToHistoryEntry').map((c) => c.params.entryId),
      [10, 12],
    );
  });

  it('says so when there is nowhere to go', async () => {
    const { driver, conn } = page();
    conn.replies['Page.getNavigationHistory'] = { currentIndex: 0, entries: [{ id: 1 }] };
    assert.deepEqual(await driver.dispatch('back'), { ok: false, error: 'Cannot go back' });
    assert.deepEqual(await driver.dispatch('forward'), { ok: false, error: 'Cannot go forward' });
    assert.equal(conn.sent('Page.navigateToHistoryEntry').length, 0);
  });
});
