/**
 * Unit tests for site credentials: sealed per persona and site, looked up by
 * host or its registrable parent, and never handing back a password in a
 * description or list.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { ownDataDir } from '../../support/data-dir.ts';

const dir = ownDataDir('oya-credentials-');
const credentials = await import('../../../../src/modules/personas/credentials.ts');
const LOGIN = { username: 'me@example.com', password: 'hunter2' };

describe('domainOf', () => {
  it('files a site under its host, lowercased, without www or a port', () => {
    assert.equal(credentials.domainOf('https://WWW.Example.com:8443/login'), 'example.com');
    assert.equal(credentials.domainOf('portal.example.com'), 'portal.example.com');
    assert.equal(credentials.domainOf('localhost'), 'localhost');
  });

  it('is null for something that is not a host', () => {
    assert.equal(credentials.domainOf('http://'), null);
    assert.equal(credentials.domainOf('a b'), null);
  });
});

describe('site credentials', () => {
  beforeEach(() => credentials.reset());

  it('stores a login and describes it by username only', () => {
    assert.deepEqual(credentials.set('p-1', 'https://www.example.com/login', LOGIN), {
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

  it('refuses a missing domain, username or password with a 400', () => {
    assert.throws(() => credentials.set('p-1', '', LOGIN), { status: 400, message: 'a domain is required' });
    assert.throws(() => credentials.set('p-1', 'a.com', { password: 'x' }), {
      status: 400,
      message: 'a username is required',
    });
    assert.throws(() => credentials.set('p-1', 'a.com', { username: 'x' }), {
      status: 400,
      message: 'a password is required',
    });
  });

  it('seals the password at rest, owner-only', () => {
    credentials.set('p-1', 'example.com', LOGIN);
    const raw = readFileSync(join(dir, 'credentials.json'), 'utf8');
    assert.doesNotMatch(raw, /hunter2|me@example/);
  });

  it('looks up a subdomain through its registrable parent', () => {
    credentials.set('p-1', 'example.com', LOGIN);
    assert.deepEqual(credentials.lookup('p-1', 'login.example.com'), { domain: 'example.com', ...LOGIN });
  });

  it('prefers a credential filed under the exact host', () => {
    credentials.set('p-1', 'example.com', LOGIN);
    credentials.set('p-1', 'login.example.com', { username: 'exact', password: 'p' });
    assert.equal(credentials.lookup('p-1', 'login.example.com').username, 'exact');
  });

  it("never finds another persona's credential", () => {
    credentials.set('p-1', 'example.com', LOGIN);
    assert.equal(credentials.lookup('p-2', 'example.com'), null);
    assert.deepEqual(credentials.describe('p-2', 'example.com'), { configured: false });
  });

  it('answers nothing for an unusable host', () => {
    assert.equal(credentials.lookup('p-1', 'a b'), null);
    assert.deepEqual(credentials.describe('p-1', 'a b'), { configured: false });
  });

  it('lists the sites a persona can sign in to, sorted, usernames only', () => {
    credentials.set('p-1', 'b.com', { username: 'bee', password: 'x' });
    credentials.set('p-1', 'a.com', { username: 'ay', password: 'y' });
    credentials.set('p-2', 'c.com', LOGIN);
    assert.deepEqual(credentials.list('p-1'), [
      { domain: 'a.com', username: 'ay' },
      { domain: 'b.com', username: 'bee' },
    ]);
  });

  it('clears exactly one site, and says whether there was one', () => {
    credentials.set('p-1', 'a.com', LOGIN);
    assert.equal(credentials.clear('p-1', 'www.a.com'), true);
    assert.equal(credentials.clear('p-1', 'a.com'), false);
  });

  it('clears every site of one persona only', () => {
    credentials.set('p-1', 'a.com', LOGIN);
    credentials.set('p-1', 'b.com', LOGIN);
    credentials.set('p-2', 'a.com', LOGIN);
    assert.equal(credentials.clearAll('p-1'), 2);
    assert.deepEqual(credentials.list('p-1'), []);
    assert.equal(credentials.list('p-2').length, 1);
    assert.equal(credentials.clearAll('p-1'), 0);
  });

  it('reloads what was saved', () => {
    credentials.set('p-1', 'a.com', LOGIN);
    credentials.reset();
    credentials.restore();
    assert.equal(credentials.lookup('p-1', 'a.com').password, 'hunter2');
  });
});
