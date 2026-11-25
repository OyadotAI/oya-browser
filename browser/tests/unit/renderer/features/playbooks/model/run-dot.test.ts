/** The library row's last-run dot (model/run-dot.ts): one test per state it can show. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { runDot } from '../../../../../../src/renderer/features/playbooks/model/run-dot.ts';

const NOW = Date.parse('2026-11-17T12:00:00Z');
const base = { name: 'lookup', variables: [], defaults: {}, steps: 2, code: '' };
const TWO_HOURS_BEFORE = '2026-11-17T10:00:00Z';

it('shows no dot when the server could not say how it last ran', () => {
  assert.equal(runDot(base, null, NOW), null);
});

it('shows a hollow dot for a playbook that never ran', () => {
  assert.deepEqual(runDot({ ...base, lastRun: null }, null, NOW), { dot: 'never', label: 'Never run' });
});

it('says when the last run passed or failed', () => {
  const passed = runDot({ ...base, lastRun: { status: 'succeeded', endedAt: TWO_HOURS_BEFORE } }, null, NOW);
  assert.equal(passed?.dot, 'succeeded');
  assert.match(passed!.label, /^Last run succeeded, .*2 hours ago$/);
  const failed = runDot({ ...base, lastRun: { status: 'failed', endedAt: TWO_HOURS_BEFORE } }, null, NOW);
  assert.equal(failed?.dot, 'failed');
});

it('a run this desktop is following wins over the last result, and only on its own row', () => {
  const item = { ...base, lastRun: { status: 'failed' as const, endedAt: TWO_HOURS_BEFORE } };
  assert.deepEqual(runDot(item, { name: 'lookup', status: 'running' }, NOW), { dot: 'running', label: 'Running now' });
  assert.deepEqual(runDot(item, { name: 'lookup', status: 'needs_attention' }, NOW), {
    dot: 'attention',
    label: 'Needs your help',
  });
  assert.equal(runDot(item, { name: 'other', status: 'running' }, NOW)?.dot, 'failed');
});
