/**
 * Unit tests for the shared command context (src/context.ts): the client's
 * key, the target browser, and JSON versus human output.
 */
import { FLAGS, captured, fakeFetch } from './support/harness.ts';
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { client, out, targetBrowser } from '../../src/context.ts';

describe('context', () => {
  afterEach(() => mock.restoreAll());

  it('refuses to build a client with no API key, with the next step', () => {
    assert.throws(() => client({}), { code: 'no_api_key', message: 'No API key.', hint: /Run oya login/ });
  });

  it('prints JSON with --json, and the human form otherwise', async () => {
    const json = await captured(() => out({ json: true }, { a: 1 }, () => console.log('human')));
    const human = await captured(() => out({}, { a: 1 }, () => console.log('human')));
    assert.deepEqual([json.out, human.out], ['{\n  "a": 1\n}', 'human']);
  });

  it('targets --id, else the newest browser', async () => {
    const calls = fakeFetch({
      'GET /api/browsers': [{ id: 'old' }, { id: 'new' }],
      'GET /api/browsers/new': { id: 'new' },
      'GET /api/browsers/x': { id: 'x' },
    });
    const oya = client(FLAGS);
    assert.equal((await targetBrowser(oya, FLAGS)).id, 'new');
    assert.equal((await targetBrowser(oya, { ...FLAGS, id: 'x' })).id, 'x');
    assert.deepEqual(
      calls.map((c) => c.path),
      ['/api/browsers', '/api/browsers/new', '/api/browsers/x'],
    );
  });

  it('refuses when no browser is running and none was named, with the next step', async () => {
    fakeFetch({ 'GET /api/browsers': [] });
    await assert.rejects(targetBrowser(client(FLAGS), FLAGS), { code: 'no_browser', hint: /oya start/ });
  });
});
