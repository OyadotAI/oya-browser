/**
 * Unit tests for site credentials: sealed per persona and site, looked up by
 * host or its registrable parent, and never handing back a password in a
 * description or list. Stored sealed in the configured storage, and taken in
 * once from a credentials.json left from before storage drivers.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { ownDataDir } from '../../support/data-dir.ts';

const dir = ownDataDir('oya-credentials-');
const credentials = await import('../../../../src/modules/personas/credentials.ts');
const { getConnection } = await import('../../../../src/platform/storage/index.ts');

/** The table credentials are kept in. */
const TABLE = 'persona_credentials';
const LOGIN = { username: 'me@example.com', password: 'hunter2' };

describe('domainOf', () => {
  it('files a site under its host, lowercased, without www or a port', async () => {
    assert.equal(credentials.domainOf('https://WWW.Example.com:8443/login'), 'example.com');
    assert.equal(credentials.domainOf('portal.example.com'), 'portal.example.com');
    assert.equal(credentials.domainOf('localhost'), 'localhost');
  });

  it('is null for a path or a stray character dressed as a host, so "../x y" is never stored as ".."', async () => {
    assert.equal(credentials.domainOf('../x y'), null);
    assert.equal(credentials.domainOf('..'), null);
  });

  it('keeps a single-label host such as an intranet name, so stored credentials stay reachable', async () => {
    assert.equal(credentials.domainOf('intranet'), 'intranet');
    assert.equal(credentials.domainOf('http://portal/login'), 'portal');
  });

  it('is null for a number or anything else that is not text', async () => {
    assert.equal(credentials.domainOf(42 as any), null);
    assert.equal(credentials.domainOf({} as any), null);
  });

  it('is null for something that is not a host', async () => {
    assert.equal(credentials.domainOf('http://'), null);
    assert.equal(credentials.domainOf('a b'), null);
  });
});

