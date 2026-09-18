/**
 * Unit tests for attaching the driver to a page target through a fake
 * connection: the domains it enables, the dialog watch, the persona applied
 * only where the provider does not ship its own stealth, and a recording
 * moved from the old target.
 */
import { describe, it, mock } from 'node:test';
import assert from 'node:assert/strict';
import { previewProfile, newPersonaSeed } from '../../../../src/modules/personas/fingerprint.ts';
import { fakeDriver } from '../../support/cdp.ts';

/** A real persona fingerprint, generated as the server generates one. */
const FINGERPRINT = previewProfile(newPersonaSeed());

/** A driver whose browser attaches targets to predictable sessions. */
function browser(options: any = {}) {
  const made = fakeDriver(options);
  made.conn.replies['Target.attachToTarget'] = (p: any) => ({ sessionId: `s-${p.targetId}` });
  made.conn.replies['Browser.getVersion'] = { userAgent: 'Mozilla/5.0 HeadlessChrome/131.0.0.0 Safari/537.36' };
  return made;
}

describe('CDPDriver.attach', () => {
  it('attaches flat and enables the page domains in the new session', async () => {
    const { driver, conn } = browser();
    await driver.attach('t-9');
    assert.deepEqual(conn.sent('Target.attachToTarget')[0].params, { targetId: 't-9', flatten: true });
    assert.deepEqual(
      conn.calls.filter((c) => c.method.endsWith('.enable')).map((c) => [c.method, c.sessionId]),
      [
        ['Page.enable', 's-t-9'],
        ['Runtime.enable', 's-t-9'],
        ['DOM.enable', 's-t-9'],
        ['Network.enable', 's-t-9'],
      ],
    );
    assert.deepEqual([driver.targetId, driver.sessionId, driver.worldContext], ['t-9', 's-t-9', null]);
  });

  it('gives each target a fresh analyzer tag attribute', async () => {
    const { driver } = browser();
    await driver.attach('t-1');
    const first = driver.tagAttr;
    await driver.attach('t-2');
    assert.match(driver.tagAttr, /^data-[0-9a-f]{8}$/);
    assert.notEqual(driver.tagAttr, first);
  });

  it('carries on when a domain cannot be enabled', async () => {
    const { driver, conn } = browser();
    conn.replies['DOM.enable'] = new Error('not supported');
    await driver.attach('t-1');
    assert.equal(driver.sessionId, 's-t-1');
  });

  it('applies the persona’s user agent on a plain CDP browser', async () => {
    mock.method(console, 'warn', () => {});
    const { driver, conn } = browser({ fingerprint: FINGERPRINT });
    await driver.attach('t-1');
    const [override] = conn.sent('Emulation.setUserAgentOverride');
    assert.ok(override, 'the UA is overridden as a header, not only in JS');
    assert.doesNotMatch(override.params.userAgent, /HeadlessChrome/);
    assert.equal(override.params.acceptLanguage, FINGERPRINT.navigator.languages.join(','));
    assert.equal(override.params.platform, FINGERPRINT.navigator.platform);
    mock.restoreAll();
  });

  it('leaves device spoofing to a provider that ships its own stealth', async () => {
    const { driver, conn } = browser({ fingerprint: FINGERPRINT, provider: 'browserbase' });
    await driver.attach('t-1');
    assert.equal(conn.sent('Browser.getVersion').length, 0);
    assert.equal(conn.sent('Emulation.setUserAgentOverride').length, 0);
  });

  it('stops the recording on the old target before leaving it', async () => {
    const { driver } = browser();
    const stop = mock.fn(async () => {});
    driver.recordChannel = { stop };
    await driver.attach('t-2');
    assert.equal(stop.mock.callCount(), 1);
    assert.equal(driver.recordChannel, null);
  });
});
