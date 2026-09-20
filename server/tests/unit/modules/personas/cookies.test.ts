/**
 * Unit tests for the persona login store: cookies and localStorage per
 * persona, the sealed file they are saved to, and the migration of the old
 * key-per-jar file. The module loads its file on import, so this file gets
 * its own data directory with a legacy file in it before importing.
 */
import { describe, it, after } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { ownDataDir } from '../../support/data-dir.ts';

const dir = ownDataDir('oya-cookies-');
const FAR = 4_000_000_000;
writeFileSync(
  join(dir, 'cookies.json'),
  JSON.stringify({
    'legacy-api-key': [{ name: 'sid', value: 'k', domain: 'a.com', expirationDate: FAR }],
    'p-0123456789abcdef': [{ name: 'sid', value: 'p', domain: 'b.com' }],
    'not-a-jar': 'ignored',
  }),
);
const cookies = await import('../../../../src/modules/personas/cookies.ts');
const { defaultPersonaSeed } = await import('../../../../src/modules/personas/fingerprint.ts');
const { openText } = await import('../../../../src/platform/secrets.ts');

after(() => cookies.drain());

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
  it('writes every persona sealed under its own scope, owner-only', async () => {
    cookies.mergeDump('p-saved', [cookie('a', 'a.com')]);
    await cookies.drain();
    const file = JSON.parse(readFileSync(join(dir, 'cookies.json'), 'utf8'));
    assert.equal(file.version, 2);
    assert.doesNotMatch(JSON.stringify(file), /v-a/);
    const state = openText('login:p-saved', file.records['p-saved']);
    assert.equal(state.cookies[0].value, 'v-a');
    assert.throws(() => openText('login:p-other', file.records['p-saved']));
  });
});
