/**
 * Schema-version-2 workflows: validation strips secret defaults, the
 * Playwright export keeps frames and assertions, missing variables are
 * reported, replay is dispatched to the browser as one `workflow` command, and
 * legacy steps still sanitize.
 */
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
const directory = await mkdtemp(join(tmpdir(), 'oya-workflow-v2-'));
process.env.OYA_DATA_DIR = directory;
const playbooks = await import('../../src/modules/playbooks/service.ts');
const { registry } = await import('../../src/modules/browsers/registry.ts');
const calls = [];
registry.add('workflow-fixture', {
  apiKey: 'fixture',
  engine: {
    send: async (action, params) => {
      calls.push({ action, params });
      return { ok: true, data: { id: 'run', status: 'succeeded', assertions: 1 } };
    },
  },
});
try {
  const draft = playbooks.validateWorkflow({
    schemaVersion: 2,
    variables: { name: { default: 'fixture' }, password: { secret: true, default: 'NEVER' } },
    steps: [
      { id: 'start', action: 'navigate', url: 'https://example.com' },
      { id: 'fill', action: 'type', text: '{{password}}', candidates: [{ kind: 'label', value: 'Password' }] },
      {
        id: 'assert',
        action: 'assert_text',
        expected: '{{name}}',
        frames: ['iframe[name="form"]'],
        candidates: [{ kind: 'css', value: 'output' }],
      },
    ],
  });
  assert.equal(draft.variables.password.default, undefined);
  const code = playbooks.renderPlaywright(draft);
  assert.ok(code.includes('frameLocator'));
  assert.ok(!code.includes('.first()'));
  assert.ok(!code.includes('NEVER'));
  assert.deepEqual(playbooks.missingVariables(draft, { name: 'Ada' }), ['password']);
  const result = await playbooks.play(
    'fixture',
    'workflow-fixture',
    draft,
    { name: 'Ada', password: 'secret' },
    { autoHeal: false },
  );
  assert.equal(result.assertions, 1);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].action, 'workflow');
  assert.equal(calls[0].params.autoHeal, false);
  assert.deepEqual(calls[0].params.draft.steps[2].frames, ['iframe[name="form"]']);
  assert.throws(() => playbooks.validateWorkflow({ schemaVersion: 3, steps: [] }), /Unsupported/);
  assert.throws(() => playbooks.validateWorkflow({ schemaVersion: 2, steps: [{ action: 'unknown' }] }), /Unsupported/);
  const legacy = playbooks.sanitizeSteps([{ action: 'click', el: { domId: 'button' } }]);
  assert.equal(legacy.length, 1);
  console.log(
    'Workflow v2: shared export, frame/assertion preservation, secret defaults, variable validation, replay dispatch and legacy compatibility passed',
  );
} finally {
  registry.remove('workflow-fixture');
  await rm(directory, { recursive: true, force: true });
}
