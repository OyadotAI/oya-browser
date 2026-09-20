/**
 * Unit tests for the recording archive without shared storage configured:
 * archiving is a no-op, unknown or foreign recordings read as absent, and an
 * owned one reports storage as unavailable. The Supabase storage path needs a
 * client this process cannot be given without replacing an import.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-control-recording-');
const { archiveRecording, archivedFrame, archivedManifest, removeArchive } =
  await import('../../../../src/modules/control/recording-storage.ts');
const { control } = await import('../../../../src/modules/control/service.ts');
const { putRow } = await import('../../support/control.ts');

const ID = '123e4567-e89b-12d3-a456-426614174000';
beforeEach(() => putRow(control(), 'recording', ID, { sessionId: ID, owner: 'owner-a' }));

describe('recording archive without shared storage', () => {
  it('archives nothing', async () => {
    await archiveRecording({ owner: 'owner-a', sessionId: 'other' }, '/nonexistent');
    assert.equal(await control().store.get('recording', 'other'), null);
  });

  it('reads an unsafe id, an unknown one, or another owner’s as absent', async () => {
    assert.equal(await archivedManifest('../x', 'owner-a'), null);
    assert.equal(await archivedManifest('00000000-0000-0000-0000-000000000000', 'owner-a'), null);
    assert.equal(await archivedManifest(ID, 'owner-b'), null);
    assert.equal(await archivedFrame(ID, 1, 'owner-b'), null);
  });

  it('reports storage unavailable for a recording the caller owns', async () => {
    await assert.rejects(archivedManifest(ID, 'owner-a'), /storage unavailable/);
    await assert.rejects(removeArchive(ID, 'owner-a'), /storage unavailable/);
  });

  it('removes nothing for another owner’s recording', async () => {
    await removeArchive(ID, 'owner-b');
    assert.ok(await control().store.get('recording', ID));
  });
});
