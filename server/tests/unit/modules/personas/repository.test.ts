/**
 * Unit tests for persona storage: the JSON file, the Supabase table (through
 * a fake client), and the fallback that keeps personas when the database
 * cannot be reached.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  FilePersonaRepository,
  SupabasePersonaRepository,
  FallbackPersonaRepository,
} from '../../../../src/modules/personas/index.ts';
import { shape } from '../../../../src/modules/personas/model.ts';

/** A persona as the service holds it. */
const persona = (id: string, extra: object = {}) =>
  shape({
    id,
    owner: 'o',
    name: id,
    seed: 7,
    prefs: { platform: 'Win32' },
    createdAt: '2026-01-01T00:00:00.000Z',
    ...extra,
  });

/** A path in a fresh directory that does not exist yet. */
const freshPath = async () => join(await mkdtemp(join(tmpdir(), 'oya-persona-unit-')), 'nested', 'personas.json');

/** A Supabase client stand-in for the `personas` table, recording what it was sent. */
function fakeDb({ rows = [] as any[], error = null as any } = {}) {
  const calls: any[] = [];
  const table = {
    select: async () => ({ data: error ? null : rows, error }),
    upsert: async (data: any, options: any) => {
      calls.push({ data, options });
      return { error };
    },
  };
  return { db: { from: (name: string) => (calls.push({ from: name }), table) } as any, calls };
}

/** A repository whose every call fails. */
const down = {
  loadAll: async () => Promise.reject(new Error('database unreachable')),
  saveAll: async () => Promise.reject(new Error('database unreachable')),
};

describe('FilePersonaRepository', () => {
  it('reads a missing file as an empty store', async () => {
    assert.deepEqual(await new FilePersonaRepository(await freshPath()).loadAll(), []);
  });

  it('round-trips personas, writing an uncapped one as null and reading it back as Infinity', async () => {
    const repo = new FilePersonaRepository(await freshPath());
    const uncapped = persona('a', { maxConcurrent: Infinity });
    await repo.saveAll([uncapped, persona('b', { maxConcurrent: 3 })]);
    assert.equal(JSON.parse(await readFile(repo.path, 'utf8'))[0].maxConcurrent, null);
    assert.deepEqual(await repo.loadAll(), [uncapped, persona('b', { maxConcurrent: 3 })]);
  });

  it('writes the file owner-only', async () => {
    const repo = new FilePersonaRepository(await freshPath());
    await repo.saveAll([persona('a')]);
    assert.equal((await stat(repo.path)).mode & 0o777, 0o600);
  });

  it('throws on a file it cannot parse, rather than reading it as empty', async () => {
    const repo = new FilePersonaRepository(await freshPath());
    await repo.saveAll([]);
    await writeFile(repo.path, '{not json');
    await assert.rejects(repo.loadAll(), SyntaxError);
  });
});

describe('SupabasePersonaRepository', () => {
  it('reads rows from the personas table as personas', async () => {
    const { db, calls } = fakeDb({
      rows: [
        {
          id: 'a',
          owner: 'o',
          name: 'A',
          seed: 7,
          prefs: null,
          proxy: null,
          max_concurrent: null,
          is_default: true,
          created_at: 'c',
          last_used_at: 'u',
        },
      ],
    });
    const [p] = await new SupabasePersonaRepository(db).loadAll();
    assert.equal(calls[0].from, 'personas');
    assert.deepEqual([p.maxConcurrent, p.isDefault, p.createdAt, p.lastUsedAt], [Infinity, true, 'c', 'u']);
  });

  it('reads no rows as an empty store', async () => {
    const { db } = fakeDb({ rows: null as any });
    assert.deepEqual(await new SupabasePersonaRepository(db).loadAll(), []);
  });

  it('upserts personas as rows keyed on id, uncapped as null', async () => {
    const { db, calls } = fakeDb();
    await new SupabasePersonaRepository(db).saveAll([persona('a', { maxConcurrent: Infinity, isDefault: true })]);
    const { data, options } = calls[1];
    assert.deepEqual(options, { onConflict: 'id' });
    assert.equal(data[0].max_concurrent, null);
    assert.equal(data[0].is_default, true);
    assert.equal(data[0].created_at, '2026-01-01T00:00:00.000Z');
    assert.ok(data[0].updated_at);
  });

  it('throws the database error on read and on write', async () => {
    const { db } = fakeDb({ error: { message: 'relation "personas" does not exist' } });
    const repo = new SupabasePersonaRepository(db);
    await assert.rejects(repo.loadAll(), /relation "personas" does not exist/);
    await assert.rejects(repo.saveAll([persona('a')]), /relation "personas" does not exist/);
  });
});

describe('FallbackPersonaRepository', () => {
  beforeEach(() => mock.method(console, 'error', () => {}));
  afterEach(() => mock.restoreAll());

  it('uses the database while it works, and leaves the file alone', async () => {
    const { db } = fakeDb({ rows: [] });
    const file = new FilePersonaRepository(await freshPath());
    const repo = new FallbackPersonaRepository(new SupabasePersonaRepository(db), file);
    await repo.saveAll([persona('a')]);
    assert.deepEqual(await file.loadAll(), []);
  });

  it('writes to the file when the database write fails', async () => {
    const file = new FilePersonaRepository(await freshPath());
    const repo = new FallbackPersonaRepository(down, file);
    await repo.saveAll([persona('a')]);
    assert.deepEqual(await file.loadAll(), [persona('a')]);
  });

  it('reads from the file when the database read fails', async () => {
    const file = new FilePersonaRepository(await freshPath());
    await file.saveAll([persona('a')]);
    assert.deepEqual(await new FallbackPersonaRepository(down, file).loadAll(), [persona('a')]);
  });

  it('logs every failed read, naming the file standing in', async () => {
    const file = new FilePersonaRepository(await freshPath());
    const repo = new FallbackPersonaRepository(down, file);
    await repo.loadAll();
    await repo.loadAll();
    const logged = (console.error as any).mock.calls.map((c) => c.arguments[0]);
    assert.equal(logged.length, 2);
    assert.match(
      logged[0],
      new RegExp(`database read failed \\(database unreachable\\) — falling back to ${file.path}`),
    );
  });

  it('logs only the first failed write, so a down database does not flood the log', async () => {
    const repo = new FallbackPersonaRepository(down, new FilePersonaRepository(await freshPath()));
    await repo.saveAll([persona('a')]);
    await repo.saveAll([persona('a')]);
    assert.equal((console.error as any).mock.callCount(), 1);
    assert.match((console.error as any).mock.calls[0].arguments[0], /database write failed/);
  });
});
