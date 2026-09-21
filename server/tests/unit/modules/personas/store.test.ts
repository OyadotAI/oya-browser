/**
 * Unit tests for PersonaStore: the in-memory persona table, its dirty flag,
 * and the autosave that retries a failed write on the next tick.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { PersonaStore } from '../../../../src/modules/personas/store.ts';
import { shape } from '../../../../src/modules/personas/model.ts';
import { AUTOSAVE_MS } from '../../../../src/modules/personas/constants.ts';
import { MemoryPersonaRepository } from '../../support/personas.ts';

/** A minimal persona with this id. */
const persona = (id: string) => shape({ id, owner: 'o', name: id, seed: 1, createdAt: 't' });

describe('PersonaStore', () => {
  beforeEach(() => {
    mock.method(console, 'log', () => {});
    mock.method(console, 'error', () => {});
  });
  afterEach(() => {
    mock.timers.reset();
    mock.restoreAll();
  });

  it('holds personas by id', () => {
    const store = new PersonaStore(new MemoryPersonaRepository());
    const p = persona('a');
    store.put(p);
    assert.equal(store.get('a'), p);
    assert.deepEqual(store.all(), [p]);
    store.delete('a');
    assert.equal(store.get('a'), undefined);
  });

  it('saves only when something changed', async () => {
    const repo = new MemoryPersonaRepository();
    const store = new PersonaStore(repo);
    await store.flush();
    assert.equal(repo.saves, 0);
    store.put(persona('a'));
    await store.flush();
    await store.flush();
    assert.equal(repo.saves, 1);
  });

  it('saves after a persona changed in place', async () => {
    const repo = new MemoryPersonaRepository();
    const store = new PersonaStore(repo);
    store.touch();
    await store.flush();
    assert.equal(repo.saves, 1);
  });

  it('saves after a delete', async () => {
    const repo = new MemoryPersonaRepository();
    const store = new PersonaStore(repo);
    store.put(persona('a'));
    await store.flush();
    store.delete('a');
    await store.flush();
    assert.equal(repo.stored, '[]');
  });

  it('keeps changes marked after a failed save, and writes them on the next try', async () => {
    const repo = new MemoryPersonaRepository();
    const store = new PersonaStore(repo);
    store.put(persona('a'));
    repo.failWith = new Error('database down');
    await store.flush();
    assert.equal(repo.saves, 0);
    repo.failWith = null;
    await store.flush();
    assert.equal(repo.saves, 1);
  });

  it('loads saved personas on restore', async () => {
    const repo = new MemoryPersonaRepository();
    await repo.saveAll([persona('a'), persona('b')]);
    const store = new PersonaStore(repo);
    await store.restore();
    assert.deepEqual(
      store.all().map((p) => p.id),
      ['a', 'b'],
    );
  });

  it('logs a failed restore and starts empty', async () => {
    const repo = new MemoryPersonaRepository();
    repo.failWith = new Error('unreachable');
    const store = new PersonaStore(repo);
    await store.restore();
    assert.deepEqual(store.all(), []);
    assert.equal((console.error as any).mock.callCount(), 1);
  });

  it('writes changes every autosave interval', async () => {
    mock.timers.enable({ apis: ['setInterval'] });
    const repo = new MemoryPersonaRepository();
    const store = new PersonaStore(repo);
    store.startAutosave();
    store.put(persona('a'));
    mock.timers.tick(AUTOSAVE_MS);
    await new Promise(setImmediate);
    assert.equal(repo.saves, 1);
    await store.drain();
  });

  it('stops autosaving on drain, after writing what is pending', async () => {
    mock.timers.enable({ apis: ['setInterval'] });
    const repo = new MemoryPersonaRepository();
    const store = new PersonaStore(repo);
    store.startAutosave(1000);
    store.put(persona('a'));
    await store.drain();
    assert.equal(repo.saves, 1);
    store.touch();
    mock.timers.tick(5000);
    await new Promise(setImmediate);
    assert.equal(repo.saves, 1);
  });

  it('forgets everything on clear, without saving', async () => {
    const repo = new MemoryPersonaRepository();
    const store = new PersonaStore(repo);
    store.put(persona('a'));
    store.clear();
    await store.flush();
    assert.deepEqual([store.all(), repo.saves], [[], 0]);
  });
});
