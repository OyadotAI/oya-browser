/**
 * Unit tests for billing's facade: it builds its parts, and reports usage on
 * an interval only on the hosted deployment.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  createBilling,
  startReporting,
  Entitlements,
  Subscriptions,
  UsageReporter,
} from '../../../../src/modules/billing/index.ts';
import { REPORT_INTERVAL_MS } from '../../../../src/modules/billing/constants.ts';

/** Wiring that owns nothing and runs nothing. */
const WIRING = {
  ownerOf: async () => null,
  keyDigestsOf: async () => new Set<string>(),
  keyDigest: (k) => k,
  cloudKeys: () => [],
  billTo: () => {},
  ownsModel: () => false,
  licenseAdmit: () => ({ ok: true, message: '' }),
  consoleUrl: () => 'https://console.test',
};

describe('billing facade', () => {
  afterEach(() => {
    delete process.env.STRIPE_SECRET_KEY;
    mock.timers.reset();
  });

  it('builds admission, subscriptions and the reporter, sending people back to the console’s billing page', () => {
    const billing = createBilling(WIRING);
    assert.ok(billing.entitlements instanceof Entitlements);
    assert.ok(billing.subscriptions instanceof Subscriptions);
    assert.ok(billing.reporter instanceof UsageReporter);
    assert.equal(billing.entitlements.deps.upgradeUrl(), 'https://console.test/dashboard/billing');
  });

  it('reports on an interval on the hosted deployment, until stopped', () => {
    process.env.STRIPE_SECRET_KEY = 'sk_test';
    mock.timers.enable({ apis: ['setInterval'] });
    let reports = 0;
    const stop = startReporting({ report: async () => void reports++ } as any);
    mock.timers.tick(REPORT_INTERVAL_MS);
    stop();
    mock.timers.tick(REPORT_INTERVAL_MS);
    assert.equal(reports, 1);
  });

  it('reports nothing on a self-hosted server', () => {
    mock.timers.enable({ apis: ['setInterval'] });
    let reports = 0;
    const stop = startReporting({ report: async () => void reports++ } as any);
    mock.timers.tick(REPORT_INTERVAL_MS);
    stop();
    assert.equal(reports, 0);
  });
});
