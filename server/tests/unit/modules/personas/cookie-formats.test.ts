/**
 * Unit tests for the formats a persona's jar is exported in: Playwright's
 * `addCookies` shape and the Netscape cookies.txt that curl and yt-dlp read.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { FORMATS, formatJar } from '../../../../src/modules/personas/cookie-formats.ts';

const SESSION = {
  name: 'sid',
  value: 'abc',
  domain: '.site.test',
  path: '/',
  secure: true,
  httpOnly: true,
  sameSite: 'no_restriction',
  t: 5,
};
const HOST = {
  name: 'pref',
  value: 'dark',
  domain: 'app.site.test',
  hostOnly: true,
  expirationDate: 4_000_000_000.5,
  sameSite: 'lax',
};

describe('formatJar', () => {
  it('names the formats it knows', () => {
    assert.deepEqual(FORMATS, ['json', 'playwright', 'netscape']);
  });

  it('leaves the jar as it is stored for json, without the sync stamp', () => {
    assert.deepEqual(
      formatJar('json', [SESSION]),
      [{ ...SESSION, t: undefined }].map(({ t: _t, ...c }) => c),
    );
  });

  it("writes Playwright's addCookies shape: its sameSite spelling, -1 for a session cookie", () => {
    assert.deepEqual(formatJar('playwright', [SESSION, HOST]), [
      {
        name: 'sid',
        value: 'abc',
        domain: '.site.test',
        path: '/',
        secure: true,
        httpOnly: true,
        sameSite: 'None',
        expires: -1,
      },
      {
        name: 'pref',
        value: 'dark',
        domain: 'app.site.test',
        path: '/',
        secure: false,
        httpOnly: false,
        sameSite: 'Lax',
        expires: 4_000_000_000.5,
      },
    ]);
  });

  it('writes a Netscape cookies.txt: subdomain flag, whole-second expiry, the HttpOnly prefix', () => {
    assert.equal(
      formatJar('netscape', [SESSION, HOST]),
      [
        '# Netscape HTTP Cookie File',
        '#HttpOnly_.site.test\tTRUE\t/\tTRUE\t0\tsid\tabc',
        'app.site.test\tFALSE\t/\tFALSE\t4000000000\tpref\tdark',
        '',
      ].join('\n'),
    );
  });

  it('answers undefined for a format it does not know', () => {
    assert.equal(formatJar('yaml', [SESSION]), undefined);
    assert.equal(formatJar('constructor', [SESSION]), undefined);
  });
});
