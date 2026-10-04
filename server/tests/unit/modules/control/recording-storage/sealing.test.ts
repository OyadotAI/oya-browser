/**
 * Unit tests for recordings at rest: frames and manifests round-trip sealed,
 * a sealed one opens only for its own recording and frame, and recordings
 * written before sealing (plain JPEG and JSON) still read.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  openFrame,
  openManifest,
  sealFrame,
  sealManifest,
} from '../../../../../src/modules/control/recording-storage/sealing.ts';

const ID = '123e4567-e89b-12d3-a456-426614174000';
const OTHER = '123e4567-e89b-12d3-a456-426614174001';
const JPEG = Buffer.from([0xff, 0xd8, 0xff, 0xe0, 1, 2, 3]);

describe('recording sealing', () => {
  it('round-trips a frame', () => {
    assert.deepEqual(openFrame(ID, 3, sealFrame(ID, 3, JPEG)), JPEG);
  });

  it('never stores the frame bytes in the clear', () => {
    assert.equal(sealFrame(ID, 0, JPEG).includes(JPEG), false);
  });

  it('refuses a frame opened for another recording or position', () => {
    const sealed = sealFrame(ID, 3, JPEG);
    assert.throws(() => openFrame(OTHER, 3, sealed));
    assert.throws(() => openFrame(ID, 4, sealed));
  });

  it('round-trips a manifest, owner included', () => {
    const manifest = { sessionId: ID, owner: 'aaaa1111', frames: [{ i: 0 }] };
    assert.deepEqual(openManifest(ID, sealManifest(manifest)), manifest);
  });

  it('refuses a manifest opened for another recording', () => {
    assert.throws(() => openManifest(OTHER, sealManifest({ sessionId: ID, owner: 'aaaa1111' })));
  });

  it('reads a frame and a manifest written before sealing as they are', () => {
    assert.deepEqual(openFrame(ID, 0, JPEG), JPEG);
    assert.deepEqual(openManifest(ID, Buffer.from(JSON.stringify({ sessionId: ID }))), { sessionId: ID });
  });
});
