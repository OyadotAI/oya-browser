/**
 * Unit tests for src/renderer/features/studio/model/save-outcome.ts: the save card
 * says "Renamed" only when the rename happened, and each failure says what was
 * and was not saved.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { savedMessage, saveFailure } from '../../../../../../src/renderer/features/studio/model/save-outcome.ts';

describe('savedMessage', () => {
  it('says the playbook was renamed only when the rename was confirmed', () => {
    assert.equal(savedMessage({ renamed: { from: 'old' } }, 'new'), 'Renamed “old” to “new” and saved it to Oya.');
    assert.equal(savedMessage({}, 'new'), 'Saved “new” to Oya.');
  });

  it('says a playbook Oya could not find was saved anew, without claiming it is gone', () => {
    assert.equal(
      savedMessage({ renamed: { from: 'old', missing: true } }, 'new'),
      'Saved “new” to Oya as a new playbook: Oya could not find “old” to rename. ' +
        'If “old” is still in Playbooks, delete it there.',
    );
  });

  it('says a copy left the saved playbook unchanged, and a copy under the same name is a plain save', () => {
    assert.equal(savedMessage({}, 'new', 'old'), 'Saved “new” to Oya as a copy. “old” is unchanged.');
    assert.equal(savedMessage({}, 'old', 'old'), 'Saved “old” to Oya.');
  });
});

describe('saveFailure', () => {
  it('a taken name says nothing was saved and asks for another name', () => {
    const result = { error: 'A playbook named new already exists', renameFailed: { from: 'old', status: 409 } };
    assert.equal(
      saveFailure(result, 'new'),
      'A playbook named new already exists. Nothing was saved. Choose another name.',
    );
  });

  it('any other refused rename names the playbook and says nothing was saved', () => {
    const result = {
      error: 'Playbook name must be 1-64 letters, digits, _ or -',
      renameFailed: { from: 'old', status: 400 },
    };
    assert.equal(
      saveFailure(result, 'new'),
      'Could not rename “old”: Playbook name must be 1-64 letters, digits, _ or -. Nothing was saved.',
    );
  });

  it('a rename that timed out never claims nothing was saved: it may have happened', () => {
    const result = { error: 'The server did not answer in time', renameFailed: { from: 'old', timedOut: true } };
    assert.equal(
      saveFailure(result, 'new'),
      'The server did not answer in time, so “old” may or may not have been renamed. Check Playbooks, then save again.',
    );
  });

  it('a rename whose save failed says it was renamed and to save again', () => {
    assert.equal(
      saveFailure({ error: 'Server returned 500', renamed: { from: 'old' } }, 'new'),
      'Renamed “old” to “new”, but the steps did not save: Server returned 500. Save again to finish.',
    );
  });

  it('otherwise keeps the server’s words, or says the save was not confirmed', () => {
    assert.equal(saveFailure({ error: 'offline', renamed: { from: 'old', missing: true } }, 'new'), 'offline');
    assert.equal(saveFailure({ error: 'offline' }, 'new'), 'offline');
    assert.equal(saveFailure(undefined, 'new'), 'The server did not confirm the save');
  });
});
