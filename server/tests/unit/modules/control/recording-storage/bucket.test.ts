/**
 * Unit tests for the recording archive's bucket helpers: which ids and files
 * may reach a storage path, frame naming, and whether archiving is configured.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  archived,
  assertShared,
  frameName,
  safe,
  sharedRecordings,
} from '../../../../../src/modules/control/recording-storage/bucket.ts';

describe('recording bucket', () => {
  it('lets only UUID-shaped ids reach a storage path', () => {
    assert.equal(safe('123e4567-e89b-12d3-a456-426614174000'), true);
    assert.equal(safe('../../etc/passwd'), false);
    assert.equal(safe('123e4567-e89b-12d3-a456-42661417400'), false);
  });

  it('names frames with six digits', () => {
    assert.equal(frameName(42), '000042.jpg');
  });

  it('archives numbered frames and the manifest only', () => {
    assert.equal(archived('000001.jpg'), true);
    assert.equal(archived('manifest.json'), true);
    assert.equal(archived('1.jpg'), false);
    assert.equal(archived('notes.txt'), false);
  });

  it('is not configured without Supabase and a bucket, and says so', () => {
    assert.equal(sharedRecordings(), false);
    assert.throws(() => assertShared(), /Recording archive storage unavailable/);
  });
});
