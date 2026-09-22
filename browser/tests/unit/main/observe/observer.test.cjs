/**
 * Unit tests for what the agent may read back: console entries and finished
 * requests, newest first, with failures findable and query strings dropped,
 * a portal puts member ids and tokens in a url, and this buffer leaves the machine.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { Observer, safeUrl } = require('../../../../main/observe/observer.cjs');
const { CONSOLE_MAX, NETWORK_MAX } = require('../../../../main/observe/constants.cjs');

describe('safeUrl', () => {
  it('keeps the origin and path and drops the query, fragment and credentials', () => {
    assert.equal(
      safeUrl('https://user:pw@portal.example.com/auth/case?memberId=JQU197A78052#step2'),
      'https://portal.example.com/auth/case',
    );
  });

  it('says so rather than passing through something unparsable', () => {
    assert.equal(safeUrl('not a url'), '[unparsable url]');
  });
});

describe('readConsole', () => {
  it('names the level Electron reports as a number', () => {
    const o = new Observer();
    o.addConsole({ level: 3, message: 'boom', sourceId: 'https://x.test/a.js?v=1', line: 7 });
    assert.deepEqual(o.readConsole()[0].level, 'error');
    assert.equal(o.readConsole()[0].source, 'https://x.test/a.js');
  });

  it('returns the newest first', () => {
    const o = new Observer();
    o.addConsole({ level: 2, message: 'first' });
    o.addConsole({ level: 2, message: 'second' });
    assert.deepEqual(
      o.readConsole().map((e) => e.message),
      ['second', 'first'],
    );
  });

  it('filters by level and by pattern', () => {
    const o = new Observer();
    o.addConsole({ level: 3, message: 'Failed to load resource: 400' });
    o.addConsole({ level: 1, message: 'just info' });
    assert.equal(o.readConsole({ level: 'error' }).length, 1);
    assert.equal(o.readConsole({ pattern: '40\\d' })[0].message.includes('400'), true);
    assert.equal(o.readConsole({ pattern: 'nothing here' }).length, 0);
  });

  it('a pattern that would backtrack forever is cut off and refused, so a page cannot freeze the app', () => {
    const o = new Observer();
    o.addConsole({ level: 1, message: 'a'.repeat(40) + '!' });
    const started = Date.now();
    assert.throws(() => o.readConsole({ pattern: '(a+)+$' }), {
      message: 'The pattern took too long to match. Use a simpler pattern, or plain text.',
    });
    assert.ok(Date.now() - started < 2000, `took ${Date.now() - started}ms`);
  });

  it('a slow pattern is refused for requests too, never answered with a quiet "nothing matched"', () => {
    const o = new Observer();
    o.addRequest({ url: 'https://x.test/' + 'a'.repeat(40) + '!', statusCode: 200 });
    assert.throws(() => o.readNetwork({ pattern: '(a+)+$' }), /took too long/);
  });

  it('treats an invalid pattern as plain text rather than throwing', () => {
    const o = new Observer();
    o.addConsole({ level: 3, message: 'a(b' });
    assert.equal(o.readConsole({ pattern: 'a(b' }).length, 1);
  });

  it('keeps only the most recent entries', () => {
    const o = new Observer();
    for (let i = 0; i < CONSOLE_MAX + 10; i++) o.addConsole({ level: 1, message: `m${i}` });
    assert.equal(o.console.length, CONSOLE_MAX);
    assert.equal(o.readConsole({ limit: 1 })[0].message, `m${CONSOLE_MAX + 9}`);
  });
});

describe('readNetwork', () => {
  it('finds the request the server refused, which is the question being asked', () => {
    const o = new Observer();
    o.addRequest({ url: 'https://portal.example.com/ok', method: 'GET', resourceType: 'script', statusCode: 200 });
    o.addRequest({
      url: 'https://portal.example.com/auth-workflow/v2?id=1',
      method: 'POST',
      resourceType: 'xhr',
      statusCode: 400,
    });
    const failed = o.readNetwork({ failedOnly: true });
    assert.equal(failed.length, 1);
    assert.deepEqual([failed[0].status, failed[0].url], [400, 'https://portal.example.com/auth-workflow/v2']);
  });

  it('counts a request that never got an answer as failed', () => {
    const o = new Observer();
    o.addRequest({ url: 'https://portal.example.com/x', method: 'GET', error: 'net::ERR_ABORTED' });
    assert.equal(o.readNetwork({ failedOnly: true })[0].error, 'net::ERR_ABORTED');
  });

  it('does not call a successful request failed, whatever Electron names its error', () => {
    const o = new Observer();
    o.addRequest({ url: 'https://portal.example.com/ok', method: 'GET', statusCode: 200, error: 'net::OK' });
    assert.equal(o.readNetwork({ failedOnly: true }).length, 0);
    assert.equal(o.readNetwork()[0].error, null);
  });

  it('leaves a 3xx alone: a redirect is not a refusal', () => {
    const o = new Observer();
    o.addRequest({ url: 'https://portal.example.com/r', method: 'GET', statusCode: 302 });
    assert.equal(o.readNetwork({ failedOnly: true }).length, 0);
  });

  it("ignores an extension's own traffic, which is never the answer to what failed", () => {
    const o = new Observer();
    o.addRequest({ url: 'chrome-extension://abc/icon.png', method: 'GET', error: 'net::ERR_FAILED' });
    o.addRequest({ url: 'devtools://devtools/x.js', method: 'GET', statusCode: 404 });
    o.addRequest({ url: 'https://portal.example.com/api', method: 'POST', statusCode: 400 });
    const failed = o.readNetwork({ failedOnly: true });
    assert.equal(failed.length, 1);
    assert.equal(failed[0].url, 'https://portal.example.com/api');
  });

  it('keeps only the most recent requests', () => {
    const o = new Observer();
    for (let i = 0; i < NETWORK_MAX + 5; i++)
      o.addRequest({ url: `https://x.test/${i}`, method: 'GET', statusCode: 200 });
    assert.equal(o.network.length, NETWORK_MAX);
  });
});
