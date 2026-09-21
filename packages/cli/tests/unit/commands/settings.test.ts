/**
 * Unit tests for `oya config` and `oya usage` (src/commands/settings.ts).
 */
import { FLAGS, captured, fakeFetch } from '../support/harness.ts';
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { cmdConfig, cmdUsage } from '../../../src/commands/settings.ts';

describe('oya config', () => {
  afterEach(() => mock.restoreAll());

  it('shows the config as JSON with no arguments', async () => {
    fakeFetch({ 'GET /api/config': { chat_model: 'm' } });
    assert.equal((await captured(() => cmdConfig([], FLAGS))).out, '{\n  "chat_model": "m"\n}');
  });

  it('sets key=value pairs, keeping any = in the value', async () => {
    const calls = fakeFetch({ 'POST /api/config': {} });
    const { out } = await captured(() => cmdConfig(['a=1', 'b=x=y'], FLAGS));
    assert.deepEqual(calls[0].body, { a: '1', b: 'x=y' });
    assert.equal(out, '✅ updated a, b');
  });

  it('refuses a pair without a key', async () => {
    fakeFetch({});
    await assert.rejects(cmdConfig(['=x'], FLAGS), /Expected key=value, got "=x"/);
  });

  it('usage prints the usage as JSON', async () => {
    fakeFetch({ 'GET /api/usage': { usd: 1 } });
    assert.equal((await captured(() => cmdUsage(FLAGS))).out, '{\n  "usd": 1\n}');
  });
});
