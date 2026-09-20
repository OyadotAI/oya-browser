/**
 * Unit tests for preflight (src/install/preflight.ts) on a deployment that
 * needs no Docker, kubectl or psql, so only Node, the port and .env are checked.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { preflight } from '../../../src/install/preflight.ts';
import type { Answers } from '../../../src/install/types.ts';

const PLAN = {
  version: 1,
  host: 'local',
  database: 'sqlite',
  fleet: 'cdp',
  workers: 0,
  llm: { provider: 'skip', baseUrl: '', model: '' },
  publicUrl: 'http://localhost:1',
  optional: { captcha: '', recordingBucket: '', metrics: false },
} as Answers;

describe('preflight', () => {
  it('checks Node and the port, and reports an existing .env with its mode', async () => {
    const root = mkdtempSync(join(tmpdir(), 'oya-pre-'));
    writeFileSync(join(root, '.env'), 'A=1', { mode: 0o640 });
    const checks = await preflight(root, PLAN);
    assert.deepEqual(
      checks.map((c) => c.label),
      ['node ≥ 22.13 (node:sqlite)', 'port 1 free', '.env exists — keeping its OYA_PROFILE_SECRET and API keys'],
    );
    assert.equal(checks[0].ok, true);
    assert.equal(checks[2].detail, 'mode 640');
  });
});
