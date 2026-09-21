/**
 * Unit tests for scripts/workspace/edits.cjs: each editor command's change to
 * the draft copy.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { applyEdit } = require('../../../../scripts/workspace/edits.cjs');
const { normalizeDraft } = require('../../../../scripts/workflow.cjs');

const draft = () =>
  normalizeDraft({
    variables: { user: { default: 'ann' }, pw: { secret: true } },
    steps: [
      { id: 'a', action: 'type', text: '{{user}}', candidates: [{ kind: 'css', value: '#u' }] },
      { id: 'b', action: 'click', candidates: [{ kind: 'css', value: '#go' }] },
      { id: 'c', action: 'press_key', key: 'Enter' },
    ],
  });
const ids = (d) => d.steps.map((s) => s.id);

describe('applyEdit', () => {
  it('caps the name and description', () => {
    const d = draft();
    applyEdit(d, { type: 'metadata', name: 'n'.repeat(80), description: 'x' });
    assert.deepEqual([d.name.length, d.description], [64, 'x']);
  });

  it('renames a variable in its settings, the secret list and every placeholder', () => {
    const d = draft();
    applyEdit(d, { type: 'rename-variable', name: 'user', nextName: 'login' });
    assert.deepEqual(Object.keys(d.variables).sort(), ['login', 'pw']);
    assert.equal(d.steps[0].text, '{{login}}');
    applyEdit(d, { type: 'rename-variable', name: 'pw', nextName: 'secret' });
    assert.deepEqual(d.secrets, ['secret']);
  });

  it('refuses a variable name that is taken, reserved or not an identifier', () => {
    for (const nextName of ['pw', '__proto__', '1x']) {
      assert.throws(
        () => applyEdit(draft(), { type: 'rename-variable', name: 'user', nextName }),
        /unique variable name/,
      );
    }
  });

  it('replaces the variables, deriving the secret list', () => {
    const d = draft();
    applyEdit(d, { type: 'variables', variables: { k: { secret: true }, v: {} } });
    assert.deepEqual(d.secrets, ['k']);
  });

  it('adds after the given step, or at the end', () => {
    const d = draft();
    applyEdit(d, { type: 'add', id: 'a', step: { id: 'n1', action: 'wait' } });
    applyEdit(d, { type: 'add', step: { id: 'n2', action: 'wait' } });
    assert.deepEqual(ids(d), ['a', 'n1', 'b', 'c', 'n2']);
  });

  it('patches a step, which must still exist', () => {
    const d = draft();
    applyEdit(d, { type: 'update', id: 'b', patch: { timeout: 2500, id: 'hijack' } });
    assert.deepEqual([d.steps[1].id, d.steps[1].timeout], ['b', 2500]);
    assert.throws(() => applyEdit(d, { type: 'update', id: 'gone', patch: {} }), /no longer exists/);
  });

  it('deletes, duplicates and moves steps, ignoring missing ones', () => {
    const d = draft();
    applyEdit(d, { type: 'duplicate', id: 'a' });
    assert.equal(d.steps.length, 4);
    assert.notEqual(d.steps[1].id, 'a');
    applyEdit(d, { type: 'delete', id: d.steps[1].id });
    applyEdit(d, { type: 'move', id: 'a', delta: 10 });
    assert.deepEqual(ids(d), ['b', 'c', 'a']);
    applyEdit(d, { type: 'move', id: 'a', delta: -10 });
    applyEdit(d, { type: 'delete', id: 'gone' });
    applyEdit(d, { type: 'move', id: 'gone', delta: 1 });
    assert.deepEqual(ids(d), ['a', 'b', 'c']);
  });

  it('refuses an unknown command, including prototype names', () => {
    assert.throws(() => applyEdit(draft(), { type: 'explode' }), /Unknown editor command/);
    assert.throws(() => applyEdit(draft(), { type: 'toString' }), /Unknown editor command/);
  });
});