describe('site credentials', () => {
  beforeEach(async () => {
    credentials.reset();
    await getConnection().delete(TABLE, {});
  });
  afterEach(() => mock.restoreAll());

  it('refuses a domain that is not a host name, naming the field and what it needs', async () => {
    await assert.rejects(credentials.set('p', '../x y', { username: 'u', password: 'p' }), {
      status: 400,
      field: 'domain',
      message: 'domain must be a host name such as accounts.google.com, not "../x y"',
    });
  });

  it('stores a login and describes it by username only', async () => {
    assert.deepEqual(await credentials.set('p-1', 'https://www.example.com/login', LOGIN), {
      configured: true,
      domain: 'example.com',
      username: LOGIN.username,
    });
    assert.deepEqual(credentials.describe('p-1', 'example.com'), {
      configured: true,
      domain: 'example.com',
      username: LOGIN.username,
    });
  });

  it('refuses a missing domain, username or password with a 400', async () => {
    await assert.rejects(credentials.set('p-1', '', LOGIN), { status: 400, message: 'a domain is required' });
    await assert.rejects(credentials.set('p-1', 'a.com', { password: 'x' }), {
      status: 400,
      message: 'a username is required',
    });
    await assert.rejects(credentials.set('p-1', 'a.com', { username: 'x' }), {
      status: 400,
      message: 'a password is required',
    });
  });

  it('stores the login sealed, never the username or password in the clear', async () => {
    await credentials.set('p-1', 'example.com', LOGIN);
    const rows = await getConnection().select(TABLE);
    assert.equal(rows.length, 1);
    assert.doesNotMatch(JSON.stringify(rows), /hunter2|me@example/);
  });

  it('keeps nothing in memory when the write fails, so what is held matches what is stored', async () => {
    mock.method(getConnection(), 'upsert', async () => Promise.reject(new Error('storage down')));
    await assert.rejects(credentials.set('p-1', 'a.com', LOGIN), /storage down/);
    assert.equal(credentials.lookup('p-1', 'a.com'), null);
  });

  it('keeps a credential in memory when removing it from storage fails', async () => {
    await credentials.set('p-1', 'a.com', LOGIN);
    mock.method(getConnection(), 'delete', async () => Promise.reject(new Error('storage down')));
    await assert.rejects(credentials.clear('p-1', 'a.com'), /storage down/);
    assert.equal(credentials.lookup('p-1', 'a.com').password, 'hunter2');
  });

  it('looks up a subdomain through its registrable parent', async () => {
    await credentials.set('p-1', 'example.com', LOGIN);
    assert.deepEqual(credentials.lookup('p-1', 'login.example.com'), { domain: 'example.com', ...LOGIN });
  });

  it('prefers a credential filed under the exact host', async () => {
    await credentials.set('p-1', 'example.com', LOGIN);
    await credentials.set('p-1', 'login.example.com', { username: 'exact', password: 'p' });
    assert.equal(credentials.lookup('p-1', 'login.example.com').username, 'exact');
  });

  it("never finds another persona's credential", async () => {
    await credentials.set('p-1', 'example.com', LOGIN);
    assert.equal(credentials.lookup('p-2', 'example.com'), null);
    assert.deepEqual(credentials.describe('p-2', 'example.com'), { configured: false });
  });

  it('answers nothing for an unusable host', async () => {
    assert.equal(credentials.lookup('p-1', 'a b'), null);
    assert.deepEqual(credentials.describe('p-1', 'a b'), { configured: false });
  });

  it('lists the sites a persona can sign in to, sorted, usernames only', async () => {
    await credentials.set('p-1', 'b.com', { username: 'bee', password: 'x' });
    await credentials.set('p-1', 'a.com', { username: 'ay', password: 'y' });
    await credentials.set('p-2', 'c.com', LOGIN);
    assert.deepEqual(credentials.list('p-1'), [
      { domain: 'a.com', username: 'ay' },
      { domain: 'b.com', username: 'bee' },
    ]);
  });

  it('clears exactly one site, and says whether there was one', async () => {
    await credentials.set('p-1', 'a.com', LOGIN);
    assert.equal(await credentials.clear('p-1', 'www.a.com'), true);
    assert.equal(await credentials.clear('p-1', 'a.com'), false);
  });

  it('clears every site of one persona only', async () => {
    await credentials.set('p-1', 'a.com', LOGIN);
    await credentials.set('p-1', 'b.com', LOGIN);
    await credentials.set('p-2', 'a.com', LOGIN);
    assert.equal(await credentials.clearAll('p-1'), 2);
    assert.deepEqual(credentials.list('p-1'), []);
    assert.equal(credentials.list('p-2').length, 1);
    assert.equal(await credentials.clearAll('p-1'), 0);
  });

  it('reloads what was saved', async () => {
    await credentials.set('p-1', 'a.com', LOGIN);
    credentials.reset();
    await credentials.restore();
    assert.equal(credentials.lookup('p-1', 'a.com').password, 'hunter2');
  });

  it('forgets a cleared credential across a reload', async () => {
    await credentials.set('p-1', 'a.com', LOGIN);
    await credentials.clearAll('p-1');
    credentials.reset();
    await credentials.restore();
    assert.equal(credentials.lookup('p-1', 'a.com'), null);
  });

  it('takes in a legacy credentials.json once and sets the file aside', async () => {
    await credentials.set('p-1', 'a.com', LOGIN);
    const [row] = await getConnection().select(TABLE);
    await getConnection().delete(TABLE, {});
    const legacy = join(dir, 'credentials.json');
    writeFileSync(legacy, JSON.stringify({ [row.id]: row.value }));
    credentials.reset();
    await credentials.restore();
    assert.equal(credentials.lookup('p-1', 'a.com').password, 'hunter2');
    assert.equal(existsSync(legacy), false);
    assert.equal(existsSync(`${legacy}.imported`), true);
  });
});
