/**
 * Unit tests for the control service entry point: the process-wide service and
 * keyOfProject, which background work uses to act for a project.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir('oya-control-service-');
const { control, keyOfProject, projectId } = await import('../../../../src/modules/control/service.ts');
const { patchRow, scratchService } = await import('../../support/control.ts');

describe('control', () => {
  it('is one service for the process', () => {
    assert.equal(control(), control());
  });
});

describe('keyOfProject', () => {
  it('unseals the key a project was created with', async () => {
    const service = scratchService();
    await service.project('key-a');
    assert.equal(await keyOfProject(projectId('key-a'), service), 'key-a');
  });

  it('answers null for an unknown project', async () => {
    assert.equal(await keyOfProject('prj_missing', scratchService()), null);
  });

  it('answers null rather than throwing when the key cannot be unsealed', async () => {
    const service = scratchService();
    await service.project('key-a');
    await patchRow(service, 'project', projectId('key-a'), { key: 'garbage' });
    assert.equal(await keyOfProject(projectId('key-a'), service), null);
  });

  it('uses the process-wide service by default', async () => {
    await control().project('key-z');
    assert.equal(await keyOfProject(projectId('key-z')), 'key-z');
  });
});
