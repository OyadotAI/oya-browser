/**
 * Unit tests for the download chip's words: a file's extension is never cut,
 * and each ended phase comes only from how main says the download ended.
 */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { phaseOf, splitName } from '../../../../../../src/renderer/features/toolbar/model/download-chip.ts';

/** A listed download in `state`. */
const entry = (state: string) => ({ id: 'd', url: '', title: 'a.pdf', time: 1, state, received: 0, total: 0 }) as any;

it('splits a name so its extension is never cut', () => {
  assert.deepEqual(splitName('Resume-Jane.pdf'), ['Resume-Jane', '.pdf']);
  assert.deepEqual(splitName('archive.tar.gz'), ['archive.tar', '.gz']);
  assert.deepEqual(splitName('.bashrc'), ['.bashrc', '']);
  assert.deepEqual(splitName('README'), ['README', '']);
});

it('reads the phase from how the download ended, and a missing file over all', () => {
  assert.equal(phaseOf(entry('progressing'), false), 'running');
  assert.equal(phaseOf(entry('completed'), false), 'saved');
  assert.equal(phaseOf(entry('cancelled'), false), 'cancelled');
  assert.equal(phaseOf(entry('interrupted'), false), 'failed');
  assert.equal(phaseOf(entry('completed'), true), 'gone');
});
