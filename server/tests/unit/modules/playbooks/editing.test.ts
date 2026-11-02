/**
 * Unit tests for editing a saved playbook in place (playbooks/editing.ts): the
 * whole definition comes back without secret defaults, a replacement is checked
 * as an import is and keeps a secret's saved value when left blank, and a draft
 * or a missing playbook is a 404.
 */
import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { ownDataDir } from '../../support/data-dir.ts';

ownDataDir();
const editing = await import('../../../../src/modules/playbooks/editing.ts');
const keyConfig = await import('../../../../src/modules/config/service.ts');

const KEY = 'editing-key';
const STEPS = [
  { action: 'navigate', url: 'https://a.test/' },
  { action: 'type', text: '{{email}}', el: { name: 'email', tag: 'input' } },
];
const CLASSIC = {
  name: 'signup',
  prompt: 'Sign up',
  steps: STEPS,
  defaults: { email: 'ada@x.test' },
  secrets: [],
  createdAt: 'c',
};
const VARIABLES = { email: { secret: false, default: 'ada@x.test' }, pw: { secret: true, default: 'hunter2' } };

describe('playbook editing', () => {
  beforeEach(async () => {
    keyConfig.reset();
    await keyConfig.savePlaybook(KEY, 'signup', structuredClone(CLASSIC));
  });

  it('answers the whole definition, without the history kept here', () => {
    const def = editing.definitionOf(KEY, 'signup');
    assert.deepEqual(def, {
      name: 'signup',
      prompt: 'Sign up',
      steps: STEPS,
      defaults: { email: 'ada@x.test' },
      secrets: [],
    });
  });

  it('never sends back a secret variable’s default', async () => {
    await keyConfig.savePlaybook(KEY, 'wf', { ...CLASSIC, variables: VARIABLES });
    const def = editing.definitionOf(KEY, 'wf');
    assert.equal(def.variables.pw.default, undefined);
    assert.equal(def.variables.email.default, 'ada@x.test');
  });

  it('answers 404 for a playbook that is not there, and for a healed draft', () => {
    assert.throws(() => editing.definitionOf(KEY, 'ghost'), { status: 404 });
    assert.throws(() => editing.definitionOf(KEY, 'signup:draft'), { status: 404 });
  });

  it('replaces the definition and keeps when it was made', async () => {
    const edited = { ...CLASSIC, steps: [STEPS[0]], defaults: {} };
    const saved = await editing.replacePlaybook(KEY, 'signup', edited);
    assert.equal(saved.steps, 1);
    const stored = keyConfig.getPlaybook(KEY, 'signup');
    assert.deepEqual([stored.createdAt, stored.steps.length, typeof stored.updatedAt], ['c', 1, 'string']);
  });

  it('keeps a secret’s saved value when the edit leaves its default blank', async () => {
    await keyConfig.savePlaybook(KEY, 'wf', { ...CLASSIC, variables: VARIABLES });
    const edited = { ...CLASSIC, variables: { ...VARIABLES, pw: { secret: true } } };
    await editing.replacePlaybook(KEY, 'wf', edited);
    assert.equal(keyConfig.getPlaybook(KEY, 'wf').variables.pw.default, 'hunter2');
  });

  it('never sends back the value of a classic playbook’s secret, and keeps it when the edit leaves it out', async () => {
    await keyConfig.savePlaybook(KEY, 'login', {
      ...CLASSIC,
      defaults: { email: 'a@x', pw: 'hunter2' },
      secrets: ['pw'],
    });
    const def = editing.definitionOf(KEY, 'login');
    assert.deepEqual(def.defaults, { email: 'a@x' });
    await editing.replacePlaybook(KEY, 'login', def);
    assert.equal(keyConfig.getPlaybook(KEY, 'login').defaults.pw, 'hunter2');
  });

  it('drops a field the edit removed instead of keeping the saved one', async () => {
    await keyConfig.savePlaybook(KEY, 'signup', { ...CLASSIC, prompt: 'Old task' });
    const { prompt, ...withoutPrompt } = editing.definitionOf(KEY, 'signup');
    assert.equal(prompt, 'Old task');
    await editing.replacePlaybook(KEY, 'signup', withoutPrompt);
    assert.equal(keyConfig.getPlaybook(KEY, 'signup').prompt, undefined);
  });

  it('refuses secrets that are not a list of names, and an over-long prompt, so the playbook list cannot break', async () => {
    await assert.rejects(editing.replacePlaybook(KEY, 'signup', { ...CLASSIC, secrets: {} }), {
      status: 400,
      message: 'secrets must be a list of variable names',
    });
    await assert.rejects(editing.replacePlaybook(KEY, 'signup', { ...CLASSIC, prompt: 'x'.repeat(2001) }), {
      status: 400,
    });
    assert.equal(keyConfig.getPlaybook(KEY, 'signup').secrets.length, 0);
  });

  it('marks a tested definition as never stored', () => {
    assert.equal(editing.draftPlaybook({ steps: STEPS }).transient, true);
  });

  it('refuses steps this server could not replay, a body that is not an object, and an unknown playbook', async () => {
    await assert.rejects(editing.replacePlaybook(KEY, 'signup', { steps: [{ action: 'eval' }] }), {
      status: 400,
      message: 'Step 1 is not a step this server can replay',
    });
    await assert.rejects(editing.replacePlaybook(KEY, 'signup', { steps: [] }), { status: 400 });
    await assert.rejects(editing.replacePlaybook(KEY, 'signup', [] as any), { status: 400 });
    await assert.rejects(editing.replacePlaybook(KEY, 'ghost', CLASSIC), { status: 404 });
  });
});
