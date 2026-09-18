/**
 * Unit tests for the MFA factor store: validation of each factor kind, the
 * site factor over the persona-wide one, never revealing a secret, and sealed
 * persistence in the data directory.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { Status } from '../../../../src/platform/http-status.ts';
import { ownDataDir } from '../../support/data-dir.ts';

const dir = ownDataDir('oya-mfa-factors-');
const factors = await import('../../../../src/modules/challenges/mfa-factors.ts');

/** A valid TOTP secret. */
const SECRET = 'GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ';
/** A relay on a public IP literal, so the SSRF check needs no DNS. */
const RELAY = 'https://93.184.216.34/sms';

describe('MFA factors', () => {
  beforeEach(() => factors.reset());

  it('refuses a factor kind it does not know', async () => {
    await assert.rejects(factors.set('p1', { type: 'push' }), {
      status: Status.BAD_REQUEST,
      message: 'mfa type must be one of totp, email, sms, gmail, graph',
    });
    await assert.rejects(factors.set('p1', null), { status: Status.BAD_REQUEST });
  });

  it('refuses a TOTP factor without a secret, or with one that does not decode', async () => {
    await assert.rejects(factors.set('p1', { type: 'totp' }), { message: 'a TOTP secret is required' });
    await assert.rejects(factors.set('p1', { type: 'totp', secret: 'not base32!' }), { status: Status.BAD_REQUEST });
  });

  it('refuses a mailbox factor without its refresh token or client id', async () => {
    await assert.rejects(factors.set('p1', { type: 'gmail', clientId: 'c' }), {
      message: 'a gmail refreshToken is required',
    });
    await assert.rejects(factors.set('p1', { type: 'graph', refreshToken: 'r' }), {
      message: 'a graph clientId is required',
    });
  });

  it('refuses a relay factor without a URL, or with one pointing at a private address', async () => {
    await assert.rejects(factors.set('p1', { type: 'sms' }), { message: 'a sms relay url is required' });
    await assert.rejects(factors.set('p1', { type: 'email', url: 'http://10.0.0.5/codes' }), {
      status: Status.BAD_REQUEST,
      message: /mfa relay url/,
    });
  });

  it('stores a factor and describes it by type only, never the secret', async () => {
    assert.deepEqual(await factors.set('p1', { type: 'totp', secret: SECRET }), { configured: true, type: 'totp' });
    assert.deepEqual(factors.describe('p1'), { configured: true, type: 'totp' });
    assert.deepEqual(factors.describe('p2'), { configured: false });
  });

  it('prefers a site’s own factor and falls back to the persona-wide one', async () => {
    await factors.set('p1', { type: 'totp', secret: SECRET });
    await factors.set('p1', { type: 'sms', url: RELAY }, 'portal.example');
    assert.equal(factors.load('p1', 'portal.example').type, 'sms');
    assert.equal(factors.load('p1', 'other.example').type, 'totp');
    assert.deepEqual(factors.describe('p1', 'portal.example'), {
      configured: true,
      type: 'sms',
      domain: 'portal.example',
    });
    assert.deepEqual(factors.describe('p1', 'other.example'), { configured: true, type: 'totp' });
  });

  it('loads nothing for a persona without factors', () => {
    assert.equal(factors.load('nobody', 'site.example'), null);
  });

  it('lists a persona’s site factors by domain, types only, sorted', async () => {
    await factors.set('p1', { type: 'totp', secret: SECRET });
    await factors.set('p1', { type: 'totp', secret: SECRET }, 'zeta.example');
    await factors.set('p1', { type: 'gmail', refreshToken: 'r', clientId: 'c' }, 'alpha.example');
    await factors.set('p10', { type: 'totp', secret: SECRET }, 'other.example');
    assert.deepEqual(factors.list('p1'), [
      { domain: 'alpha.example', type: 'gmail' },
      { domain: 'zeta.example', type: 'totp' },
    ]);
  });

  it('clears one site’s factor, or the persona-wide one, reporting whether there was one', async () => {
    await factors.set('p1', { type: 'totp', secret: SECRET });
    await factors.set('p1', { type: 'totp', secret: SECRET }, 'site.example');
    assert.equal(factors.clear('p1', 'site.example'), true);
    assert.equal(factors.clear('p1', 'site.example'), false);
    assert.equal(factors.describe('p1', 'site.example').domain, undefined);
    assert.equal(factors.clear('p1'), true);
    assert.equal(factors.describe('p1').configured, false);
  });

  it('clears every factor of one persona and no other', async () => {
    await factors.set('p1', { type: 'totp', secret: SECRET });
    await factors.set('p1', { type: 'totp', secret: SECRET }, 'a.example');
    await factors.set('p11', { type: 'totp', secret: SECRET });
    assert.equal(factors.clearAll('p1'), 2);
    assert.equal(factors.clearAll('p1'), 0);
    assert.equal(factors.describe('p11').configured, true);
  });

  it('persists factors sealed, and restores them from disk', async () => {
    await factors.set('p1', { type: 'totp', secret: SECRET });
    assert.ok(!readFileSync(join(dir, 'mfa.json'), 'utf8').includes(SECRET), 'the secret is not stored in the clear');
    factors.reset();
    factors.restore();
    assert.equal(factors.load('p1').secret, SECRET);
  });

  it('refuses to start from a store it cannot read', () => {
    writeFileSync(join(dir, 'mfa.json'), '{not json');
    assert.throws(() => factors.restore(), /Cannot read MFA settings/);
  });
});
