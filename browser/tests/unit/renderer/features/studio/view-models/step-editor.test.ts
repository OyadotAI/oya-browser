/**
 * Unit tests for the step editor (step-editor.ts, step-format.ts): its
 * heading follows moves, quick edits build on each other, toggles flip the
 * step as it is now, running to a step needs a runnable workflow, and how a
 * step's row reads.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { studioApp } from '../studio-app.ts';
import { studioMode } from '../../../../../../src/renderer/features/studio/model/studio-model.ts';
import {
  editorTitle,
  framesText,
  stepNumber,
  stepValue,
} from '../../../../../../src/renderer/features/studio/model/step-format.ts';
import type { Step } from '../../../../../../src/renderer/features/studio/model/types.ts';

/** The step `id` in the studio's snapshot. */
const stepOf = (vm: any, id: string): Step => vm.state.snapshot!.draft.steps.find((s) => s.id === id)!;

describe('StepEditor', () => {
  it('renumbers its heading when the step moves', async () => {
    const { vm } = await studioApp();
    vm.select('b');
    const steps = () => vm.state.snapshot!.draft.steps;
    assert.equal(editorTitle(stepOf(vm, 'b'), steps()), 'Step 2 · Click');
    await vm.editor.move('b', -1);
    assert.equal(editorTitle(stepOf(vm, 'b'), steps()), 'Step 1 · Click');
  });

  it('builds a second quick edit on the first instead of overwriting it', async () => {
    const { vm, ws } = await studioApp();
    const kind = vm.editor.setTarget('b', 'kind', 'text');
    const value = vm.editor.setTarget('b', 'value', 'Go');
    await Promise.all([kind, value]);
    const target = ws.draft.steps.find((s: Step) => s.id === 'b').candidates[0];
    assert.deepEqual([target.kind, target.value], ['text', 'Go']);
  });

  it('flips a step off and on, and its breakpoint, as the step is when the command runs', async () => {
    const { vm } = await studioApp();
    await Promise.all([vm.editor.toggle('b', 'enabled'), vm.editor.toggle('b', 'enabled')]);
    assert.equal(stepOf(vm, 'b').enabled, true);
    await vm.editor.toggle('b', 'breakpoint');
    assert.equal(stepOf(vm, 'b').breakpoint, true);
    assert.equal(stepNumber(stepOf(vm, 'b'), 1), '●');
  });

  it('turns off Run to here while a step blocks the run', async () => {
    const { vm } = await studioApp({ steps: [{ id: 'x', action: 'click', candidates: [] }] });
    assert.equal(studioMode(vm.state).runnable, false);
  });

  it('runs to a step with a test run that stops there', async () => {
    const { vm, fake } = await studioApp();
    await vm.editor.runTo('b');
    const cmd = fake.called('workspace').at(-1)?.[0] as any;
    assert.deepEqual([cmd.type, cmd.runTo], ['validate', 'b']);
  });

  it('sends no timeout when the timeout field is cleared', async () => {
    const patches: object[] = [];
    const { vm, ws } = await studioApp({ commands: { update: (cmd) => (patches.push(cmd.patch), ws.snapshot()) } });
    await vm.editor.setTimeout('b', undefined);
    assert.equal(patches.length, 1);
    assert.ok(Object.hasOwn(patches[0], 'timeout'));
    assert.equal((patches[0] as any).timeout, undefined);
  });

  it('duplicates and deletes a step', async () => {
    const { vm } = await studioApp();
    await vm.editor.duplicate('b');
    assert.equal(vm.state.snapshot!.draft.steps.length, 3);
    await vm.editor.remove('b');
    assert.equal(vm.state.snapshot!.draft.steps.length, 2);
  });

  it('writes frames joined by arrows and reads them back without empty levels', async () => {
    const { vm } = await studioApp();
    await vm.editor.setFrames('b', 'iframe#a → → iframe#b');
    assert.deepEqual(stepOf(vm, 'b').frames, ['iframe#a', 'iframe#b']);
    assert.equal(framesText(stepOf(vm, 'b').frames), 'iframe#a → iframe#b');
  });

  it('makes another recorded target the first, keeping the rest behind it', async () => {
    const steps = [
      { id: 'a', action: 'navigate', url: 'https://x.test/' },
      {
        id: 'b',
        action: 'click',
        candidates: [
          { kind: 'css', value: '#go' },
          { kind: 'text', value: 'Go' },
        ],
      },
    ];
    const { vm } = await studioApp({ steps });
    await vm.editor.chooseTarget('b', { kind: 'text', value: 'Go' });
    assert.deepEqual(stepOf(vm, 'b').candidates, [
      { kind: 'text', value: 'Go' },
      { kind: 'css', value: '#go' },
    ]);
  });

  it('says how to pick a target once picking started, and nothing when it was refused', async () => {
    const { vm, ws } = await studioApp({ commands: { pick: () => ws.snapshot() } });
    await vm.editor.pick('b');
    assert.equal(vm.state.messages['record-result'].text, 'Click an element in the page. Escape cancels.');
    const refused = await studioApp({
      commands: {
        pick: () => {
          throw new Error('Open a page first');
        },
      },
    });
    await refused.vm.editor.pick('b');
    assert.deepEqual(refused.vm.state.messages['record-result'], { text: 'Open a page first', error: true });
  });
});

