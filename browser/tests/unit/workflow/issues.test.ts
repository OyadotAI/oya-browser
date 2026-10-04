/**
 * Unit tests for src/workflow/issues.ts: what blocks a run, and which
 * variables a draft uses.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { issues, variableNames } from '../../../src/workflow/issues.ts';
import { normalizeDraft } from '../../../src/workflow/normalize.ts';

const draftOf = (...steps) => normalizeDraft({ steps: steps.map((s, i) => ({ id: 's' + i, ...s })) });

describe('issues', () => {
  it('flags unsupported actions, missing targets, bad URLs, empty targets and capture problems', () => {
    const draft = draftOf(
      { action: 'execute' },
      { action: 'click' },
      { action: 'navigate', url: 'ftp://x' },
      { action: 'click', candidates: [{ kind: 'css', value: ' ' }] },
      { action: 'click', candidates: [{ kind: 'css', value: '#a' }], captureIssue: 'frame lost' },
    );
    assert.deepEqual(
      issues(draft).map((i) => [i.stepId, i.message]),
      [
        ['s0', 'Unsupported interaction: execute. Replace or disable this step.'],
        ['s1', 'Pick a target before validation.'],
        ['s2', 'Navigation requires an HTTP or HTTPS URL.'],
        ['s3', 'Target cannot be empty.'],
        ['s4', 'frame lost'],
      ],
    );
  });

  it('ignores disabled steps', () => {
    assert.deepEqual(issues(draftOf({ action: 'click', enabled: false })), []);
  });
});

describe('variableNames', () => {
  it('lists each placeholder once, in order of use', () => {
    const draft = draftOf({ action: 'type', text: '{{b}} {{a}}' }, { action: 'navigate', url: 'https://x/{{b}}' });
    assert.deepEqual(variableNames(draft), ['b', 'a']);
  });
});
