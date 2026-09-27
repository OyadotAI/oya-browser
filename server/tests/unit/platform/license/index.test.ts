/**
 * Unit tests for the license facade: it holds the free cap itself when the
 * vendored module is missing, and otherwise asks the module.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { writeFileSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import * as license from '../../../../src/platform/license/index.ts';

describe('license facade', () => {
  afterEach(() => {
    license.setModuleForTests(null);
    mock.restoreAll();
  });

  it('without the module, allows five cloud browsers at once and refuses the sixth, naming who sells licenses', () => {
    assert.equal(license.admit(5).ok, true);
    const refused = license.admit(6);
    assert.equal(refused.ok, false);
    assert.match(refused.message, /above 5 concurrent cloud browsers needs a commercial license: sales@getoya\.ai/);
  });

  it('keeps the free cap when the module file is missing, and says so', async () => {
    const warn = mock.method(console, 'warn', () => {});
    await license.load(pathToFileURL(join(tmpdir(), 'no-such-oya-license.js')).href);
    assert.equal(license.admit(6).ok, false);
    assert.match(String(warn.mock.calls[0].arguments[0]), /capped at 5/);
  });

  it('asks the module once it is loaded', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'oya-license-'));
    const file = join(dir, 'fake.mjs');
    writeFileSync(file, 'export default { start() {}, admit: (n) => ({ ok: n <= 50, message: "fifty" }) };');
    await license.load(pathToFileURL(file).href);
    assert.deepEqual(license.admit(50), { ok: true, message: 'fifty' });
  });

  it('loads the vendored module this repository ships', async () => {
    await license.load();
    assert.equal(license.admit(5).ok, true);
    assert.equal(license.admit(6).ok, false);
  });

  it('starts the module with the server’s details and where to ping, and never throws when it fails', async () => {
    let given: any;
    license.setModuleForTests({ start: (deps) => void (given = deps), admit: () => ({ ok: true, message: '' }) });
    await license.start({ version: '1.2.3', browsers: () => 0 });
    assert.equal(given.version, '1.2.3');
    assert.equal(given.pingUrl, 'https://oyabrowser.com/api/telemetry/ping');
    assert.equal(given.freeCap, 5);
    license.setModuleForTests({
      start: async () => Promise.reject(new Error('boom')),
      admit: () => ({ ok: true, message: '' }),
    });
    await assert.doesNotReject(license.start({ version: '1', browsers: () => 0 }));
  });

  it('is never the hosted deployment without the module, and asks the module otherwise', () => {
    assert.equal(license.hostedDeployment(), false);
    license.setModuleForTests({ start() {}, admit: () => ({ ok: true, message: '' }), isHosted: () => true });
    assert.equal(license.hostedDeployment(), true);
  });

  it('issues a license only where the signing key is set, and says what is wrong with a bad request', () => {
    const minted = {
      license: { id: 'L', licensee: 'A', maxConcurrent: 9, expiresAt: '2030-01-01T00:00:00.000Z' },
      token: 't',
    };
    const fake = {
      start() {},
      admit: () => ({ ok: true, message: '' }),
      mint: (req: any) =>
        req.licensee
          ? minted
          : (() => {
              throw new Error('licensee is required');
            })(),
    };
    license.setModuleForTests(fake);
    assert.throws(() => license.mint({ licensee: 'A' }), { status: 503 });
    process.env.OYA_LICENSE_SIGNING_KEY = 'k';
    try {
      assert.deepEqual(license.mint({ licensee: 'A' }), minted);
      assert.throws(() => license.mint({}), { status: 400, message: 'licensee is required' });
    } finally {
      delete process.env.OYA_LICENSE_SIGNING_KEY;
    }
  });

  it('starts nothing when there is no module', async () => {
    await assert.doesNotReject(license.start({ version: '1', browsers: () => 0 }));
  });
});
