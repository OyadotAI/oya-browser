/**
 * Unit tests for the login store's rules: which cookies are kept, when one
 * has expired, which hosts it is sent to, and which localStorage is kept.
 */
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  cookieKey,
  validCookie,
  expired,
  isRecord,
  hostsOf,
  carries,
  validOrigin,
  storageEntries,
} from '../../../../src/modules/personas/cookie-rules.ts';
import {
  MAX_COOKIE_HOSTS,
  MAX_ORIGIN_STORAGE_CHARS,
  MAX_STORAGE_KEY_CHARS,
} from '../../../../src/modules/personas/constants.ts';

describe('cookie identity and validity', () => {
  it('identifies a cookie by domain, path and name, with "/" as the default path', () => {
    assert.equal(cookieKey({ domain: '.a.com', path: '/x', name: 'sid' }), '.a.com|/x|sid');
    assert.equal(cookieKey({ domain: 'a.com', name: 'sid' }), 'a.com|/|sid');
  });

  it('keeps only cookies with a string name, value and a non-empty domain', () => {
    assert.equal(validCookie({ name: 'a', value: '', domain: 'a.com' }), true);
    assert.equal(validCookie({ name: 'a', value: 'v', domain: '' }), false);
    assert.equal(validCookie({ name: 'a', value: 1, domain: 'a.com' }), false);
    assert.ok(!validCookie(null));
  });
});

describe('expired', () => {
  afterEach(() => mock.timers.reset());

  it('is true once the expiry, in seconds, has passed', () => {
    mock.timers.enable({ apis: ['Date'], now: 2_000_000 });
    assert.equal(expired({ expirationDate: 1999 }), true);
    assert.equal(expired({ expires: 2000 }), true);
    assert.equal(expired({ expirationDate: 2001 }), false);
  });

  it('never expires a session cookie', () => {
    assert.equal(expired({}), false);
    assert.equal(expired({ expires: -1 }), false);
    assert.equal(expired({ expirationDate: 0 }), false);
  });
});

describe('isRecord', () => {
  it('is true only for plain objects', () => {
    assert.deepEqual([isRecord({}), isRecord([]), isRecord(null), isRecord('x')], [true, false, false, false]);
  });
});

describe('hostsOf', () => {
  it('lowercases hosts, drops a leading dot and anything not a string', () => {
    assert.deepEqual(hostsOf(['.A.com', 7, 'b.COM']), ['a.com', 'b.com']);
  });

  it('asks about at most the host limit', () => {
    const many = Array.from({ length: MAX_COOKIE_HOSTS + 5 }, (_, i) => `h${i}.com`);
    assert.equal(hostsOf(many).length, MAX_COOKIE_HOSTS);
  });
});

describe('carries', () => {
  it('sends a cookie to its own host', () => {
    assert.equal(carries({ domain: 'a.com' }, ['a.com']), true);
  });

  it('sends a domain cookie to subdomains', () => {
    assert.equal(carries({ domain: '.a.com' }, ['app.a.com']), true);
  });

  it('keeps a host-only cookie, or one without a leading dot, off subdomains', () => {
    assert.equal(carries({ domain: '.a.com', hostOnly: true }, ['app.a.com']), false);
    assert.equal(carries({ domain: 'a.com' }, ['app.a.com']), false);
  });

  it('never sends a cookie to a host that merely ends with the same letters', () => {
    assert.equal(carries({ domain: '.a.com' }, ['evila.com']), false);
  });
});

describe('validOrigin', () => {
  it('accepts an http(s) origin written canonically', () => {
    assert.equal(validOrigin('https://a.com'), true);
    assert.equal(validOrigin('http://a.com:8080'), true);
  });

  it('refuses paths, other schemes and non-URLs', () => {
    assert.equal(validOrigin('https://a.com/'), false);
    assert.equal(validOrigin('file:///etc'), false);
    assert.equal(validOrigin('not a url'), false);
  });
});

describe('storageEntries', () => {
  it('keeps string values under keys within the length limit', () => {
    assert.deepEqual(storageEntries({ a: '1', b: 2, ['k'.repeat(MAX_STORAGE_KEY_CHARS + 1)]: 'x' }), [['a', '1']]);
  });

  it('skips an origin that is not a record', () => {
    assert.equal(storageEntries(['a']), null);
  });

  it('skips an origin whose storage is too large', () => {
    assert.equal(storageEntries({ big: 'x'.repeat(MAX_ORIGIN_STORAGE_CHARS) }), null);
  });
});
