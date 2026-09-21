/**
 * Unit tests for scripts/workflow/normalize.cjs: the shape every draft and
 * step is brought to, and the input it refuses.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { normalizeStep, normalizeDraft } = require('../../../../scripts/workflow/normalize.cjs');

describe('normalizeStep', () => {
  it('fills defaults: a fresh id, enabled, main tab, no frames, 15 s timeout', () => {
    const step = normalizeStep({ action: 'click' });
    assert.match(step.id, /^[0-9a-f-]{36}$/);
    assert.deepEqual(
      [step.enabled, step.breakpoint, step.tab, step.frames, step.timeout],
      [true, false, 'main', [], 15000],
    );
  });

  it('keeps a safe id and replaces an unsafe one', () => {
    assert.equal(normalizeStep({ id: 'step:1.a-b' }).id, 'step:1.a-b');
    assert.notEqual(normalizeStep({ id: 'bad id' }).id, 'bad id');
  });

  it('clamps the timeout and scroll amount', () => {
    assert.equal(normalizeStep({ timeout: 10 }).timeout, 500);
    assert.equal(normalizeStep({ timeout: 1e9 }).timeout, 90000);
    assert.equal(normalizeStep({ timeout: Infinity }).timeout, 90000);
    assert.equal(normalizeStep({ amount: -300 }).amount, 300);
    assert.equal(normalizeStep({ amount: 'x' }).amount, 500);
  });

  it('derives locators from the recorded element when none are given', () => {
    const step = normalizeStep({ el: { type: 'button', text: 'Go', extra: 'dropped' } });
    assert.deepEqual(step.candidates, [{ kind: 'text', value: 'Go' }]);
    assert.deepEqual(step.el, { type: 'button', text: 'Go' });
  });

  it('keeps at most twelve locators, each reduced to its known fields', () => {
    const many = Array.from({ length: 20 }, () => ({ kind: 'css', value: '#a', role: 'x', junk: 1 }));
    const step = normalizeStep({ candidates: many });
    assert.equal(step.candidates.length, 12);
    assert.deepEqual(step.candidates[0], { kind: 'css', value: '#a' });
  });

  it('refuses malformed steps, text, locators and frame paths', () => {
    assert.throws(() => normalizeStep([]), /Invalid step/);
    assert.throws(() => normalizeStep({ url: 5 }), /Invalid step url/);
    assert.throws(() => normalizeStep({ candidates: [{ kind: 'xpath', value: 'a' }] }), /locator candidate/);
    assert.throws(
      () => normalizeStep({ candidates: [{ kind: 'role', role: 'Bad', value: 'a' }] }),
      /locator candidate/,
    );
    assert.throws(() => normalizeStep({ frames: 'iframe' }), /frame path/);
  });

  it('keeps recording marks', () => {
    assert.deepEqual([normalizeStep({ t: '12', start: 1 }).t, normalizeStep({ start: 1 }).start], [12, true]);
  });
});

describe('normalizeDraft', () => {
  it('gives a fresh paused draft with defaults', () => {
    const draft = normalizeDraft();
    assert.deepEqual(
      [draft.schemaVersion, draft.name, draft.phase, draft.steps, draft.revision],
      [2, 'Untitled workflow', 'paused', [], 0],
    );
  });

  it('never keeps a default for a secret variable', () => {
    const draft = normalizeDraft({
      variables: { pw: { secret: true, default: 'x' }, u: { default: 'demo' } },
      secrets: ['tok'],
    });
    assert.deepEqual(draft.variables, {
      pw: { secret: true },
      u: { secret: false, default: 'demo' },
      tok: { secret: true },
    });
    assert.deepEqual(draft.secrets, ['tok', 'pw']);
  });

  it('copies repair, publish and run details deeply', () => {
    const run = { status: 'x' };
    const draft = normalizeDraft({ run, publishedAt: 5 });
    assert.deepEqual(draft.run, run);
    assert.notEqual(draft.run, run);
    assert.equal(draft.publishedAt, 5);
  });

  it('refuses another schema, too many steps, and bad variables', () => {
    assert.throws(() => normalizeDraft({ schemaVersion: 3 }), /Unsupported recording version 3/);
    assert.throws(() => normalizeDraft({ steps: Array.from({ length: 501 }, () => ({})) }), /at most 500/);
    assert.throws(() => normalizeDraft({ variables: [] }), /Invalid variables/);
    assert.throws(() => normalizeDraft({ variables: { 'a b': {} } }), /Invalid variable name/);
    assert.throws(() => normalizeDraft({ variables: { a: { default: 5 } } }), /Invalid variable default/);
    assert.throws(() => normalizeDraft({ secrets: ['__proto__'] }), /Invalid secret variable/);
  });

  it('refuses two steps with one id', () => {
    assert.throws(() => normalizeDraft({ steps: [{ id: 'a' }, { id: 'a' }] }), /unique/);
  });
});
