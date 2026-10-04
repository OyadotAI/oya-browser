/**
 * Unit tests for the Ask pane's setup cards: signed out shows "Sign in", a
 * project with no model asks for a key, the card opens on what the server
 * runs, and saving sends a waiting question again.
 */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  ModelSetupViewModel,
  keyHint,
} from '../../../../../../src/renderer/features/ask/view-models/model-setup-view-model.ts';
import { triggerText } from '../../../../../../src/renderer/features/ask/model/model-search.ts';
import { fakeBridge } from '../../../support/bridge.ts';
import { settle } from '../support.ts';

/** A project on OpenAI's gpt-4.1, offered OpenAI and OpenRouter. */
const ON_OPENAI = {
  signedIn: true,
  hasLlmKey: true,
  provider: 'openai',
  model: 'gpt-4.1',
  catalog: [
    {
      id: 'openai',
      label: 'OpenAI',
      hint: 'sk-...',
      keysUrl: 'https://platform.openai.com/api-keys',
      model: 'gpt-4o-mini',
      models: [
        { id: 'gpt-4o-mini', label: 'GPT-4o mini' },
        { id: 'gpt-4.1', label: 'GPT-4.1' },
      ],
    },
    {
      id: 'openrouter',
      label: 'OpenRouter',
      hint: 'sk-or-...',
      keysUrl: 'https://openrouter.ai/keys',
      model: 'anthropic/claude-sonnet-5',
      models: [{ id: 'anthropic/claude-sonnet-5', label: 'Claude Sonnet 5' }],
    },
  ],
};

/** Setup cards over a fake bridge whose modelStatus is `status`, once the first read is in. */
async function setup(status: unknown, answers: Record<string, unknown> = {}) {
  const fake = fakeBridge({ modelStatus: status, ...answers });
  const resent: number[] = [];
  const vm = new ModelSetupViewModel(fake.bridge, () => resent.push(1));
  await settle();
  return { fake, vm, resent };
}

/** The model the picker's button names. */
const modelName = (vm: ModelSetupViewModel) => triggerText(vm.picker.state.models, vm.picker.state.value).name;

