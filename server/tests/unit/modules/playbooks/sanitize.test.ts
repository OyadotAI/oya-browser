/**
 * Unit tests for checking a recording before it becomes a playbook: unknown
 * actions and fields are dropped, strings capped, duplicate navigations and
 * click-then-type pairs folded, and workflow drafts validated.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { sanitizeSteps, validateWorkflow } from '../../../../src/modules/playbooks/sanitize.ts';
import { MAX_LEN, MAX_SCROLL_AMOUNT, MAX_STEPS } from '../../../../src/modules/playbooks/constants.ts';

const GO = { action: 'navigate', url: 'https://a.test/' };
const CLICK = { action: 'click', el: { text: 'Go' } };

describe('sanitizeSteps', () => {
  it('refuses anything that is not an array', () => {
    assert.throws(() => sanitizeSteps('nope'), { status: 400, message: 'steps must be an array' });
  });

  it('refuses more steps than a recording may hold', () => {
    assert.throws(() => sanitizeSteps(Array(MAX_STEPS + 1).fill(CLICK)), { status: 400, message: /limited to 500/ });
  });

  it('refuses a recording that only navigates', () => {
    assert.throws(() => sanitizeSteps([GO]), { status: 400, message: /no actions/ });
  });

  it('keeps known actions and drops unknown ones and junk', () => {
    const out = sanitizeSteps([null, { action: 'evaluate', code: 'x' }, GO, CLICK]);
    assert.deepEqual(
      out.map((s) => s.action),
      ['navigate', 'click'],
    );
  });

  it('keeps only known step and element fields', () => {
    const [step] = sanitizeSteps([{ ...CLICK, el: { text: 'Go', onclick: 'evil()', id: 7 }, script: 'x' }]);
    assert.deepEqual(step, { action: 'click', el: { text: 'Go' } });
  });

  it('caps long strings and keeps numbers as strings', () => {
    const [step] = sanitizeSteps([{ action: 'type', text: 'x'.repeat(MAX_LEN + 10), el: { name: 42 } }]);
    assert.equal(step.text.length, MAX_LEN);
    assert.equal(step.el.name, '42');
  });

  it('caps a scroll amount and makes it positive', () => {
    const [a, b, c] = sanitizeSteps([
      { action: 'scroll', amount: -300 },
      { action: 'scroll', amount: 10 ** 9 },
      { action: 'scroll', amount: 'lots' },
    ]);
    assert.deepEqual([a.amount, b.amount, c.amount], [300, MAX_SCROLL_AMOUNT, 0]);
  });

  it('keeps the start marker', () => {
    assert.equal(sanitizeSteps([{ ...GO, start: 1 }, CLICK])[0].start, true);
  });

  it('drops a navigation that is not http(s)', () => {
    const out = sanitizeSteps([{ action: 'navigate', url: 'javascript:alert(1)' }, CLICK]);
    assert.deepEqual(
      out.map((s) => s.action),
      ['click'],
    );
  });

  it('drops a click, type or select that recorded no element', () => {
    const out = sanitizeSteps([
      { action: 'click' },
      { action: 'type', el: {} },
      { action: 'select_option', el: 'x' },
      CLICK,
    ]);
    assert.equal(out.length, 1);
  });

  it('keeps a key press or upload with no element', () => {
    const out = sanitizeSteps([
      { action: 'press_key', key: 'Enter' },
      { action: 'upload_file', file: '{{f}}' },
    ]);
    assert.equal(out.length, 2);
  });

  it('folds the same navigation seen twice in a row', () => {
    const out = sanitizeSteps([GO, GO, CLICK, GO]);
    assert.deepEqual(
      out.map((s) => s.action),
      ['navigate', 'click', 'navigate'],
    );
  });

  it('folds a click into a field followed by typing in it', () => {
    const field = { type: 'input', name: 'email' };
    const out = sanitizeSteps([
      { action: 'click', el: field },
      { action: 'type', text: 'a@b', el: field },
    ]);
    assert.deepEqual(
      out.map((s) => s.action),
      ['type'],
    );
  });

  it('keeps a click on a button before typing, and a click into another field', () => {
    const out = sanitizeSteps([
      { action: 'click', el: { type: 'button', text: 'Edit' } },
      { action: 'click', el: { type: 'input', name: 'a' } },
      { action: 'type', text: 'x', el: { type: 'input', name: 'b' } },
    ]);
    assert.equal(out.length, 3);
  });
});

describe('validateWorkflow', () => {
  it('normalises a valid draft', () => {
    const draft = validateWorkflow({ steps: [{ action: 'navigate', url: 'https://a.test/' }] });
    assert.equal(draft.schemaVersion, 2);
    assert.equal(draft.steps[0].enabled, true);
  });

  it('answers 400 with the generator’s reason for an invalid draft', () => {
    assert.throws(() => validateWorkflow({ steps: [{ action: 'navigate', url: 'ftp://x' }] }), {
      status: 400,
      message: /HTTP or HTTPS/,
    });
    assert.throws(() => validateWorkflow({ schemaVersion: 3 }), {
      status: 400,
      message: /Unsupported recording version 3/,
    });
  });
});
