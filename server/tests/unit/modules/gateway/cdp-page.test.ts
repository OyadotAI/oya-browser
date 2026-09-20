/**
 * Unit tests for openPage: a second CDP connection attached to the browser's
 * first page, against a loopback browser.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { openPage } from '../../../../src/modules/gateway/cdp-page.ts';
import { fakeCdp, pageBrowser } from '../../support/gateway.ts';

describe('openPage', () => {
  it("attaches to the browser's first page with a flattened session", async () => {
    const browser = await fakeCdp(pageBrowser());
    const page = await openPage(browser.url);
    assert.equal(page.sessionId, 's-1');
    assert.deepEqual(browser.commands[1], {
      method: 'Target.attachToTarget',
      params: { targetId: 't-1', flatten: true },
      sessionId: undefined,
    });
    page.conn.close();
    await browser.close();
  });

  it('is null, with the connection closed, when the browser has no page', async () => {
    const browser = await fakeCdp((method) =>
      method === 'Target.getTargets' ? { targetInfos: [{ type: 'worker' }] } : {},
    );
    assert.equal(await openPage(browser.url), null);
    assert.equal(browser.commands.length, 1);
    await browser.close();
  });

  it('treats a browser that reports no targets as having no page', async () => {
    const browser = await fakeCdp(() => ({}));
    assert.equal(await openPage(browser.url), null);
    await browser.close();
  });
});
