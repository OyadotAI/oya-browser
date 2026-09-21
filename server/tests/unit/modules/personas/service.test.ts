/**
 * Unit tests for PersonaService: creating, resolving, updating, cloning and
 * deleting personas, per-key ownership, concurrency slots, persistence, and
 * the rule that a persona's fingerprint never changes.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { PersonaService } from '../../../../src/modules/personas/index.ts';
import { defaultPersonaSeed } from '../../../../src/modules/personas/fingerprint.ts';
import { generateProfile } from '../../../../src/modules/personas/profile.ts';
import {
  DEFAULT_MAX_CONCURRENT,
  DEFAULT_PERSONA_MAX_CONCURRENT,
  MAX_NAME_CHARS,
} from '../../../../src/modules/personas/constants.ts';
import { Status } from '../../../../src/platform/http-status.ts';
import { personaService, personaDeps, MemoryPersonaRepository } from '../../support/personas.ts';

beforeEach(() => mock.method(console, 'log', () => {}));
afterEach(() => mock.restoreAll());

describe('PersonaService default persona', () => {
  it("creates a key's default persona on first use, with the seed of its pre-persona fingerprint", () => {
    const { service } = personaService();
    const p = service.defaultFor('key-a');
    assert.equal(p.id, defaultPersonaSeed('key-a').id);
    assert.equal(p.seed, defaultPersonaSeed('key-a').seed);
    assert.deepEqual([p.name, p.isDefault, p.owner], ['Default', true, 'owner:key-a']);
  });

  it('takes the platform of the first browser that connects, once: a Mac desktop is not given a Linux device', () => {
    const { service } = personaService();
    const created = service.resolve('key-mac', null, { platform: 'MacIntel' });
    assert.equal(created.prefs?.platform, 'MacIntel');
    assert.equal(service.fingerprintFor(created).navigator.platform, 'MacIntel');
    const later = service.resolve('key-mac', null, { platform: 'Win32' });
    assert.equal(later.prefs?.platform, 'MacIntel', 'the device is chosen at creation and never changes after');
  });

  it('ignores a platform it does not offer, and stays as it was for callers that give none', () => {
    const { service } = personaService();
    assert.equal(service.resolve('key-odd', null, { platform: 'BeOS' }).prefs ?? null, null);
    assert.equal(service.defaultFor('key-none').prefs ?? null, null);
  });

  it('returns the same default persona on later calls', () => {
    const { service } = personaService();
    assert.equal(service.defaultFor('key-a'), service.defaultFor('key-a'));
  });

  it('is uncapped unless the deployment says otherwise', () => {
    const { service } = personaService();
    assert.equal(service.defaultFor('key-a').maxConcurrent, DEFAULT_PERSONA_MAX_CONCURRENT);
  });
});

describe('PersonaService.create', () => {
  it('makes a named persona owned by the key, with a fresh seed and the default cap', () => {
    const { service } = personaService();
    const p = service.create('key-a', { name: 'Work' });
    assert.match(p.id, /^p-[0-9a-f]{16}$/);
    assert.deepEqual(
      [p.name, p.owner, p.isDefault, p.maxConcurrent],
      ['Work', 'owner:key-a', false, DEFAULT_MAX_CONCURRENT],
    );
    assert.notEqual(p.seed, service.create('key-a').seed);
  });

  it('names a persona after its id when no name is given, and cuts long names', () => {
    const { service } = personaService();
    const p = service.create('key-a');
    assert.equal(p.name, p.id);
    assert.equal(service.create('key-a', { name: 'n'.repeat(500) }).name.length, MAX_NAME_CHARS);
  });

  it('keeps a positive cap and treats anything else as the default', () => {
    const { service } = personaService();
    assert.equal(service.create('k', { maxConcurrent: 4 }).maxConcurrent, 4);
    assert.equal(service.create('k', { maxConcurrent: 0 }).maxConcurrent, DEFAULT_MAX_CONCURRENT);
    assert.equal(service.create('k', { maxConcurrent: -3 }).maxConcurrent, DEFAULT_MAX_CONCURRENT);
  });

  it('keeps only the device choices of its prefs, marked as checked under the current rule', () => {
    const { service } = personaService();
    const p = service.create('k', { prefs: { platform: 'MacIntel', seed: 5 } as any });
    assert.deepEqual(p.prefs, { platform: 'MacIntel', checked: true });
  });

  it('marks even a persona without prefs as checked, so the current rule applies to it', () => {
    const { service } = personaService();
    assert.deepEqual(service.create('k').prefs, { checked: true });
  });

  it('keeps its own proxy', () => {
    const { service } = personaService();
    const proxy = { host: 'proxy.example', port: 8080, geo: 'US' };
    assert.deepEqual(service.create('k', { proxy }).proxy, proxy);
    assert.equal(service.create('k').proxy, null);
  });
});

describe('PersonaService ownership', () => {
  it('finds a persona only for the key that owns it', () => {
    const { service } = personaService();
    const p = service.create('key-a');
    assert.equal(service.get('key-a', p.id), p);
    assert.equal(service.get('key-b', p.id), null);
    assert.equal(service.get('key-a', 'p-missing'), null);
  });

  it("lists a key's own personas, the default included, and nobody else's", () => {
    const { service } = personaService();
    const mine = service.create('key-a');
    service.create('key-b');
    const listed = service.list('key-a');
    assert.deepEqual(listed.map((p) => p.id).sort(), [mine.id, defaultPersonaSeed('key-a').id].sort());
  });

  it("refuses to update, clone or remove another key's persona", () => {
    const { service } = personaService();
    const p = service.create('key-a');
    assert.equal(service.update('key-b', p.id, { name: 'stolen' }), null);
    assert.equal(service.clone('key-b', p.id), null);
    assert.equal(service.remove('key-b', p.id), false);
    assert.equal(p.name, p.id);
  });
});

describe('PersonaService.update', () => {
  it('changes the name, cap and proxy', () => {
    const { service } = personaService();
    const p = service.create('k');
    service.update('k', p.id, { name: 'Renamed', maxConcurrent: 3, proxy: { host: 'h', port: 1 } });
    assert.deepEqual([p.name, p.maxConcurrent, p.proxy], ['Renamed', 3, { host: 'h', port: 1 }]);
  });

  it('never changes the seed or prefs, whatever the caller sends', () => {
    const { service } = personaService();
    const p = service.create('k', { prefs: { platform: 'Win32' } });
    const before = { seed: p.seed, prefs: structuredClone(p.prefs) };
    service.update('k', p.id, { seed: 1, prefs: { platform: 'MacIntel' } } as any);
    assert.deepEqual({ seed: p.seed, prefs: p.prefs }, before);
  });

  it('leaves fields that were not given alone', () => {
    const { service } = personaService();
    const p = service.create('k', { name: 'Keep', maxConcurrent: 4 });
    service.update('k', p.id, {});
    assert.deepEqual([p.name, p.maxConcurrent], ['Keep', 4]);
  });

  it('names a persona after its id when the new name is empty', () => {
    const { service } = personaService();
    const p = service.create('k', { name: 'Old' });
    service.update('k', p.id, { name: '' });
    assert.equal(p.name, p.id);
  });

  it('lifts the cap for null or Infinity, and falls back to the default for anything not positive', () => {
    const { service } = personaService();
    const p = service.create('k');
    service.update('k', p.id, { maxConcurrent: null });
    assert.equal(p.maxConcurrent, Infinity);
    service.update('k', p.id, { maxConcurrent: Infinity });
    assert.equal(p.maxConcurrent, Infinity);
    service.update('k', p.id, { maxConcurrent: 0 });
    assert.equal(p.maxConcurrent, DEFAULT_MAX_CONCURRENT);
  });

  it("gives the default persona the deployment's cap for anything not positive", () => {
    const { service } = personaService();
    const d = service.defaultFor('k');
    service.update('k', d.id, { maxConcurrent: 7 });
    assert.equal(d.maxConcurrent, 7);
    service.update('k', d.id, { maxConcurrent: 0 });
    assert.equal(d.maxConcurrent, DEFAULT_PERSONA_MAX_CONCURRENT);
    service.update('k', d.id, { maxConcurrent: null });
    assert.equal(d.maxConcurrent, Infinity);
  });

  it('drops the proxy for null or anything that is not an object', () => {
    const { service } = personaService();
    const p = service.create('k', { proxy: { host: 'h' } });
    service.update('k', p.id, { proxy: 'h:1' as any });
    assert.equal(p.proxy, null);
  });

  it('returns null for a persona that does not exist', () => {
    const { service } = personaService();
    assert.equal(service.update('k', 'p-missing', { name: 'x' }), null);
  });
});

describe('PersonaService.clone', () => {
  it('copies the device choices, proxy and cap onto a fresh seed', () => {
    const { service } = personaService();
    const src = service.create('k', {
      name: 'Src',
      prefs: { platform: 'MacIntel' },
      proxy: { host: 'h' },
      maxConcurrent: 3,
    });
    const copy = service.clone('k', src.id);
    assert.notEqual(copy.id, src.id);
    assert.notEqual(copy.seed, src.seed);
    assert.deepEqual([copy.name, copy.prefs, copy.proxy, copy.maxConcurrent], ['Src (copy)', src.prefs, src.proxy, 3]);
  });

  it('takes the name it is given', () => {
    const { service } = personaService();
    const src = service.create('k');
    assert.equal(service.clone('k', src.id, { name: 'Twin' }).name, 'Twin');
  });

  it('keeps an older persona’s prefs under the rule they were made with', () => {
    const { service } = personaService({ repository: legacyRepository() });
    return service.restore().then(() => {
      const copy = service.clone('k', 'p-legacy00000000');
      assert.deepEqual(copy.prefs, { platform: 'Win32', timezone: 'Europe/Berlin' });
    });
  });

  it('gives a copy of an uncapped persona the default cap', () => {
    const { service } = personaService();
    const src = service.create('k');
    service.update('k', src.id, { maxConcurrent: null });
    assert.equal(service.clone('k', src.id).maxConcurrent, DEFAULT_MAX_CONCURRENT);
  });
});

/** A repository holding one persona made before prefs were validated. */
function legacyRepository() {
  const repo = new MemoryPersonaRepository();
  repo.stored = JSON.stringify([
    {
      id: 'p-legacy00000000',
      owner: 'owner:k',
      name: 'Legacy',
      seed: 42,
      prefs: { platform: 'Win32', timezone: 'Europe/Berlin' },
      maxConcurrent: 2,
      createdAt: '2025-01-01T00:00:00.000Z',
    },
  ]);
  return repo;
}