describe('ModelSetupViewModel', () => {
  it('asks a signed-out browser to sign in, and signs in through the console', async () => {
    const { fake, vm } = await setup({ signedIn: false, hasLlmKey: true });
    assert.equal(vm.state.signedIn, false);
    assert.equal(vm.state.open, false);
    vm.signIn();
    assert.equal(fake.called('openConsole').length, 1);
  });

  it('opens no card while signed out, even when a question needs a key', async () => {
    const { vm } = await setup({ signedIn: false });
    vm.needed();
    assert.equal(vm.state.open, false);
  });

  it('asks for a model key as soon as Ask opens on a project without one', async () => {
    const { vm } = await setup({ signedIn: true, hasLlmKey: false });
    assert.equal(vm.state.open, true);
    assert.equal(vm.state.optional, false);
  });

  it('shows no card once the project has a model', async () => {
    const { vm } = await setup({ signedIn: true, hasLlmKey: true });
    assert.deepEqual([vm.state.signedIn, vm.state.open], [true, false]);
  });

  it('counts an unreadable status as signed in with a key, so nothing nags', async () => {
    const { vm } = await setup(() => Promise.reject(new Error('offline')));
    assert.deepEqual([vm.state.signedIn, vm.state.open], [true, false]);
  });

  it('closes the card it forced open once the key is set elsewhere', async () => {
    let status: unknown = { signedIn: true, hasLlmKey: false };
    const { fake, vm } = await setup(() => status);
    status = { signedIn: true, hasLlmKey: true };
    fake.emit('onWsStatus', { connected: true });
    await settle();
    assert.equal(vm.state.open, false);
  });

  it('sends a waiting question again once the key is saved, and only once', async () => {
    const { fake, vm, resent } = await setup({ signedIn: true, hasLlmKey: true }, { saveModelKey: { ok: true } });
    vm.needed();
    vm.setKey('sk-ant-1');
    await vm.save();
    assert.deepEqual(fake.called('saveModelKey')[0], [{ provider: '', model: '', key: 'sk-ant-1' }]);
    assert.equal(resent.length, 1);
    assert.deepEqual([vm.state.open, vm.state.key], [false, '']);
    await vm.save();
    assert.equal(resent.length, 1);
  });

  it('shows the provider’s refusal on the card it reopens', async () => {
    const { vm } = await setup({ signedIn: true, hasLlmKey: true });
    vm.needed('Your AI provider refused the request (401).');
    assert.equal(vm.state.open, true);
    assert.equal(vm.state.error, 'Your AI provider refused the request (401).');
  });

  it('keeps the card open with the reason when the key is refused', async () => {
    const { vm, resent } = await setup({ signedIn: true, hasLlmKey: false }, { saveModelKey: { error: 'Bad key' } });
    vm.needed();
    await vm.save();
    assert.deepEqual([vm.state.open, vm.state.error, resent.length], [true, 'Bad key', 0]);
  });

  it('keeps the card open with the reason when saving fails outright', async () => {
    const { vm } = await setup(
      { signedIn: true, hasLlmKey: false },
      { saveModelKey: () => Promise.reject(new Error('Not connected to server')) },
    );
    await vm.save();
    assert.equal(vm.state.error, 'Not connected to server');
  });

  it('lets a working key be changed from the Model button, with Cancel', async () => {
    const { vm } = await setup({ signedIn: true, hasLlmKey: true });
    await vm.openCard();
    assert.deepEqual([vm.state.open, vm.state.optional], [true, true]);
    vm.hide();
    assert.equal(vm.state.open, false);
  });

  it('opens on the provider and model the server runs on, not a default', async () => {
    const { vm } = await setup(ON_OPENAI);
    await vm.openCard();
    assert.equal(vm.state.picked, 'openai');
    assert.equal(modelName(vm), 'GPT-4.1');
    assert.match(keyHint(vm.state.status, vm.entry).help, /saved key is kept/);
  });

  it('saves a model picked from the list without a key, on the provider it stays on', async () => {
    const { fake, vm } = await setup(ON_OPENAI, { saveModelKey: { ok: true } });
    await vm.openCard();
    vm.picker.openList();
    vm.picker.search('gpt-4o');
    vm.picker.key('Enter');
    await vm.save();
    assert.deepEqual(fake.called('saveModelKey')[0], [{ provider: 'openai', model: 'gpt-4o-mini', key: '' }]);
  });

  it('offers a changed provider’s models on its default, and asks for its key', async () => {
    const { vm } = await setup(ON_OPENAI);
    await vm.openCard();
    vm.choose('openrouter');
    assert.equal(vm.state.picked, 'openrouter');
    assert.equal(modelName(vm), 'Claude Sonnet 5');
    assert.equal(keyHint(vm.state.status, vm.entry).placeholder, 'sk-or-...');
  });

  it('keeps the model chosen when the picked provider’s chip is pressed again', async () => {
    const { vm } = await setup(ON_OPENAI);
    await vm.openCard();
    vm.choose('openai');
    assert.equal(vm.picker.state.value, 'gpt-4.1');
  });

  it('shows a model the list does not have as a custom id', async () => {
    const { vm } = await setup({ ...ON_OPENAI, model: 'ft:gpt-4.1:acme' });
    await vm.openCard();
    const { name, detail } = triggerText(vm.picker.state.models, vm.picker.state.value);
    assert.deepEqual([name, detail], ['ft:gpt-4.1:acme', 'Custom model id']);
  });

  it('follows a change made in the console while the card is closed', async () => {
    let status = ON_OPENAI;
    const { fake, vm } = await setup(() => status);
    status = { ...ON_OPENAI, model: 'gpt-4o-mini' };
    fake.emit('onSettingsChanged', { type: 'settings_changed', scope: 'llm' });
    await settle();
    await vm.openCard();
    assert.equal(modelName(vm), 'GPT-4o mini');
  });

  it('leaves the card alone when the console changes it while the person is editing', async () => {
    let status = ON_OPENAI;
    const { fake, vm } = await setup(() => status);
    await vm.openCard();
    vm.choose('openrouter');
    status = { ...ON_OPENAI, model: 'gpt-4o-mini' };
    fake.emit('onSettingsChanged', {});
    await settle();
    assert.equal(vm.state.picked, 'openrouter');
  });

  it('opens the picked provider’s key page in a new tab', async () => {
    const { fake, vm } = await setup(ON_OPENAI);
    await vm.openCard();
    vm.getKey();
    assert.deepEqual(fake.called('newTab'), [['https://platform.openai.com/api-keys']]);
  });
});
