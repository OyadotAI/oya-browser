/**
 * Unit tests for starting the Next.js frontend: nothing to start in an
 * API-only deployment, and a clear refusal for a bad mode or a missing build.
 * Spawning and proxying to a real Next.js process is left to the stack tests.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { startFrontend } from '../../../src/app/frontend.ts';
import { UI_DIR } from '../../../src/platform/paths.ts';

describe('startFrontend', () => {
  const saved = process.env.OYA_UI_MODE;
  afterEach(() => (saved === undefined ? delete process.env.OYA_UI_MODE : (process.env.OYA_UI_MODE = saved)));

  it('starts nothing when OYA_UI_MODE is unset', () => {
    delete process.env.OYA_UI_MODE;
    assert.equal(startFrontend(), null);
  });

  it('refuses a mode other than development or production', () => {
    process.env.OYA_UI_MODE = 'staging';
    assert.throws(() => startFrontend(), /OYA_UI_MODE must be development or production/);
  });

  it(
    'refuses production when there is no standalone build',
    {
      skip: existsSync(join(UI_DIR, '.next/standalone/server.js')),
    },
    () => {
      process.env.OYA_UI_MODE = 'production';
      assert.throws(() => startFrontend(), /Next.js is missing/);
    },
  );
});