describe('PersonaService.remove', () => {
  it('deletes the persona and everything stored for it', () => {
    const { service, deps } = personaService();
    const p = service.create('k');
    assert.equal(service.remove('k', p.id), true);
    assert.equal(service.get('k', p.id), null);
    assert.deepEqual(deps.logins.clear.mock.calls[0].arguments, [p.id]);
    assert.deepEqual(deps.mfa.clearAll.mock.calls[0].arguments, [p.id]);
    assert.deepEqual(deps.credentials.clearAll.mock.calls[0].arguments, [p.id]);
  });

  it('refuses the default persona with a 400', () => {
    const { service } = personaService();
    const d = service.defaultFor('k');
    assert.throws(() => service.remove('k', d.id), {
      status: Status.BAD_REQUEST,
      message: 'The default persona cannot be deleted',
    });
  });

  it('refuses a persona in use with a 409', () => {
    const { service } = personaService();
    const p = service.create('k');
    service.acquire(p, 'b-1');
    assert.throws(() => service.remove('k', p.id), { status: Status.CONFLICT, message: 'Persona is in use' });
  });

  it('returns false for a persona that does not exist', () => {
    const { service } = personaService();
    assert.equal(service.remove('k', 'p-missing'), false);
  });
});

describe('PersonaService.resolve', () => {
  it('resolves nothing, or "default", to the default persona', () => {
    const { service } = personaService();
    const d = service.defaultFor('k');
    assert.equal(service.resolve('k'), d);
    assert.equal(service.resolve('k', null), d);
    assert.equal(service.resolve('k', 'default'), d);
  });

  it('resolves an owned id to that persona', () => {
    const { service } = personaService();
    const p = service.create('k');
    assert.equal(service.resolve('k', p.id), p);
  });

  it("refuses an unknown or another key's id with a 404", () => {
    const { service } = personaService();
    const p = service.create('key-a');
    assert.throws(() => service.resolve('key-b', p.id), {
      status: Status.NOT_FOUND,
      message: `No such persona: ${p.id}`,
    });
    assert.throws(() => service.resolve('key-a', 'p-missing'), { status: Status.NOT_FOUND });
  });

  it('resolves "auto" to the least recently used persona, never-used ones first', () => {
    const { service } = personaService();
    const d = service.defaultFor('k');
    const used = service.create('k');
    const fresh = service.create('k');
    d.lastUsedAt = '2025-01-02T00:00:00.000Z';
    used.lastUsedAt = '2025-01-01T00:00:00.000Z';
    assert.equal(service.resolve('k', 'auto'), fresh);
    fresh.lastUsedAt = '2025-01-03T00:00:00.000Z';
    assert.equal(service.resolve('k', 'auto'), used);
  });

  it('skips personas at their cap when resolving "auto"', () => {
    const { service } = personaService();
    const d = service.defaultFor('k');
    service.update('k', d.id, { maxConcurrent: 1 });
    const p = service.create('k', { maxConcurrent: 1 });
    service.acquire(p, 'b-1');
    assert.equal(service.resolve('k', 'auto'), d);
  });

  it('answers 429 for "auto" when every persona is at its cap', () => {
    const { service } = personaService();
    const d = service.defaultFor('k');
    service.update('k', d.id, { maxConcurrent: 1 });
    service.acquire(d, 'b-1');
    assert.throws(() => service.resolve('k', 'auto'), {
      status: Status.TOO_MANY_REQUESTS,
      message: 'Every persona is at its concurrency cap',
    });
  });
});

