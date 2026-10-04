/**
 * Unit tests for the HTTP app's environment-tuned values: which origins CORS
 * lets call the API, read from OYA_CORS_ORIGINS once at load.
 */
import { describe, it, afterEach } from 'node:test';
import assert from 'node:assert/strict';

/** The module loaded afresh, so its environment is read again. */
const load = async () => import(`../../../src/app/constants.ts?fresh=${Math.random()}`);

describe('CORS_ORIGIN', () => {
  afterEach(() => delete process.env.OYA_CORS_ORIGINS);

  it('lets any origin call the API when OYA_CORS_ORIGINS is unset', async () => {
    delete process.env.OYA_CORS_ORIGINS;
    assert.equal((await load()).CORS_ORIGIN, '*');
  });

  it('pins the API to the origins OYA_CORS_ORIGINS lists, trimmed', async () => {
    process.env.OYA_CORS_ORIGINS = 'https://oyabrowser.com, http://localhost:3000';
    assert.deepEqual((await load()).CORS_ORIGIN, ['https://oyabrowser.com', 'http://localhost:3000']);
  });
});
