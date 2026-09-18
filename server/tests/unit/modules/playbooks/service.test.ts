/**
 * Unit tests for the playbooks facade: it exposes the module's operations
 * and nothing else.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const playbooks = await import('../../../../src/modules/playbooks/service.ts');

describe('playbooks facade', () => {
  it('exports matching, checking, variables, the export, the catalog and replay', () => {
    assert.deepEqual(Object.keys(playbooks).sort(), [
      'create',
      'list',
      'matchElement',
      'missingVariables',
      'play',
      'promote',
      'remove',
      'rename',
      'renderPlaywright',
      'sanitizeSteps',
      'templateValues',
      'validateWorkflow',
      'variablesOf',
    ]);
  });
});
