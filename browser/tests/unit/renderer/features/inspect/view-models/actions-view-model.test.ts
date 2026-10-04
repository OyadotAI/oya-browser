/**
 * Unit tests for the Inspect Actions pane: inputs are checked before sending,
 * Enter runs a field's action, one action runs at a time with a labelled
 * result, page actions wait for control, an analysis lists elements to pick,
 * a long result is cut with a note, Copy says how it went, and Clear empties.
 */
import { describe, it, mock, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import {
  ActionsViewModel,
  actionLook,
} from '../../../../../../src/renderer/features/inspect/view-models/actions-view-model.ts';
import { RendererConstants as C } from '../../../../../../src/renderer/core/constants.ts';
import { fakeBridge } from '../../../support/bridge.ts';

/** A panel that records Clear registrations. */
function fakePanel() {
  const clears = new Map<string, () => void>();
  return {
    clears,
    onClear: (pane: string, fn: () => void) => (clears.set(pane, fn), () => void clears.delete(pane)),
    show() {},
  };
}

/** Lets pending promises settle. */
const settle = () => new Promise((resolve) => setImmediate(resolve));

/** An Actions pane over a fake bridge answering `answers`. */
async function pane(answers: Record<string, unknown> = {}, clipboard = { writeText: async (_t: string) => {} }) {
  const fake = fakeBridge({ getControlState: { interactive: true }, ...answers });
  const panel = fakePanel();
  const vm = new ActionsViewModel({ bridge: fake.bridge, panel: panel as never, clipboard });
  await settle();
  return { fake, panel, vm };
}

afterEach(() => mock.timers.reset());

describe('ActionsViewModel', () => {
  it('refuses an empty or non-numeric coordinate instead of clicking the corner', async () => {
    const { fake, vm } = await pane();
    vm.setField('action-cy', '5');
    await vm.run('click-coords');
    assert.deepEqual(fake.called('devAction'), []);
    assert.match(vm.state.result?.text ?? '', /X and Y as numbers/);
    assert.equal(vm.state.result?.failed, true);
  });

  it('refuses an element number that is not a whole number', async () => {
    const { fake, vm } = await pane();
    vm.setField('action-click-id', 'a1b2');
    await vm.run('click');
    assert.deepEqual(fake.called('devAction'), []);
    assert.match(vm.state.result?.text ?? '', /element number/);
  });

  it('runs a field’s action when Enter is pressed in it', async () => {
    const { fake, vm } = await pane({ devAction: { ok: true, data: {} } });
    vm.setField('action-key', ' Escape ');
    await vm.enter('action-key');
    vm.setField('action-click-id', '3');
    vm.setField('action-type-text', 'hi');
    await vm.enter('action-type-text');
    assert.deepEqual(fake.called('devAction'), [
      ['press-key', { key: 'Escape' }],
      ['type', { element_id: '3', text: 'hi' }],
    ]);
  });

  it('turns off page actions while the agent has control, but not a screenshot', async () => {
    const { fake, vm } = await pane();
    fake.emit('onControlState', { interactive: false, mode: 'agent' });
    assert.deepEqual(actionLook(vm.state, 'click'), { disabled: true, title: 'Take control to use this' });
    assert.equal(actionLook(vm.state, 'analyze').disabled, true);
    assert.equal(actionLook(vm.state, 'screenshot').disabled, false);
    await vm.run('reload');
    assert.deepEqual(fake.called('devAction'), []);
  });

  it('runs one action at a time and labels the result with it', async () => {
    let finish: (value: unknown) => void = () => {};
    const { fake, vm } = await pane({ devAction: () => new Promise((resolve) => (finish = resolve)) });
    const first = vm.run('reload');
    await vm.run('screenshot');
    assert.equal(fake.called('devAction').length, 1);
    assert.equal(actionLook(vm.state, 'screenshot').disabled, true);
    finish({ ok: true });
    await first;
    assert.match(vm.state.result?.title ?? '', /^Reload · done · /);
    assert.equal(actionLook(vm.state, 'screenshot').disabled, false);
  });

  it('names an action by its title, and says when it failed', async () => {
    const { vm } = await pane({ devAction: { ok: false, error: 'No tab' } });
    await vm.run('list-tabs');
    assert.match(vm.state.result?.title ?? '', /^Tabs · failed · /);
    assert.equal(vm.state.result?.text, 'No tab');
  });

  it('shows a call that threw as a failure', async () => {
    const { vm } = await pane({
      devAction: () => {
        throw new Error('gone');
      },
    });
    await vm.run('reload');
    assert.deepEqual([vm.state.result?.failed, vm.state.result?.text, vm.state.running], [true, 'gone', null]);
  });

  it('says when a long result was cut short, and copies all of it', async () => {
    const page = 'x'.repeat(C.ANALYZE_PREVIEW + 10);
    const copied: string[] = [];
    const { vm } = await pane(
      { devAction: { ok: true, data: { page, elements: [] } } },
      {
        writeText: async (t: string) => void copied.push(t),
      },
    );
    await vm.run('analyze');
    assert.deepEqual([vm.state.result?.cut, vm.state.result?.text.length], [true, C.ANALYZE_PREVIEW]);
    await vm.copy();
    assert.equal(copied[0], page);
  });

  it('shows a screenshot as an image', async () => {
    const { vm } = await pane({ devAction: { ok: true, data: { screenshot: 'data:image/png;base64,AA' } } });
    await vm.run('screenshot');
    assert.equal(vm.state.result?.image, 'data:image/png;base64,AA');
  });

  it('shows other results as JSON', async () => {
    const { vm } = await pane({ devAction: { ok: true, data: { tabs: 2 } } });
    await vm.run('new-tab');
    assert.equal(vm.state.result?.text, JSON.stringify({ tabs: 2 }, null, C.JSON_INDENT));
  });

  it('lists an analysis’s visible elements, and picking one fills the element number', async () => {
    const elements = [
      { id: 12, type: 'button', text: 'Sign in', visible: true },
      { id: 13, type: 'link', text: 'Hidden', visible: false },
    ];
    const { vm } = await pane({ devAction: { ok: true, data: { page: '# A', elements } } });
    await vm.run('analyze');
    assert.deepEqual(vm.state.elements, [{ id: '12', label: '#12 button Sign in' }]);
    assert.equal(vm.state.hint, 'Pick an element, then Click, Hover or Type.');
    vm.pickElement('12');
    assert.equal(vm.state.fields['action-click-id'], '12');
  });

  it('says when an analysis found nothing to act on', async () => {
    const { vm } = await pane({ devAction: { ok: true, data: { page: '', elements: [] } } });
    await vm.run('analyze');
    assert.equal(vm.state.hint, 'Nothing to act on in view.');
  });

  it('says Copied for a moment, or that copying failed', async () => {
    mock.timers.enable({ apis: ['setTimeout'] });
    const { vm } = await pane({}, { writeText: async () => Promise.reject(new Error('denied')) });
    await vm.copy();
    assert.equal(vm.state.copyLabel, 'Copy failed');
    mock.timers.tick(C.COPIED_MS);
    assert.equal(vm.state.copyLabel, 'Copy');
  });

  it('empties the result and the elements on the pane’s Clear', async () => {
    const { panel, vm } = await pane({
      devAction: { ok: true, data: { page: '# A', elements: [{ id: 1, type: 'a' }] } },
    });
    await vm.run('analyze');
    panel.clears.get('actions')?.();
    assert.deepEqual([vm.state.result, vm.state.elements], [null, []]);
  });
});