describe('PersonaService concurrency slots', () => {
  it('counts the browsers running as a persona and notes when it was last used', () => {
    mock.timers.enable({ apis: ['Date'], now: Date.parse('2026-01-01T00:00:00.000Z') });
    const { service } = personaService();
    const p = service.create('k');
    service.acquire(p, 'b-1');
    assert.equal(service.activeCount(p.id), 1);
    assert.equal(p.lastUsedAt, '2026-01-01T00:00:00.000Z');
    mock.timers.reset();
  });

  it('refuses a start past the cap with a 429 and counts the refusal', () => {
    const { service, deps } = personaService();
    const p = service.create('k', { name: 'Phone', maxConcurrent: 1 });
    service.acquire(p, 'b-1');
    assert.throws(() => service.acquire(p, 'b-2'), {
      status: Status.TOO_MANY_REQUESTS,
      message: 'Persona "Phone" already has 1 of 1 browsers running',
    });
    assert.equal(deps.metrics.personaCapped.inc.mock.callCount(), 1);
  });

  it('lets the same browser take its slot again without counting twice', () => {
    const { service } = personaService();
    const p = service.create('k', { maxConcurrent: 1 });
    service.acquire(p, 'b-1');
    service.acquire(p, 'b-1');
    assert.equal(service.activeCount(p.id), 1);
  });

  it('frees a slot on release, so another browser can start', () => {
    const { service } = personaService();
    const p = service.create('k', { maxConcurrent: 1 });
    service.acquire(p, 'b-1');
    service.release(p, 'b-1');
    assert.equal(service.activeCount(p.id), 0);
    assert.doesNotThrow(() => service.acquire(p, 'b-2'));
  });

  it('ignores a release without a persona', () => {
    const { service } = personaService();
    assert.doesNotThrow(() => service.release(null, 'b-1'));
  });

  it('reports the running total to metrics', () => {
    const { service, deps } = personaService();
    const a = service.create('k');
    const b = service.create('k');
    service.acquire(a, 'b-1');
    service.acquire(b, 'b-2');
    assert.deepEqual(deps.metrics.personasActive.set.mock.calls.at(-1).arguments, [{}, 2]);
    service.release(a, 'b-1');
    assert.deepEqual(deps.metrics.personasActive.set.mock.calls.at(-1).arguments, [{}, 1]);
  });

  it('forgets the slots of a removed persona', () => {
    const { service } = personaService();
    const p = service.create('k');
    service.acquire(p, 'b-1');
    service.release(p, 'b-1');
    service.remove('k', p.id);
    assert.equal(service.activeCount(p.id), 0);
  });
});

