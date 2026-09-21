/**
 * Unit tests for the HTTP path (src/client.ts), through the Oya client: how
 * the key, URL and body travel, and how a failed answer becomes an OyaError.
 */
import { describe, it, beforeEach, afterEach } from 'node:test';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import assert from 'node:assert/strict';
import { Oya, OyaError } from '../../dist/index.js';
import { BASE, client, fakeFetch } from './support/fake-fetch.ts';

describe('Oya construction', () => {
  const saved = { key: process.env.OYA_API_KEY, url: process.env.OYA_BASE_URL, home: process.env.OYA_CONFIG_HOME };
  // The client falls back to what `oya login` saved, so these tests point that
  // lookup at a directory of their own: otherwise they read whoever runs them.
  const scratch = mkdtempSync(join(tmpdir(), 'oya-sdk-config-'));
  beforeEach(() => (process.env.OYA_CONFIG_HOME = join(scratch, 'empty')));
  afterEach(() => {
    process.env.OYA_API_KEY = saved.key;
    process.env.OYA_BASE_URL = saved.url;
    process.env.OYA_CONFIG_HOME = saved.home;
    if (saved.key === undefined) delete process.env.OYA_API_KEY;
    if (saved.url === undefined) delete process.env.OYA_BASE_URL;
    if (saved.home === undefined) delete process.env.OYA_CONFIG_HOME;
  });

  it('refuses to start without an API key', () => {
    delete process.env.OYA_API_KEY;
    assert.throws(() => new Oya({ fetch: fakeFetch().fetch }), /No API key/);
  });

  it('falls back to the credentials `oya login` saved, and lets the environment win', async () => {
    const home = join(scratch, 'saved');
    mkdirSync(home, { recursive: true });
    writeFileSync(join(home, 'config.json'), JSON.stringify({ apiKey: 'k-file', baseUrl: `${BASE}/` }));
    process.env.OYA_CONFIG_HOME = home;
    delete process.env.OYA_API_KEY;
    delete process.env.OYA_BASE_URL;
    const fromFile = fakeFetch({ 'GET /api/browsers': { body: [] } });
    await new Oya({ fetch: fromFile.fetch }).browser.list();
    assert.equal(fromFile.calls[0].path, '/api/browsers');
    assert.equal(fromFile.calls[0].headers.Authorization, 'Bearer k-file');
    process.env.OYA_API_KEY = 'k-env';
    const fromEnv = fakeFetch({ 'GET /api/browsers': { body: [] } });
    await new Oya({ fetch: fromEnv.fetch }).browser.list();
    assert.equal(fromEnv.calls[0].headers.Authorization, 'Bearer k-env');
  });

  it('reads the key and URL from the environment, trimming trailing slashes', async () => {
    process.env.OYA_API_KEY = 'k-env';
    process.env.OYA_BASE_URL = 'https://env.test//';
    const seen: string[] = [];
    const fetch = (async (url: string, init: RequestInit) => {
      seen.push(`${url} ${(init.headers as Record<string, string>).Authorization}`);
      return new Response('[]');
    }) as typeof globalThis.fetch;
    await new Oya({ fetch }).browser.list();
    assert.deepEqual(seen, ['https://env.test/api/browsers Bearer k-env']);
  });

  it('prefers options over the environment', async () => {
    process.env.OYA_API_KEY = 'k-env';
    const { oya, calls } = client({ 'GET /api/browsers': { body: [] } });
    await oya.browser.list();
    assert.equal(calls[0].headers.Authorization, 'Bearer k-test');
  });
});

describe('Http.request', () => {
  it('sends JSON only when there is a body', async () => {
    const { oya, calls } = client({ 'GET /api/usage': { body: {} }, 'POST /api/proxies': { body: { id: 'p' } } });
    await oya.usage();
    await oya.proxies.create({ url: 'http://u:p@h:1' });
    assert.equal(calls[0].headers['Content-Type'], undefined);
    assert.equal(calls[0].body, undefined);
    assert.equal(calls[1].headers['Content-Type'], 'application/json');
    assert.deepEqual(calls[1].body, { url: 'http://u:p@h:1' });
  });

  it("throws an OyaError carrying the server's message, status and body", async () => {
    const { oya } = client({ 'GET /api/usage': { status: 429, body: { error: 'quota' } } });
    await assert.rejects(oya.usage(), (e: unknown) => {
      assert.ok(e instanceof OyaError);
      assert.deepEqual([e.message, e.status, e.body], ['quota', 429, { error: 'quota' }]);
      return true;
    });
  });

  it('names the call when the server gave no message', async () => {
    const { oya } = client({ 'GET /api/usage': { status: 502, text: 'Bad gateway' } });
    await assert.rejects(oya.usage(), { message: 'GET /api/usage failed (502)', body: 'Bad gateway' });
  });

  it('explains an invalid key with the URL and the shell-beats-.env hint', async () => {
    const { oya } = client({ 'GET /api/usage': { status: 401, body: { error: 'Invalid API key' } } });
    await assert.rejects(oya.usage(), (e: Error) =>
      e.message.startsWith(`Invalid API key for ${BASE}. Check OYA_API_KEY`),
    );
  });

  it('answers null for an empty body and text for a non-JSON one', async () => {
    const { oya } = client({ 'GET /api/usage': [{ text: '' }, { text: 'plain' }] });
    assert.equal(await oya.usage(), null);
    assert.equal(await oya.usage(), 'plain');
  });
});
