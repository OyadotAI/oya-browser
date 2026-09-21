/**
 * Unit tests for `oya cookies` (src/commands/cookies.ts): moving a persona's
 * logins to a file, from a file, and to another persona.
 */
import { FLAGS, captured, fakeFetch } from '../support/harness.ts';
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { cmdCookies } from '../../../src/commands/cookies.ts';

const COOKIE = { name: 'sid', value: 'v', domain: '.x.com' };
const dir = mkdtempSync(join(tmpdir(), 'oya-cli-cookies-'));

describe('oya cookies', () => {
  afterEach(() => mock.restoreAll());

  it('export prints the jar, ready for Playwright when asked', async () => {
    const calls = fakeFetch({ 'GET /api/pool/cookies?persona=p1&format=playwright': { cookies: [COOKIE] } });
    const { out } = await captured(() => cmdCookies(['export', 'p1'], { ...FLAGS, format: 'playwright' }));
    assert.deepEqual(JSON.parse(out), [COOKIE]);
    assert.equal(calls.length, 1);
  });

  it('export --out writes a file only its owner can read, and says how many cookies went in', async () => {
    fakeFetch({ 'GET /api/pool/cookies?persona=p1&format=json': { cookies: [COOKIE] } });
    const file = join(dir, 'jar.json');
    const { out } = await captured(() => cmdCookies(['export', 'p1'], { ...FLAGS, out: file }));
    assert.deepEqual(JSON.parse(readFileSync(file, 'utf8')), [COOKIE]);
    assert.equal(statSync(file).mode & 0o777, 0o600);
    assert.match(out, /1 cookie.*jar\.json/);
  });

  it('import reads a list, or an export wrapped in { cookies }, and says what was merged', async () => {
    const calls = fakeFetch({ 'PUT /api/pool/cookies?persona=p2': { imported: 1, skipped: 0, total: 5 } });
    const file = join(dir, 'in.json');
    writeFileSync(file, JSON.stringify({ cookies: [COOKIE] }));
    const { out } = await captured(() => cmdCookies(['import', 'p2', file], FLAGS));
    assert.deepEqual(calls[0].body, { cookies: [COOKIE] });
    assert.match(out, /1 imported, 0 skipped, 5 in the jar/);
  });

  it('copy moves one persona’s logins into another', async () => {
    const calls = fakeFetch({
      'GET /api/pool/cookies?persona=p1&format=json': { cookies: [COOKIE] },
      'PUT /api/pool/cookies?persona=p2': { imported: 1, skipped: 0, total: 1 },
    });
    await captured(() => cmdCookies(['copy', 'p1', 'p2'], FLAGS));
    assert.deepEqual(
      calls.map((c) => c.method),
      ['GET', 'PUT'],
    );
  });

  it('says how to use it when the arguments are missing, or the file holds no cookie list', async () => {
    await assert.rejects(cmdCookies([], FLAGS), /Usage: oya cookies/);
    await assert.rejects(cmdCookies(['import', 'p2'], FLAGS), /Usage: oya cookies import/);
    const file = join(dir, 'bad.json');
    writeFileSync(file, '{"nope":1}');
    await assert.rejects(cmdCookies(['import', 'p2', file], FLAGS), /no list of cookies/);
  });
});
