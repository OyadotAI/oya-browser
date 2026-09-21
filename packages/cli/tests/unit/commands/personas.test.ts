/**
 * Unit tests for `oya personas` (src/commands/personas.ts): subcommands,
 * platform aliases and the listing.
 */
import { FLAGS, captured, fakeFetch } from '../support/harness.ts';
import { describe, it, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { cmdPersonas } from '../../../src/commands/personas.ts';

const PERSONA = {
  id: 'p1',
  name: 'Ada',
  isDefault: true,
  activeBrowsers: 1,
  maxConcurrent: null,
  proxy: { geo: 'US' },
  exit: null,
  fingerprint: { platform: 'MacIntel', timezone: 'UTC', locale: 'en-US', screen: '1x1', webgl: 'g' },
  mfa: { configured: true, type: 'totp' },
};

describe('oya personas', () => {
  afterEach(() => mock.restoreAll());

  it('lists personas with cap, geo, factor and default marker', async () => {
    fakeFetch({ 'GET /api/personas': { personas: [PERSONA] } });
    const { out } = await captured(() => cmdPersonas([], FLAGS));
    assert.equal(out, `p1  ${'Ada'.padEnd(20)} 1/∞ running  MacIntel · UTC  geo US  mfa:totp  (default)`);
  });

  it('new maps platform aliases and flags into the create body', async () => {
    const calls = fakeFetch({ 'POST /api/personas': PERSONA });
    await captured(() => cmdPersonas(['new', 'Ada'], { ...FLAGS, platform: 'mac', tz: 'UTC', max: '2', geo: 'DE' }));
    assert.deepEqual(calls[0].body, {
      name: 'Ada',
      prefs: { platform: 'MacIntel', timezone: 'UTC' },
      proxy: { geo: 'DE' },
      maxConcurrent: 2,
    });
  });

  it('new --preview shows the fingerprint and creates nothing', async () => {
    const calls = fakeFetch({ 'POST /api/personas/preview': { fingerprint: PERSONA.fingerprint } });
    const { out } = await captured(() => cmdPersonas(['new'], { ...FLAGS, preview: true, platform: 'windows' }));
    assert.deepEqual(
      calls.map((c) => [c.path, c.body]),
      [['/api/personas/preview', { prefs: { platform: 'Win32' } }]],
    );
    assert.equal(out, 'MacIntel · UTC · en-US · 1x1 · g');
  });

  it('edit sends only the flags given, and --max none removes the cap', async () => {
    const calls = fakeFetch({ 'PUT /api/personas/p1': PERSONA });
    await captured(() => cmdPersonas(['edit', 'p1'], { ...FLAGS, max: 'none' }));
    assert.deepEqual(calls[0].body, { maxConcurrent: null });
  });

  it('rm and delete remove, and every subcommand needs an id', async () => {
    const calls = fakeFetch({ 'DELETE /api/personas/p1': {} });
    const { out } = await captured(() => cmdPersonas(['delete', 'p1'], FLAGS));
    assert.equal(out, '✅ removed p1');
    assert.equal(calls.length, 1);
    for (const sub of ['edit', 'clone', 'rm']) await assert.rejects(cmdPersonas([sub], FLAGS), /Usage: oya personas/);
  });
});
