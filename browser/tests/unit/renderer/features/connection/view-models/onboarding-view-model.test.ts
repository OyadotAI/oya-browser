/** First-run paths advance only on verified outcomes and ignore canceled provider results. */
import { it } from 'node:test';
import assert from 'node:assert/strict';
import { build, settle } from '../harness.ts';

it('starts with three-path selection and holds setup before saving manual connection settings', async () => {
  const app = build({ getConfig: { serverUrl: 'wss://oya.test/ws' } });
  await settle();
  assert.equal(app.setup.onboarding.state.page, 'choices');
  app.setup.onboarding.choose('technical');
  app.setup.edit('apiKey', 'synthetic-workspace-key');
  await app.setup.connect();
  const methods = app.fake.calls.map(([method]) => method);
  assert(methods.indexOf('beginOnboarding') < methods.indexOf('saveConfig'));
  assert.equal(app.setup.onboarding.state.page, 'technical');
  app.fake.emit('onWsStatus', { connected: true });
  assert.equal(app.setup.onboarding.state.page, 'connected');
  app.setup.dispose();
});
it('a provider result alone cannot claim an authenticated browser connection', async () => {
  const app = build({ connectChatgpt: { ok: true }, onboardingStatus: { connected: false } });
  app.setup.onboarding.choose('nontechnical');
  await app.setup.onboarding.connectChatgpt();
  assert.equal(app.setup.onboarding.state.page, 'waiting');
  assert.deepEqual(app.fake.called('connectChatgpt'), [[{ importLogins: false }]]);
  app.fake.emit('onWsStatus', { connected: true });
  assert.equal(app.setup.onboarding.state.page, 'connected');
  assert.equal(app.setup.onboarding.state.chatgptReady, true);
  app.setup.dispose();
});
it('late ChatGPT completion cannot advance a newly selected path', async () => {
  const reply = Promise.withResolvers<object>();
  const app = build({ connectChatgpt: () => reply.promise, onboardingStatus: { connected: true } });
  await settle();
  app.setup.onboarding.choose('nontechnical');
  const pending = app.setup.onboarding.connectChatgpt();
  await settle();
  app.setup.onboarding.choose('technical');
  reply.resolve({ ok: true });
  await pending;
  app.fake.emit('onWsStatus', { connected: true });
  assert.equal(app.setup.onboarding.state.page, 'technical');
  assert.equal(app.setup.onboarding.state.chatgptReady, false);
  app.setup.dispose();
});
it('changing paths before the pairing hold completes prevents opening the old console', async () => {
  const begin = Promise.withResolvers<void>();
  const app = build({ beginOnboarding: () => begin.promise, openConsole: 'https://oya.test/dashboard' });
  await settle();
  app.setup.onboarding.choose('expert');
  const pending = app.setup.signIn();
  app.setup.onboarding.choose('technical');
  begin.resolve();
  await pending;
  assert.equal(app.fake.called('openConsole').length, 0);
  assert.equal(app.setup.onboarding.state.page, 'technical');
  app.setup.dispose();
});
it('a late console-open result cannot replace a newly selected path', async () => {
  const consoleResult = Promise.withResolvers<string>();
  const app = build({ openConsole: () => consoleResult.promise });
  await settle();
  app.setup.onboarding.choose('expert');
  const pending = app.setup.signIn();
  await settle();
  app.setup.onboarding.choose('technical');
  consoleResult.resolve('https://oya.test/dashboard');
  await pending;
  assert.equal(app.setup.state.view, 'checklist');
  assert.equal(app.setup.onboarding.state.page, 'technical');
  app.setup.dispose();
});
it('a refused provider does not complete on an unrelated socket event', async () => {
  const app = build({ connectChatgpt: { error: 'Provider refused this sign-in.' } });
  app.setup.onboarding.choose('nontechnical');
  await app.setup.onboarding.connectChatgpt();
  app.fake.emit('onWsStatus', { connected: true });
  assert.equal(app.setup.onboarding.state.page, 'waiting');
  assert.match(app.setup.onboarding.state.error, /refused/);
  app.setup.dispose();
});
it('disconnect preserves walkthrough position and explicit browse releases setup', async () => {
  const app = build();
  app.setup.onboarding.choose('expert');
  app.setup.onboarding.show('guide');
  app.setup.onboarding.move(1);
  app.fake.emit('onWsStatus', { connected: false });
  assert.deepEqual([app.setup.onboarding.state.page, app.setup.onboarding.state.step], ['guide', 1]);
  await app.setup.reconnect();
  app.fake.emit('onWsStatus', { connected: true });
  assert.deepEqual([app.setup.onboarding.state.page, app.setup.onboarding.state.step], ['guide', 1]);
  app.setup.skip();
  assert.equal(app.fake.called('finishOnboarding').length, 1);
  app.setup.dispose();
});
it('reconnecting a paired browser uses main-owned settings without replacing its key', async () => {
  const app = build();
  await settle();
  app.setup.onboarding.choose('expert');
  await app.setup.reconnect();
  assert.deepEqual(app.fake.called('saveConfig'), [[{}]]);
  app.fake.emit('onWsStatus', { connected: true });
  assert.equal(app.setup.onboarding.state.page, 'connected');
  app.setup.dispose();
});