describe('PersonaService fingerprints', () => {
  it('gives a persona the same fingerprint on every call', () => {
    const { service } = personaService();
    const p = service.create('k', { prefs: { platform: 'MacIntel', timezone: 'Europe/Paris' } });
    const first = service.fingerprintFor(p);
    assert.equal(service.fingerprintFor(p), first);
    assert.deepEqual(service.fingerprintFor({ ...p }), first);
  });

  it('gives a fresh service built from the stored record the same fingerprint', async () => {
    const repository = new MemoryPersonaRepository();
    const writer = new PersonaService(personaDeps({ repository }));
    const p = writer.create('k', {
      prefs: { platform: 'Linux x86_64', locale: 'ja-JP' },
      proxy: { host: 'h', port: 1 },
    });
    const before = structuredClone(writer.fingerprintFor(p));
    await writer.drain();

    const reader = new PersonaService(personaDeps({ repository }));
    await reader.restore();
    const restored = reader.get('k', p.id);
    assert.deepEqual(reader.fingerprintFor(restored), before);
    assert.deepEqual(
      generateProfile({ id: restored.id, seed: restored.seed, prefs: restored.prefs, proxy: restored.proxy }),
      before,
      'regenerated from the stored record alone, not from a cache',
    );
  });

  it('never moves a persona’s fingerprint when its name, cap or proxy change', () => {
    const { service } = personaService();
    const p = service.create('k');
    const before = structuredClone(service.fingerprintFor(p));
    service.update('k', p.id, { name: 'Renamed', maxConcurrent: 5 });
    assert.deepEqual(generateProfile({ id: p.id, seed: p.seed, prefs: p.prefs, proxy: p.proxy }), before);
  });

  it("keeps the key's default persona on its pre-persona fingerprint", () => {
    const { service } = personaService();
    const d = service.defaultFor('key-legacy');
    assert.deepEqual(
      service.fingerprintFor(d),
      generateProfile({ ...defaultPersonaSeed('key-legacy'), prefs: null, proxy: null }),
    );
  });

  it('gives a clone a different device of the same kind', () => {
    const { service } = personaService();
    const src = service.create('k', { prefs: { platform: 'MacIntel', timezone: 'Asia/Tokyo' } });
    const copy = service.clone('k', src.id);
    const [a, b] = [service.fingerprintFor(src), service.fingerprintFor(copy)];
    assert.deepEqual([b.navigator.platform, b.timezone], [a.navigator.platform, a.timezone]);
    assert.notEqual(b.canvas.noiseSeed, a.canvas.noiseSeed);
  });

  it('previews the device prefs would give, without keeping a persona', () => {
    const { service } = personaService();
    const fp = service.preview({ platform: 'MacIntel', timezone: 'Europe/Berlin' });
    assert.deepEqual([fp.navigator.platform, fp.timezone], ['MacIntel', 'Europe/Berlin']);
    assert.match(fp.id, /^preview-p-/);
    assert.deepEqual(
      service.list('k').map((p) => p.isDefault),
      [true],
    );
  });
});

