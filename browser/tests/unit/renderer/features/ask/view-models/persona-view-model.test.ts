/**
 * Unit tests for the Ask pane's profile picker: it lists the project's
 * personas with the one in use chosen, switches between them, and is locked
 * offline, while switching, and while the agent works.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  PersonaViewModel,
  personaLocked,
} from '../../../../../../src/renderer/features/ask/view-models/persona-view-model.ts';
import { fakeBridge } from '../../../support/bridge.ts';
import { settle } from '../support.ts';

/** The project's personas. */
const personas = [
  { id: 'd', name: 'Default', isDefault: true },
  { id: 'p-work', name: 'Work', isDefault: false },
];

/** A picker over a fake bridge listing `list`, once its first read is in. */
async function picker(list?: unknown) {
  const fake = fakeBridge({ listPersonas: list });
  const vm = new PersonaViewModel(fake.bridge);
  await settle();
  return { fake, vm };
}

describe('PersonaViewModel', () => {
  it('fills itself at load, for a window that opened after the browser connected', async () => {
    const { vm } = await picker({ personas, active: 'default' });
    assert.equal(vm.state.options.length, 2);
    assert.equal(vm.state.ready, true);
  });

  it('is locked, offering only the default, until the browser is connected', async () => {
    const { vm } = await picker();
    assert.deepEqual(vm.state.options, [{ value: 'default', label: 'Default profile' }]);
    assert.equal(vm.state.ready, false);
  });

  it('stays locked when the list cannot be read', async () => {
    const { vm } = await picker(() => Promise.reject(new Error('offline')));
    assert.equal(vm.state.ready, false);
  });

  it('lists the named personas once connected, with the one in use chosen', async () => {
    const server = { list: undefined as unknown };
    const { fake, vm } = await picker(() => server.list);
    server.list = { personas, active: 'p-work' };
    fake.emit('onWsStatus', { connected: true });
    await settle();
    assert.deepEqual(vm.state.options, [
      { value: 'default', label: 'Default profile' },
      { value: 'p-work', label: 'Work' },
    ]);
    assert.equal(vm.state.active, 'p-work');
    assert.equal(vm.state.ready, true);
  });

  it('reconnects as the chosen persona, locked until the switch lands', async () => {
    const { fake, vm } = await picker({ personas, active: 'default' });
    vm.change('p-work');
    assert.deepEqual(fake.called('saveConfig').at(-1), [{ persona: 'p-work' }]);
    assert.equal(vm.state.ready, false);
    fake.emit('onFingerprintChanged', {});
    await settle();
    assert.equal(vm.state.ready, true);
  });

  it('locks once offline', async () => {
    const { fake, vm } = await picker({ personas, active: 'default' });
    fake.emit('onWsStatus', { connected: false });
    assert.equal(vm.state.ready, false);
  });

  it('cannot be changed while the agent works', () => {
    assert.equal(personaLocked(true, true), true);
    assert.equal(personaLocked(true, false), false);
    assert.equal(personaLocked(false, false), true);
  });
});
