/**
 * Unit tests for the browser half of the fleet view: tallies by client,
 * provider, health and persona, and the command totals.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { summarize } from '../../../../src/modules/fleet/summary.ts';

/** One browser row as the registry lists it. */
const row = (overrides = {}) => ({ health: 'ok', commands: 1, errors: 0, pending: 0, ...overrides });

describe('summarize', () => {
  it('summarises no browsers as zeroes, with every health bucket present', () => {
    assert.deepEqual(summarize([]), {
      total: 0,
      byClient: {},
      byProvider: {},
      byHealth: { ok: 0, stale: 0, errors: 0, dead: 0 },
      byPersona: {},
      commands: 0,
      errors: 0,
      pending: 0,
    });
  });

  it('counts browsers by client, treating a missing client type as oya', () => {
    const s = summarize([row(), row({ clientType: 'cdp' }), row({ clientType: 'cdp' })]);
    assert.deepEqual(s.byClient, { oya: 1, cdp: 2 });
  });

  it('counts providers only for browsers that have one', () => {
    const s = summarize([row(), row({ provider: 'anchor' })]);
    assert.deepEqual(s.byProvider, { anchor: 1 });
  });

  it('counts health, persona names before ids, and a dash for neither', () => {
    const s = summarize([
      row({ health: 'dead', personaName: 'Ada', persona: 'p-1' }),
      row({ health: 'stale', persona: 'p-2' }),
      row(),
    ]);
    assert.deepEqual(s.byHealth, { ok: 1, stale: 1, errors: 0, dead: 1 });
    assert.deepEqual(s.byPersona, { Ada: 1, 'p-2': 1, '—': 1 });
  });

  it('adds up commands, errors and pending across browsers', () => {
    const s = summarize([row({ commands: 3, errors: 1, pending: 2 }), row({ commands: 4, errors: 2, pending: 0 })]);
    assert.deepEqual([s.total, s.commands, s.errors, s.pending], [2, 7, 3, 2]);
  });
});
