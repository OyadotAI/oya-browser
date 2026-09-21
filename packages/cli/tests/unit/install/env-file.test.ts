/**
 * Unit tests for .env handling (src/install/env-file.ts).
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, statSync, writeFileSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { readEnv, renderEnv, writeEnv, SECRET_KEYS } from '../../../src/install/env-file.ts';

const dir = mkdtempSync(join(tmpdir(), 'oya-env-'));

describe('readEnv', () => {
  it('reads KEY=value lines and ignores comments and junk', () => {
    const path = join(dir, 'a.env');
    writeFileSync(path, '# c\nA_1=x=y\n  B=2  \nlower=no\n');
    assert.deepEqual(readEnv(path), { A_1: 'x=y', B: '2' });
  });

  it('is empty when there is no file', () => {
    assert.deepEqual(readEnv(join(dir, 'missing')), {});
  });
});

describe('renderEnv', () => {
  it('groups values under their section, puts unknown ones under Other, then the extra lines', () => {
    const body = renderEnv({ API_KEYS: 'k', PORT: '1', ZZZ: 'z', EMPTY: '' }, ['# extra']);
    assert.equal(
      body,
      [
        '# Written by `oya install`. Re-run it, or edit by hand, both are fine.',
        '',
        '# ── Server ──',
        'PORT=1',
        '',
        '# ── Tenant keys, each is an identity; add more from the dashboard ──',
        'API_KEYS=k',
        '',
        '# ── Other ──',
        'ZZZ=z',
        '',
        '# extra',
        '',
      ].join('\n'),
    );
  });
});

describe('writeEnv', () => {
  it('writes the file owner-only, even over a looser existing one', () => {
    const path = join(dir, 'w.env');
    writeFileSync(path, 'old', { mode: 0o644 });
    writeEnv(path, 'new');
    assert.equal(readFileSync(path, 'utf8'), 'new');
    assert.equal(statSync(path).mode & 0o777, 0o600);
  });
});

describe('SECRET_KEYS', () => {
  it('names credentials that no keyword pattern would catch', () => {
    assert.ok(SECRET_KEYS.has('DATABASE_URL') && SECRET_KEYS.has('OYA_CDP_WS_URL'));
  });
});
