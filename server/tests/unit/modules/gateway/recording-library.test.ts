/**
 * Unit tests for stored recordings: listing, manifests and frames scoped to
 * their owner (someone else's reads as absent), deletion, and retention.
 * Recordings are written into this file's spool directly.
 */
import { describe, it, beforeEach, afterEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdirSync, writeFileSync, existsSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-recording-library-');
const library = await import('../../../../src/modules/gateway/recording-library.ts');
const { DIR, active, frameFile } = await import('../../../../src/modules/gateway/recording-live.ts');
const { control } = await import('../../../../src/modules/control/service.ts');
const { DEFAULT_RECORDING_DAYS, MS_PER_DAY } = await import('../../../../src/modules/gateway/constants.ts');

/** Spools a finished recording for `owner`, started `startedAt`, with one frame. */
function spool(owner: string, startedAt = new Date().toISOString()) {
  const id = randomUUID();
  mkdirSync(join(DIR, id), { recursive: true });
  const manifest = { sessionId: id, owner, startedAt, frameCount: 1, frames: [{ i: 0 }] };
  writeFileSync(join(DIR, id, 'manifest.json'), JSON.stringify(manifest));
  writeFileSync(join(DIR, id, frameFile(0)), 'jpeg-0');
  return id;
}

beforeEach(() => {
  rmSync(DIR, { recursive: true, force: true });
  mkdirSync(DIR, { recursive: true });
});
afterEach(() => {
  active.clear();
  mock.restoreAll();
});

describe('list', () => {
  it("lists the owner's recordings, newest first, without frame lists", async () => {
    const older = spool('aaaa1111', '2026-01-01T00:00:00.000Z');
    const newer = spool('aaaa1111', '2026-02-01T00:00:00.000Z');
    spool('bbbb2222');
    const listed = await library.list('aaaa1111');
    assert.deepEqual(
      listed.map((r) => r.sessionId),
      [newer, older],
    );
    assert.deepEqual([listed[0].frames, listed[0].live], [undefined, false]);
  });

  it('lists everyone’s for an operator', async () => {
    spool('aaaa1111');
    spool('bbbb2222');
    assert.equal((await library.list(null)).length, 2);
  });

  it('includes a live recording that has no manifest yet, for its owner only', async () => {
    const id = randomUUID();
    mkdirSync(join(DIR, id));
    active.set(id, { owner: 'aaaa1111', frames: [{}, {}], bytes: 10, startedAt: Date.now() });
    assert.deepEqual(await library.list('aaaa1111'), [{ sessionId: id, owner: 'aaaa1111', live: true, frameCount: 2 }]);
    assert.deepEqual(await library.list('bbbb2222'), []);
  });

  it('falls back to the archive when the spool is missing', async () => {
    rmSync(DIR, { recursive: true, force: true });
    assert.deepEqual(await library.list('aaaa1111'), []);
  });
});

describe('manifest and frame', () => {
  it("returns the owner's manifest and null for anyone else", async () => {
    const id = spool('aaaa1111');
    assert.equal((await library.manifest(id, 'aaaa1111')).sessionId, id);
    assert.equal(await library.manifest(id, 'bbbb2222'), null);
    assert.equal((await library.manifest(id, null)).owner, 'aaaa1111');
  });

  it('refuses an id that is not a session id', async () => {
    assert.equal(await library.manifest('../etc/passwd', 'aaaa1111'), null);
    assert.equal(await library.frame('../x', 0, 'aaaa1111'), null);
  });

  it('describes a recording still running from memory', async () => {
    const id = randomUUID();
    active.set(id, { owner: 'aaaa1111', frames: [{ i: 0 }], bytes: 9, startedAt: Date.now() });
    const m = await library.manifest(id, 'aaaa1111');
    assert.deepEqual([m.live, m.frameCount, m.bytes], [true, 1, 9]);
    assert.equal(await library.manifest(id, 'bbbb2222'), null);
  });

  it('is null for a recording that does not exist anywhere', async () => {
    assert.equal(await library.manifest(randomUUID(), 'aaaa1111'), null);
  });

  it("serves a frame to the recording's owner only", async () => {
    const id = spool('aaaa1111');
    assert.equal((await library.frame(id, '0', 'aaaa1111')).toString(), 'jpeg-0');
    assert.equal(await library.frame(id, 0, 'bbbb2222'), null);
  });

  it('refuses a frame index out of range or not a whole number', async () => {
    const id = spool('aaaa1111');
    for (const index of [-1, 1.5, 'x', 1e9]) assert.equal(await library.frame(id, index, 'aaaa1111'), null);
  });

  it('is null for a frame the spool does not have', async () => {
    const id = spool('aaaa1111');
    assert.equal(await library.frame(id, 5, 'aaaa1111'), null);
  });
});

describe('remove', () => {
  it("deletes the owner's recording and nobody else's", async () => {
    const id = spool('aaaa1111');
    assert.equal(await library.remove(id, 'bbbb2222'), false);
    assert.equal(existsSync(join(DIR, id)), true);
    assert.equal(await library.remove(id, 'aaaa1111'), true);
    assert.equal(existsSync(join(DIR, id)), false);
  });

  it('says false for an id that is not a session id', async () => {
    assert.equal(await library.remove('nope', 'aaaa1111'), false);
  });
});

describe('maintain', () => {
  it("deletes recordings past their project's retention, else the default", async () => {
    const now = Date.now();
    const expired = spool('aaaa1111', new Date(now - (DEFAULT_RECORDING_DAYS + 1) * MS_PER_DAY).toISOString());
    const kept = spool('aaaa1111', new Date(now - MS_PER_DAY).toISOString());
    const shortLived = spool('cccc3333', new Date(now - 2 * MS_PER_DAY).toISOString());
    mock.method(control().store as any, 'load', async () => [
      [{ body: { legacyOwner: 'cccc3333', settings: { recordingDays: 1 } } }],
      [],
    ]);
    await library.maintain();
    assert.deepEqual(
      [expired, kept, shortLived].map((id) => existsSync(join(DIR, id))),
      [false, true, false],
    );
  });

  it('leaves live recordings alone', async () => {
    const id = spool('aaaa1111', '2000-01-01T00:00:00.000Z');
    active.set(id, {
      owner: 'aaaa1111',
      frames: [],
      bytes: 0,
      startedAt: 0,
      conn: { send: async () => {}, close() {} },
    });
    mock.method(control().store as any, 'load', async () => [[], []]);
    await library.maintain();
    assert.equal(existsSync(join(DIR, id)), true);
  });
});