describe('a step row', () => {
  /** The row values of `steps`, after the studio took them in. */
  async function values(steps: unknown[], variables = {}) {
    const { vm, ws, push } = await studioApp({ steps });
    if (Object.keys(variables).length) ws.edit({ type: 'variables', variables });
    await push();
    const d = vm.state.snapshot!.draft;
    return d.steps.map((step) => stepValue(step, d.variables));
  }

  it('names a step by its label on the page, not the CSS id replay tries first', async () => {
    const steps = [
      {
        id: 'e',
        action: 'fill',
        candidates: [
          { kind: 'css', value: '[id="email"]' },
          { kind: 'label', value: 'Email' },
        ],
      },
      { id: 'g', action: 'click', candidates: [{ kind: 'css', value: '#go' }] },
    ];
    assert.deepEqual(await values(steps), ['Email', '#go']);
  });

  it('shows what a step types, picks or expects next to its target, and a secret only as a secret', async () => {
    const steps = [
      { id: 't', action: 'type', text: 'ann', candidates: [{ kind: 'label', value: 'Name' }] },
      { id: 'p', action: 'type', text: '{{password}}', candidates: [{ kind: 'label', value: 'Password' }] },
      { id: 's', action: 'select_option', option: 'Canada', candidates: [{ kind: 'label', value: 'Country' }] },
      { id: 'c', action: 'assert_page', expected: 'https://x.test/done?session=abc&s=price', params: 's' },
      { id: 'b', action: 'go_back' },
    ];
    assert.deepEqual(await values(steps, { password: { secret: true } }), [
      'Name · “ann”',
      'Password · secret {{password}}',
      'Country · “Canada”',
      'x.test/done (s=price)',
      'to the previous page',
    ]);
  });

  it('names a step with no words on the page by what was recorded about it, else a short selector', async () => {
    const steps = [
      {
        id: 'a',
        action: 'click',
        el: { type: 'checkbox', name: 'terms' },
        candidates: [{ kind: 'css', value: '[name="terms"]' }],
      },
      {
        id: 'b',
        action: 'click',
        candidates: [{ kind: 'css', value: 'body > div > ul > li:nth-of-type(2) > [id="x"]' }],
      },
    ];
    assert.deepEqual(await values(steps), ['terms checkbox', '… > li:nth-of-type(2) > #x']);
  });

  it('asks to be configured when it has nothing to show', async () => {
    assert.deepEqual(await values([{ id: 'w', action: 'wait', candidates: [] }]), ['Select to configure']);
  });

  it('numbers steps with two digits', () => {
    assert.equal(stepNumber({ id: 'x', action: 'click', enabled: true, breakpoint: false }, 2), '03');
  });
});