describe('PersonaService persistence', () => {
  it('writes changes out on drain, and not when nothing changed', async () => {
    const { service, deps } = personaService();
    const repo = deps.repository as MemoryPersonaRepository;
    await service.drain();
    assert.equal(repo.saves, 0);
    service.create('k');
    await service.drain();
    assert.equal(repo.saves, 1);
    await service.drain();
    assert.equal(repo.saves, 1);
  });

  it('round-trips an uncapped persona and its ownership through storage', async () => {
    const repository = new MemoryPersonaRepository();
    const writer = new PersonaService(personaDeps({ repository }));
    const made = writer.create('k', { name: 'Uncapped' });
    writer.update('k', made.id, { maxConcurrent: null });
    await writer.drain();

    const reader = new PersonaService(personaDeps({ repository }));
    await reader.restore();
    assert.deepEqual(reader.get('k', made.id), made);
    assert.equal(reader.get('k', made.id).maxConcurrent, Infinity);
    assert.equal(reader.get('other', made.id), null);
  });

  it('saves on the autosave timer', async () => {
    mock.timers.enable({ apis: ['setInterval'] });
    const { service, deps } = personaService();
    service.startAutosave(1000);
    service.create('k');
    mock.timers.tick(1000);
    await new Promise(setImmediate);
    assert.equal((deps.repository as MemoryPersonaRepository).saves, 1);
    await service.drain();
    mock.timers.reset();
  });

  it('logs a failed restore rather than throwing', async () => {
    const error = mock.method(console, 'error', () => {});
    const repository = new MemoryPersonaRepository();
    repository.failWith = new Error('disk gone');
    const { service } = personaService({ repository });
    await service.restore();
    assert.match(String(error.mock.calls[0].arguments.join(' ')), /restore failed: disk gone/);
  });

  it('forgets every persona and slot on reset', () => {
    const { service } = personaService();
    const p = service.create('k');
    service.acquire(p, 'b-1');
    service.reset();
    assert.equal(service.get('k', p.id), null);
    assert.equal(service.activeCount(p.id), 0);
  });
});

