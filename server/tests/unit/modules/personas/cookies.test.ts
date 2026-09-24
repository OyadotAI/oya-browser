/**
 * Unit tests for the persona login store: cookies and localStorage per
 * persona, the sealed rows they are saved to in the configured storage, and
 * the one-time import of the old key-per-jar cookies.json. This file gets its
 * own data directory with a legacy file in it, then restores from it.
 */
import { describe, it, after, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { ownDataDir } from '../../support/data-dir.ts';

const dir = ownDataDir('oya-cookies-');
/** An expiry far in the future, in seconds, so a cookie stays live. */
const FAR = 4_000_000_000;
/** Where the legacy file sits. */
const LEGACY = join(dir, 'cookies.json');
writeFileSync(
  LEGACY,
  JSON.stringify({
    'legacy-api-key': [{ name: 'sid', value: 'k', domain: 'a.com', expirationDate: FAR }],
    'p-0123456789abcdef': [{ name: 'sid', value: 'p', domain: 'b.com' }],
    'not-a-jar': 'ignored',
  }),
);
const cookies = await import('../../../../src/modules/personas/cookies.ts');
const { defaultPersonaSeed } = await import('../../../../src/modules/personas/fingerprint.ts');
const { openText } = await import('../../../../src/platform/secrets.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');
await cookies.restore();

/** The table login state is kept in. */
const TABLE = 'persona_logins';

/** The stored row for one persona, or undefined. */
const storedRow = async (id: string) => (await getConnection().select(TABLE, { id }))[0];

after(() => cookies.drain());
afterEach(() => mock.restoreAll());

/** A valid cookie. */
const cookie = (name: string, domain: string, extra: object = {}) => ({ name, value: `v-${name}`, domain, ...extra });

describe('loading the old key-per-jar file', () => {
  it("moves an API key's jar to that key's default persona", () => {
    const id = defaultPersonaSeed('legacy-api-key').id;
    assert.deepEqual(
      cookies.getAll(id).map((c) => c.value),
      ['k'],
    );
    assert.deepEqual(cookies.getAll('legacy-api-key'), []);
  });

  it('keeps a jar already named by a persona id', () => {
    assert.equal(cookies.getAll('p-0123456789abcdef')[0].value, 'p');
  });

  it('stores the imported jars and sets the file aside', async () => {
    assert.ok(await storedRow('p-0123456789abcdef'));
    assert.deepEqual([existsSync(LEGACY), existsSync(`${LEGACY}.imported`)], [false, true]);
  });
});

describe('cookie jar', () => {
  it('merges a dump, keeping valid cookies and returning the unexpired ones', () => {
    const kept = cookies.mergeDump('p-merge', [cookie('a', 'a.com'), { name: 'bad' }, cookie('b', 'b.com')]);
    assert.deepEqual(kept.map((c) => c.name).sort(), ['a', 'b']);
  });

  it('removes a cookie when the dump carries it expired', () => {
    cookies.mergeDump('p-expire', [cookie('a', 'a.com')]);
    cookies.mergeDump('p-expire', [cookie('a', 'a.com', { expirationDate: 1 })]);
    assert.deepEqual(cookies.getAll('p-expire'), []);
  });

  it('stamps a cookie when it changes, and leaves the stamp alone when a dump repeats it', () => {
    mock.timers.enable({ apis: ['Date'], now: 5000 });
    cookies.mergeDump('p-stamp', [cookie('sid', 'a.com'), cookie('pref', 'a.com')]);
    mock.timers.setTime(9000);
    cookies.mergeDump('p-stamp', [cookie('sid', 'a.com', { value: 'fresh login' }), cookie('pref', 'a.com')]);
    cookies.applyChange('p-stamp', { cookie: cookie('cart', 'a.com') });
    const stamps = Object.fromEntries(cookies.getAll('p-stamp').map((c) => [c.name, c.t]));
    mock.timers.reset();
    assert.deepEqual(stamps, { sid: 9000, pref: 5000, cart: 9000 });
  });

  it('ignores a dump without a persona or a list', () => {
    assert.deepEqual(cookies.mergeDump('', [cookie('a', 'a.com')]), []);
    assert.deepEqual(cookies.mergeDump('p-x', 'nope' as any), []);
  });

  it('applies one added or removed cookie', () => {
    cookies.applyChange('p-change', { cookie: cookie('a', 'a.com') });
    assert.equal(cookies.getAll('p-change').length, 1);
    cookies.applyChange('p-change', { cookie: cookie('a', 'a.com'), removed: true });
    assert.equal(cookies.getAll('p-change').length, 0);
  });

  it('ignores a change without a persona or a valid cookie', () => {
    assert.equal(cookies.applyChange('p-change', { cookie: { name: 'x' } }), null);
    assert.equal(cookies.applyChange('', { cookie: cookie('a', 'a.com') }), null);
  });

  it('keeps each persona’s jar to itself', () => {
    cookies.mergeDump('p-mine', [cookie('a', 'a.com')]);
    assert.deepEqual(cookies.getAll('p-theirs'), []);
  });

  it('returns only the cookies a request to the asked hosts would carry', () => {
    cookies.mergeDump('p-hosts', [cookie('dom', '.a.com'), cookie('host', 'a.com'), cookie('other', 'b.com')]);
    assert.deepEqual(
      cookies.getForDomains('p-hosts', ['app.a.com']).map((c) => c.name),
      ['dom'],
    );
    assert.deepEqual(cookies.getForDomains('p-hosts', 'a.com' as any), []);
  });
});

describe('localStorage', () => {
  it('replaces only the origins given, including emptied ones', () => {
    cookies.mergeStorage('p-store', { 'https://a.com': { k: '1' }, 'https://b.com': { k: '2' } });
    cookies.mergeStorage('p-store', { 'https://a.com': {} });
    assert.deepEqual(cookies.getStorage('p-store'), { 'https://a.com': {}, 'https://b.com': { k: '2' } });
  });

  it('skips origins that are not canonical http(s) origins', () => {
    cookies.mergeStorage('p-bad-origin', { 'https://a.com/path': { k: '1' }, 'javascript:alert(1)': { k: '1' } });
    assert.deepEqual(cookies.getStorage('p-bad-origin'), {});
  });

  it('ignores storage that is not a record', () => {
    cookies.mergeStorage('p-none', ['x'] as any);
    assert.deepEqual(cookies.getStorage('p-none'), {});
  });
});

describe('summary and clear', () => {
  it('lists cookie counts and sites without any values', () => {
    cookies.mergeDump('p-sum', [cookie('a', '.a.com'), cookie('b', 'b.com')]);
    cookies.mergeStorage('p-sum', { 'https://c.com': { token: 'secret' }, 'https://d.com': {} });
    const s = cookies.summary('p-sum');
    assert.deepEqual([s.cookies, s.sites], [2, ['a.com', 'b.com', 'c.com']]);
    assert.ok(s.updatedAt);
    assert.doesNotMatch(JSON.stringify(s), /secret|v-a/);
  });

  it('forgets a persona’s cookies and storage', () => {
    cookies.mergeDump('p-clear', [cookie('a', 'a.com')]);
    cookies.mergeStorage('p-clear', { 'https://a.com': { k: '1' } });
    cookies.clear('p-clear');
    assert.deepEqual([cookies.getAll('p-clear'), cookies.getStorage('p-clear')], [[], {}]);
    assert.equal(cookies.summary('p-clear').cookies, 0);
  });
});

describe('saving', () => {
  it('writes a persona sealed under its own scope, never a value in the clear', async () => {
    cookies.mergeDump('p-saved', [cookie('a', 'a.com')]);
    await cookies.drain();
    const row = await storedRow('p-saved');
    assert.doesNotMatch(JSON.stringify(row), /v-a/);
    assert.equal(openText('login:p-saved', row.value).cookies[0].value, 'v-a');
    assert.throws(() => openText('login:p-other', row.value));
  });

  it('writes only the personas that changed', async () => {
    await cookies.drain();
    const upsert = mock.method(getConnection(), 'upsert');
    cookies.mergeDump('p-only', [cookie('a', 'a.com')]);
    await cookies.drain();
    assert.deepEqual(
      upsert.mock.calls.flatMap((c) => c.arguments[1].map((r) => r.id)),
      ['p-only'],
    );
  });

  it('removes a cleared persona from storage', async () => {
    cookies.mergeDump('p-gone', [cookie('a', 'a.com')]);
    await cookies.drain();
    cookies.clear('p-gone');
    await cookies.drain();
    assert.equal(await storedRow('p-gone'), undefined);
  });

  it('keeps a change whose write failed, and writes it on the next save', async () => {
    const upsert = mock.method(getConnection(), 'upsert', async () => Promise.reject(new Error('storage down')));
    cookies.mergeDump('p-retry', [cookie('a', 'a.com')]);
    await assert.rejects(cookies.drain(), /storage down/);
    upsert.mock.restore();
    await cookies.drain();
    assert.ok(await storedRow('p-retry'));
  });

  it('restores stored personas, as after a restart', async () => {
    cookies.mergeDump('p-back', [cookie('a', 'a.com')]);
    await cookies.drain();
    const row = await storedRow('p-back');
    cookies.clear('p-back');
    await cookies.drain();
    // Put back as a previous run left it, with nothing of it in memory.
    await getConnection().upsert(TABLE, [row]);
    await cookies.restore();
    assert.equal(cookies.getAll('p-back')[0].value, 'v-a');
  });
});
