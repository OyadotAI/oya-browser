/**
 * Unit tests for the studio's variables (renderer/studio/variables.js): what
 * was typed into a run input survives a redraw, and quick edits all land.
 */
const { describe, it } = require('node:test');
const assert = require('node:assert/strict');
const { studioApp } = require('../../support/studio.cjs');
const { Event } = require('../../support/fake-dom.cjs');

/** Steps that use {{who}} and {{where}}. */
const STEPS = [
  { id: 'a', action: 'navigate', url: 'https://x.test/{{where}}' },
  { id: 'b', action: 'type', text: '{{who}}', candidates: [{ kind: 'css', value: '#name' }] },
];

describe('studio variables', () => {
  it('keeps what was typed into a run input when the studio redraws', async () => {
    const { ws, push, $ } = await studioApp({ steps: STEPS });
    const input = $('run-inputs').querySelector('[data-variable="who"]');
    input.value = 'Ada';
    ws.edit({ type: 'metadata', name: 'renamed' });
    await push();
    assert.equal($('run-inputs').querySelector('[data-variable="who"]').value, 'Ada');
  });

  it('applies two quick edits to different variables, both of them', async () => {
    const { ws, push, $, settle } = await studioApp({ steps: STEPS });
    ws.edit({ type: 'variables', variables: { who: { default: '' }, where: { default: '' } } });
    await push();
    for (const [name, value] of [
      ['who', 'Ada'],
      ['where', 'home'],
    ]) {
      const input = $('workflow-variables').querySelector(`[data-key="${name}:default"]`);
      input.value = value;
      input.dispatchEvent(new Event('change'));
    }
    await settle();
    assert.deepEqual([ws.draft.variables.who.default, ws.draft.variables.where.default], ['Ada', 'home']);
  });
});
