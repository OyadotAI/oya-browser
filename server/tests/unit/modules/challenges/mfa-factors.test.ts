/**
 * Unit tests for the MFA factor store: validation of each factor kind, the
 * site factor over the persona-wide one, never revealing a secret, sealed
 * persistence in the configured storage, and the one-time import of a legacy
 * mfa.json.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { Status } from '../../../../src/platform/http-status.ts';
import { ownDataDir } from '../../support/data-dir.ts';

const dir = ownDataDir('oya-mfa-factors-');
const factors = await import('../../../../src/modules/challenges/mfa-factors.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');

/** The table factors are kept in. */
const TABLE = 'mfa_factors';
/** Where a legacy mfa.json would sit. */
const LEGACY = join(dir, 'mfa.json');

/** A valid TOTP secret. */
const SECRET = 'GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ';
/** A relay on a public IP literal, so the SSRF check needs no DNS. */
const RELAY = 'https://93.184.216.34/sms';

describe('MFA factors', () => {
  beforeEach(async () => {
    factors.reset();
    await getConnection().delete(TABLE, {});
  });
  afterEach(() => mock.restoreAll());

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
    assert.equal(await factors.clear('p1', 'site.example'), true);
    assert.equal(await factors.clear('p1', 'site.example'), false);
    assert.equal(factors.describe('p1', 'site.example').domain, undefined);
    assert.equal(await factors.clear('p1'), true);
    assert.equal(factors.describe('p1').configured, false);
  });

  it('clears every factor of one persona and no other', async () => {
    await factors.set('p1', { type: 'totp', secret: SECRET });
    await factors.set('p1', { type: 'totp', secret: SECRET }, 'a.example');
    await factors.set('p11', { type: 'totp', secret: SECRET });
    assert.equal(await factors.clearAll('p1'), 2);
    assert.equal(await factors.clearAll('p1'), 0);
    assert.equal(factors.describe('p11').configured, true);
  });

  it('stores factors sealed, never the secret in the clear', async () => {
    await factors.set('p1', { type: 'totp', secret: SECRET });
    const rows = await getConnection().select(TABLE);
    assert.equal(rows.length, 1);
    assert.ok(!JSON.stringify(rows).includes(SECRET), 'the secret is not stored in the clear');
  });

  it('restores stored factors after a restart', async () => {
    await factors.set('p1', { type: 'totp', secret: SECRET });
    factors.reset();
    await factors.restore();
    assert.equal(factors.load('p1').secret, SECRET);
  });

  it('forgets a cleared factor across a restart', async () => {
    await factors.set('p1', { type: 'totp', secret: SECRET });
    await factors.clearAll('p1');
    factors.reset();
    await factors.restore();
    assert.equal(factors.load('p1'), null);
  });

  it('holds nothing new when the write fails, so memory matches storage', async () => {
    mock.method(getConnection(), 'upsert', async () => Promise.reject(new Error('storage down')));
    await assert.rejects(factors.set('p1', { type: 'totp', secret: SECRET }), /storage down/);
    assert.equal(factors.load('p1'), null);
  });

  it('takes in a legacy mfa.json once and sets the file aside', async () => {
    await factors.set('p1', { type: 'totp', secret: SECRET });
    const [row] = await getConnection().select(TABLE);
    await getConnection().delete(TABLE, {});
    writeFileSync(LEGACY, JSON.stringify({ [row.id]: row.value }));
    factors.reset();
    await factors.restore();
    assert.equal(factors.load('p1').secret, SECRET);
    assert.deepEqual([existsSync(LEGACY), existsSync(`${LEGACY}.imported`)], [false, true]);
  });

  it('refuses to start from a legacy file it cannot read', async () => {
    writeFileSync(LEGACY, '{not json');
    await assert.rejects(factors.restore(), SyntaxError);
  });
});
