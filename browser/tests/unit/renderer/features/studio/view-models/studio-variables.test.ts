/**
 * Unit tests for the studio's variables (studio-variables.ts): what was typed
 * into a run input survives a redraw, quick edits all land, a secret loses
 * its default, renaming and removing.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { studioApp } from '../studio-app.ts';

/** Steps that use {{who}} and {{where}}. */
const STEPS = [
  { id: 'a', action: 'navigate', url: 'https://x.test/{{where}}' },
  { id: 'b', action: 'type', text: '{{who}}', candidates: [{ kind: 'css', value: '#name' }] },
];

describe('StudioVariables', () => {
  it('keeps what was typed into a run input when the studio redraws', async () => {
    const { vm, ws, push } = await studioApp({ steps: STEPS });
    vm.run.setInput('who', 'Ada');
    ws.edit({ type: 'metadata', name: 'renamed' });
    await push();
    assert.equal(vm.state.runInputs.who, 'Ada');
  });

  it('offers one run input per variable the steps use, each at its default when the set of inputs changes', async () => {
    const { vm, ws, push } = await studioApp({ steps: STEPS });
    assert.deepEqual(vm.state.runInputs, { where: '', who: '' });
    ws.edit({ type: 'variables', variables: { who: { default: 'Ann' } } });
    ws.edit({ type: 'add', step: { action: 'type', text: '{{extra}}', candidates: [] } });
    await push();
    assert.deepEqual(vm.state.runInputs, { where: '', who: 'Ann', extra: '' });
  });

  it('applies two quick edits to different variables, both of them', async () => {
    const { vm, ws, push } = await studioApp({ steps: STEPS });
    ws.edit({ type: 'variables', variables: { who: { default: '' }, where: { default: '' } } });
    await push();
    await Promise.all([vm.variables.setDefault('who', 'Ada'), vm.variables.setDefault('where', 'home')]);
    assert.deepEqual([ws.draft.variables.who.default, ws.draft.variables.where.default], ['Ada', 'home']);
  });

  it('drops a variable’s default when it becomes secret', async () => {
    const { vm, ws, push } = await studioApp({ steps: STEPS });
    ws.edit({ type: 'variables', variables: { who: { default: 'Ann' } } });
    await push();
    await vm.variables.setSecret('who', true);
    assert.equal(ws.draft.variables.who.secret, true);
    assert.equal(ws.draft.variables.who.default, undefined);
  });

  it('renames a variable everywhere, and says why a name is refused', async () => {
    const { vm, ws, push } = await studioApp({ steps: STEPS });
    ws.edit({ type: 'variables', variables: { who: { default: '' } } });
    await push();
    await vm.variables.rename('who', 'person');
    assert.equal(ws.draft.steps[1].text, '{{person}}');
    await vm.variables.rename('person', 'not ok');
    assert.equal(vm.state.messages['record-result'].error, true);
  });

  it('removes a variable', async () => {
    const { vm, ws, push } = await studioApp({ steps: STEPS });
    ws.edit({ type: 'variables', variables: { who: { default: '' }, where: { default: '' } } });
    await push();
    await vm.variables.remove('who');
    assert.deepEqual(Object.keys(ws.draft.variables), ['where']);
  });
});
