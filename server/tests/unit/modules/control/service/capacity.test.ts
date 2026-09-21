/**
 * Unit tests for capacity arithmetic: browser and persona slots in use, and
 * money spent or reserved.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { atCapacity, spentUsd } from '../../../../../src/modules/control/service/capacity.ts';

/** A project with the given cap. */
const project = (maxConcurrent = null, costUsd = 0) => ({ settings: { maxConcurrent }, costUsd });

describe('atCapacity', () => {
  const sessions = [
    { id: 'a', state: 'ready', persona: 'p' },
    { id: 'b', state: 'queued', persona: 'p' },
    { id: 'c', state: 'disconnected', persona: 'p' },
  ];

  it('counts only sessions that hold a slot', () => {
    assert.equal(atCapacity(sessions, project(), { maxConcurrent: 2 }), false);
    assert.equal(atCapacity(sessions, project(), { maxConcurrent: 1 }), true);
  });

  it('uses the tighter of the project’s and caller’s caps, or whichever is set', () => {
    assert.equal(atCapacity(sessions, project(1), { maxConcurrent: 5 }), true);
    assert.equal(atCapacity(sessions, project(1), {}), true);
    assert.equal(atCapacity(sessions, project(5), { maxConcurrent: 2 }), false);
  });

  it('applies a persona’s own limit', () => {
    assert.equal(atCapacity(sessions, project(), { persona: 'p', personaLimit: 1 }), true);
    assert.equal(atCapacity(sessions, project(), { persona: 'q', personaLimit: 1 }), false);
  });

  it('leaves out the session being re-checked', () => {
    assert.equal(atCapacity(sessions, project(), { maxConcurrent: 1 }, 'a'), false);
  });

  it('is never full without any cap', () => {
    assert.equal(atCapacity(sessions, project(), {}), false);
  });
});

describe('spentUsd', () => {
  it('adds live reservations to what the project has already been billed', () => {
    const sessions = [
      { state: 'ready', reservedCostUsd: 0.5 },
      { state: 'stopped', reservedCostUsd: 9 },
      { state: 'queued' },
    ];
    assert.equal(spentUsd(sessions, project(null, 2)), 2.5);
    assert.equal(spentUsd([], { settings: {} }), 0);
  });
});
