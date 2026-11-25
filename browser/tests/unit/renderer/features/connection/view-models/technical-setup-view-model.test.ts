/** Direct Technical login exits only after authenticated readiness and never resumes a cancelled route. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { setImmediate as turn } from 'node:timers/promises';
import { fakeBridge } from '../../../support/bridge.ts';
import { TechnicalSetupViewModel } from '../../../../../../src/renderer/features/connection/view-models/technical-setup-view-model.ts';

it('a direct successful login enters browsing without an extra continuation screen', async () => {
  let connected = false;
  const fake = fakeBridge({
    localAgents: [],
    onboardingStatus: () => ({ connected, signedIn: false }),
    startDesktopSignIn: () => {
      connected = true;
      return { ok: true };
    },
  });
  const vm = new TechnicalSetupViewModel(fake.bridge);
  await vm.signIn();
  assert.equal(fake.called('finishOnboarding').length, 1);
  assert.equal(fake.called('openConsole').length, 0);
  vm.dispose();
});
it('leaving a pending sign-in cannot open the browser from its late reply', async () => {
  const auth = Promise.withResolvers<object>();
  const fake = fakeBridge({
    localAgents: [],
    onboardingStatus: { connected: false },
    startDesktopSignIn: () => auth.promise,
  });
  const vm = new TechnicalSetupViewModel(fake.bridge),
    pending = vm.signIn();
  await turn();
  vm.show('agents');
  auth.resolve({ ok: true });
  await pending;
  assert.equal(vm.state.view, 'agents');
  assert.equal(fake.called('finishOnboarding').length, 0);
  vm.dispose();
});
it('a saved workspace reconnects without another login', async () => {
  let connected = false;
  const fake = fakeBridge({
    localAgents: [],
    onboardingStatus: () => ({ connected, signedIn: true }),
    saveConfig: () => {
      connected = true;
      return true;
    },
  });
  const vm = new TechnicalSetupViewModel(fake.bridge);
  await vm.signIn();
  assert.deepEqual(fake.called('saveConfig'), [[{}]]);
  assert.equal(fake.called('startDesktopSignIn').length, 0);
  assert.equal(fake.called('finishOnboarding').length, 1);
  vm.dispose();
});
