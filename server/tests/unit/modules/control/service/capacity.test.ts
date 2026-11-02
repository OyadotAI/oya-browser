/**
 * Unit tests for capacity arithmetic: browser and persona slots in use, and
 * money spent or reserved.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { atCapacity, leastLoaded, spentUsd } from '../../../../../src/modules/control/service/capacity.ts';

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

describe('leastLoaded', () => {
  const choices = [
    { persona: 'a', personaLimit: 1 },
    { persona: 'b', personaLimit: 2 },
    { persona: 'c', personaLimit: null },
  ];

  it('keeps the given order (used longest ago first) when nothing runs', () => {
    assert.equal(leastLoaded([], choices).persona, 'a');
  });

  it('picks the member with the fewest slot-holding sessions among those under their cap', () => {
    const sessions = [
      { id: '1', state: 'ready', persona: 'a' },
      { id: '2', state: 'ready', persona: 'c' },
      { id: '3', state: 'disconnected', persona: 'b' },
    ];
    assert.equal(leastLoaded(sessions, choices).persona, 'b');
  });

  it('skips a member at its cap even when it runs fewer than an uncapped one', () => {
    const sessions = [
      { id: '1', state: 'ready', persona: 'a' },
      { id: '2', state: 'ready', persona: 'b' },
      { id: '3', state: 'ready', persona: 'b' },
      { id: '4', state: 'ready', persona: 'c' },
      { id: '5', state: 'ready', persona: 'c' },
      { id: '6', state: 'ready', persona: 'c' },
    ];
    assert.equal(leastLoaded(sessions, choices).persona, 'c');
  });

  it('falls back to the least busy member when every one is at its cap, for a start that will queue', () => {
    const capped = [
      { persona: 'a', personaLimit: 1 },
      { persona: 'b', personaLimit: 1 },
    ];
    const sessions = [
      { id: '1', state: 'ready', persona: 'a' },
      { id: '2', state: 'queued', persona: 'a' },
      { id: '3', state: 'ready', persona: 'b' },
    ];
    assert.equal(leastLoaded(sessions, capped).persona, 'b');
  });
});
