/** Technical model setup preserves login drafts, uses catalog defaults and requires real diagnostic success. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { setImmediate as turn } from 'node:timers/promises';
import { fakeBridge } from '../../../support/bridge.ts';
import { TechnicalModelViewModel } from '../../../../../../src/renderer/features/connection/view-models/technical-model-view-model.ts';

const catalog = [
  { id: 'gemini', label: 'Gemini', model: 'catalog-gemini' },
  { id: 'anthropic', label: 'Claude', model: 'catalog-claude' },
  { id: 'openrouter', label: 'OpenRouter', model: 'catalog-router' },
];
/** All settings and provider replies are synthetic and never leave the fake bridge. */
function fixture(overrides: Record<string, unknown> = {}) {
  const fake = fakeBridge({
    onboardingStatus: { connected: true, signedIn: true },
    modelStatus: { catalog },
    saveModelKey: { ok: true },
    testModelKey: { ok: true },
    ...overrides,
  });
  return { fake, vm: new TechnicalModelViewModel(fake.bridge) };
}
it('no provider is preferred and each supported provider saves its own catalog default', async () => {
  for (const provider of ['gemini', 'anthropic', 'openrouter'] as const) {
    const { vm, fake } = fixture();
    assert.equal(vm.state.provider, '');
    vm.choose(provider);
    vm.edit(' synthetic-key ');
    await vm.connect();
    assert.deepEqual(fake.called('modelStatus'), [[true]]);
    assert.deepEqual(fake.called('saveModelKey'), [
      [{ provider, key: 'synthetic-key', model: catalog.find((p) => p.id === provider)!.model, baseUrl: '' }],
    ]);
    assert.deepEqual(fake.called('testModelKey'), [[provider]]);
    assert.equal(vm.state.ready, true);
    assert.equal(vm.state.key, '');
    vm.dispose();
  }
});
it('a successful save followed by a refused test remains saved but never ready', async () => {
  const { vm, fake } = fixture({ testModelKey: { error: 'Key refused', code: 'model_key_refused' } });
  vm.choose('gemini');
  vm.edit('synthetic-key');
  await vm.connect();
  assert.equal(vm.state.saved, true);
  assert.equal(vm.state.ready, false);
  assert.match(vm.state.error, /Key saved.*Test did not pass.*Key refused/);
  await vm.connect();
  assert.equal(fake.called('saveModelKey').length, 1);
  assert.equal(fake.called('testModelKey').length, 2);
  vm.edit('corrected-synthetic-key');
  await vm.connect();
  assert.equal(fake.called('saveModelKey').length, 2);
  vm.dispose();
});
it('a pending key resumes after correlated login without asking for an Oya workspace key', async () => {
  let connected = false;
  const auth = Promise.withResolvers<object>();
  const { vm, fake } = fixture({
    onboardingStatus: () => ({ connected, signedIn: false }),
    startDesktopSignIn: () => auth.promise,
  });
  vm.choose('anthropic');
  vm.edit('synthetic-api-key');
  const pending = vm.connect();
  await turn();
  assert.equal(vm.state.key, 'synthetic-api-key');
  assert.equal(fake.called('saveModelKey').length, 0);
  connected = true;
  auth.resolve({ ok: true });
  await pending;
  assert.equal(vm.state.ready, true);
  assert.equal(fake.called('startDesktopSignIn').length, 1);
  assert.equal(fake.called('beginTechnicalModel').length, 1);
  assert.equal(fake.called('openConsole').length, 0);
  vm.dispose();
});
it('changing provider during a late sign-in clears its key and prevents saving it', async () => {
  const auth = Promise.withResolvers<object>();
  const { vm, fake } = fixture({ onboardingStatus: { connected: false }, startDesktopSignIn: () => auth.promise });
  vm.choose('gemini');
  vm.edit('old-synthetic-key');
  const pending = vm.connect();
  await turn();
  vm.choose('openrouter');
  auth.resolve({ ok: true });
  await pending;
  assert.equal(vm.state.key, '');
  assert.equal(vm.state.provider, 'openrouter');
  assert.equal(vm.state.ready, false);
  assert.equal(fake.called('saveModelKey').length, 0);
  assert(fake.called('cancelOnboardingAttempt').length > 0);
  vm.dispose();
});
it('missing catalog support and empty diagnostic replies cannot claim readiness', async () => {
  const first = fixture({ modelStatus: { catalog: [] } });
  first.vm.choose('openrouter');
  first.vm.edit('synthetic-key');
  await first.vm.connect();
  assert.match(first.vm.state.error, /server needs an update/);
  assert.equal(first.fake.called('saveModelKey').length, 0);
  first.vm.dispose();
  const second = fixture({ testModelKey: {} });
  second.vm.choose('gemini');
  second.vm.edit('synthetic-key');
  await second.vm.connect();
  assert.equal(second.vm.state.saved, true);
  assert.equal(second.vm.state.ready, false);
  second.vm.dispose();
});
it('a strict catalog error is shown before an older server can save a fallback model', async () => {
  const { vm, fake } = fixture({
    modelStatus: { error: 'This server needs an update to support model setup.', catalog },
  });
  vm.choose('gemini');
  vm.edit('synthetic-key');
  await vm.connect();
  assert.match(vm.state.error, /server needs an update/);
  assert.equal(fake.called('saveModelKey').length, 0);
  assert.equal(fake.called('testModelKey').length, 0);
  vm.dispose();
});