describe('PersonaService.mirror', () => {
  const device = {
    navigator: { platform: 'MacIntel' },
    screen: { width: 1440, height: 900 },
    chromeVersion: '128.0.0.0',
  };

  it('creates a persona owned by the key that runs as the captured device', () => {
    const { service } = personaService();
    const p = service.mirror('key-a', { source: 'chrome', profile: 'Default', name: 'Chrome — You', device });
    assert.match(p.id, /^m-[0-9a-f]{16}$/);
    assert.deepEqual([p.owner, p.name, p.device.chromeVersion], ['owner:key-a', 'Chrome — You', '128.0.0.0']);
  });

  it('is idempotent per source and profile, and keeps the first captured device', () => {
    const { service } = personaService();
    const a = service.mirror('key-a', { source: 'chrome', profile: 'Default', device });
    const b = service.mirror('key-a', {
      source: 'chrome',
      profile: 'Default',
      device: { ...device, chromeVersion: '999' },
    });
    assert.equal(a, b);
    assert.equal(b.device.chromeVersion, '128.0.0.0');
  });

  it('gives different profiles and different owners different personas', () => {
    const { service } = personaService();
    const a = service.mirror('key-a', { source: 'chrome', profile: 'Default', device });
    const b = service.mirror('key-a', { source: 'chrome', profile: 'Profile 1', device });
    const c = service.mirror('key-b', { source: 'chrome', profile: 'Default', device });
    assert.equal(new Set([a.id, b.id, c.id]).size, 3);
  });
});

describe('PersonaService.fingerprintFor', () => {
  it('returns the captured device for a mirrored persona, under the persona id', () => {
    const { service } = personaService();
    const device = {
      navigator: { platform: 'MacIntel' },
      screen: { width: 1440 },
      webgl: { unmaskedRenderer: 'Apple M2' },
    };
    const p = service.mirror('key-a', { source: 'chrome', profile: 'Default', device });
    const fp = service.fingerprintFor(p);
    assert.deepEqual([fp.id, fp.webgl.unmaskedRenderer], [p.id, 'Apple M2']);
  });

  it('generates a profile for a persona that mirrors nothing', () => {
    const { service } = personaService();
    const p = service.defaultFor('key-a');
    assert.deepEqual(
      service.fingerprintFor(p),
      generateProfile({ id: p.id, seed: p.seed, prefs: p.prefs, proxy: p.proxy }),
    );
  });
});
