/**
 * Unit tests for cleaning legacy recording noise out of stored playbooks:
 * hidden-input steps, templated click labels and unused variables.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { cleanPlaybook } from '../../../../src/modules/playbooks/cleanup.ts';

const HIDDEN = { action: 'type', text: '{{token}}', el: { tag: 'input', type: 'hidden' } };
const HIDDEN_BY_INPUT_TYPE = { action: 'type', el: { tag: 'input', inputType: 'hidden' } };
const CLICK = { action: 'click', el: { text: 'Go' } };

describe('cleanPlaybook', () => {
  it('returns anything without a steps array unchanged', () => {
    const odd = { name: 'x' };
    assert.equal(cleanPlaybook(odd), odd);
    assert.equal(cleanPlaybook(null), null);
  });

  it('returns a copy, leaving the original untouched', () => {
    const pb = { steps: [HIDDEN, CLICK] };
    const cleaned = cleanPlaybook(pb);
    assert.notEqual(cleaned, pb);
    assert.equal(pb.steps.length, 2);
  });

  it('drops steps on hidden inputs, by type or input type', () => {
    const cleaned = cleanPlaybook({ steps: [HIDDEN, HIDDEN_BY_INPUT_TYPE, CLICK] });
    assert.deepEqual(cleaned.steps, [CLICK]);
  });

  it('keeps steps on a hidden-looking element that is not an input', () => {
    const div = { action: 'click', el: { tag: 'div', type: 'hidden' } };
    assert.equal(cleanPlaybook({ steps: [div] }).steps.length, 1);
  });

  it('moves healedFrom back past the hidden steps it followed', () => {
    const cleaned = cleanPlaybook({ steps: [HIDDEN, CLICK, HIDDEN, CLICK], healedFrom: 3 });
    assert.equal(cleaned.healedFrom, 1);
  });

  it('gives a click templated from a button label its recorded label back', () => {
    const pb = {
      steps: [{ action: 'click', el: { text: '{{signIn}}' } }],
      labels: ['signIn'],
      defaults: { signIn: 'Sign in' },
    };
    const cleaned = cleanPlaybook(pb);
    assert.equal(cleaned.steps[0].el.text, 'Sign in');
    assert.deepEqual(cleaned.labels, []);
    assert.deepEqual(cleaned.defaults, {});
  });

  it('leaves a templated click alone when its name is a secret, not a label, or has no text default', () => {
    const click = { action: 'click', el: { text: '{{x}}' } };
    assert.equal(
      cleanPlaybook({ steps: [click], labels: ['x'], secrets: ['x'], defaults: { x: 'X' } }).steps[0].el.text,
      '{{x}}',
    );
    assert.equal(cleanPlaybook({ steps: [click], labels: [], defaults: { x: 'X' } }).steps[0].el.text, '{{x}}');
    assert.equal(cleanPlaybook({ steps: [click], labels: ['x'], defaults: { x: 5 } }).steps[0].el.text, '{{x}}');
  });

  it('drops defaults, labels and secrets nothing uses any more', () => {
    const pb = {
      steps: [
        HIDDEN,
        { action: 'type', text: '{{email|lower}}', el: { name: 'e' } },
        { action: 'click', el: { text: '{{plan}}' } },
      ],
      defaults: { token: 't', email: 'a@b', plan: 'Pro' },
      labels: ['plan', 'token', 'email'],
      secrets: ['token', 'email', 'plan'],
    };
    const cleaned = cleanPlaybook(pb);
    assert.deepEqual(cleaned.defaults, { email: 'a@b', plan: 'Pro' });
    assert.deepEqual(cleaned.labels, ['plan']);
    assert.deepEqual(cleaned.secrets, ['email', 'plan']);
  });

  it('keeps a variable the healing prompt still uses', () => {
    const cleaned = cleanPlaybook({ steps: [HIDDEN], prompt: 'Use {{token}}', defaults: { token: 't' } });
    assert.deepEqual(cleaned.defaults, { token: 't' });
  });
});
