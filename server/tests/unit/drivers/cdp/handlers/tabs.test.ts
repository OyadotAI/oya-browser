/**
 * Unit tests for the tab handlers against a fake CDP connection: listing page
 * targets, opening, switching and closing them, and re-attaching after the
 * attached one closes.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { TARGET, fakeDriver } from '../../../support/cdp.ts';

/** A browser with two pages and a service worker; attaching answers a new session. */
function browser() {
  const made = fakeDriver();
  made.conn.replies['Target.getTargets'] = {
    targetInfos: [
      { targetId: TARGET, type: 'page', url: 'https://a.example/', title: 'A' },
      { targetId: 't-2', type: 'page', url: 'https://b.example/', title: 'B' },
      { targetId: 'sw', type: 'service_worker', url: 'https://a.example/sw.js', title: '' },
    ],
  };
  made.conn.replies['Target.attachToTarget'] = (p: any) => ({ sessionId: `s-${p.targetId}` });
  return made;
}

describe('tabs', () => {
  it('lists page targets only, marking the attached one', async () => {
    const { driver } = browser();
    assert.deepEqual(await driver.dispatch('list-tabs'), {
      ok: true,
      data: {
        tabs: [
          { id: TARGET, url: 'https://a.example/', title: 'A', active: true },
          { id: 't-2', url: 'https://b.example/', title: 'B', active: false },
        ],
      },
    });
  });

  it('opens a tab and attaches to it', async () => {
    const { driver, conn } = browser();
    conn.replies['Target.createTarget'] = { targetId: 't-new' };
    assert.deepEqual(await driver.dispatch('open_tab', { url: 'https://c.example/' }), {
      ok: true,
      data: { id: 't-new', tab_id: 't-new' },
    });
    assert.deepEqual(conn.sent('Target.createTarget')[0].params, { url: 'https://c.example/' });
    assert.deepEqual([driver.targetId, driver.sessionId], ['t-new', 's-t-new']);
  });

  it('opens a blank tab when no URL is given', async () => {
    const { driver, conn } = browser();
    conn.replies['Target.createTarget'] = { targetId: 't-new' };
    await driver.dispatch('new-tab');
    assert.equal(conn.sent('Target.createTarget')[0].params.url, 'about:blank');
  });

  it('switches to a tab by id, under the client’s tab_id too', async () => {
    const { driver } = browser();
    assert.deepEqual(await driver.dispatch('switch_tab', { tab_id: 't-2' }), { ok: true, data: { id: 't-2' } });
    assert.equal(driver.targetId, 't-2');
  });

  it('requires a tab id to switch', async () => {
    const { driver } = browser();
    assert.deepEqual(await driver.dispatch('switch-tab', {}), { ok: false, error: 'tab id required' });
  });

  it('closes another tab without leaving the attached one', async () => {
    const { driver, conn } = browser();
    assert.deepEqual(await driver.dispatch('close-tab', { id: 't-2' }), { ok: true });
    assert.deepEqual(conn.sent('Target.closeTarget')[0].params, { targetId: 't-2' });
    assert.equal(driver.targetId, TARGET);
  });

  it('moves to another page after closing the attached one, never back to it', async () => {
    const { driver, conn } = browser();
    await driver.dispatch('close_tab');
    assert.deepEqual(conn.sent('Target.closeTarget')[0].params, { targetId: TARGET });
    assert.equal(driver.targetId, 't-2');
  });
});
