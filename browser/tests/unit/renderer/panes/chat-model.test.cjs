/**
 * Unit tests for the Ask pane's setup cards (renderer/panes/chat-model.js):
 * signed out shows "Sign in", a project with no model asks for a key, and a
 * question refused for want of a key is sent again once one is saved.
 */
const { describe, it, afterEach } = require('node:test');
const assert = require('node:assert/strict');
const { loadRenderer, settle } = require('../../support/renderer-harness.cjs');

/** Every renderer a test loaded, so its step-line ticker can be stopped after. */
const loaded = [];
afterEach(() => loaded.splice(0).forEach((app) => app.run('ChatProgress.stop()')));

/** A loaded renderer whose modelStatus is `status`, with any further `answers`. */
async function withStatus(status, answers = {}) {
  const app = loadRenderer({ answers: { modelStatus: status, ...answers } });
  loaded.push(app);
  await settle();
  return app;
}

/** Types `key` into the model card and saves it. */
async function saveKey(app, key) {
  app.$('chat-model-key').value = key;
  app.fire(app.$('chat-model'), 'submit');
  await settle();
}

describe('the Ask setup cards', () => {
  it('asks a signed-out browser to sign in, and signs in through the console', async () => {
    const app = await withStatus({ signedIn: false, hasLlmKey: true });
    assert.equal(app.$('chat-signin').hidden, false);
    assert.equal(app.$('chat-model').hidden, true);
    assert.equal(app.$('chat-model-open').hidden, true);
    app.fire(app.$('chat-signin-button'), 'click');
    assert.equal(app.bridge.called('openConsole').length, 1);
  });

  it('asks for a model key as soon as Ask opens on a project without one', async () => {
    const app = await withStatus({ signedIn: true, hasLlmKey: false });
    assert.equal(app.$('chat-signin').hidden, true);
    assert.equal(app.$('chat-model').hidden, false);
  });

  it('shows no card once the project has a model', async () => {
    const app = await withStatus({ signedIn: true, hasLlmKey: true });
    assert.equal(app.$('chat-model').hidden, true);
    assert.equal(app.$('chat-signin').hidden, true);
  });

  it('sends a question refused for want of a key again once the key is saved', async () => {
    const answers = [{ error: 'No LLM key', code: 'llm_unconfigured' }, { text: 'Done' }];
    const app = await withStatus(
      { signedIn: true, hasLlmKey: true },
      { sendChat: () => answers.shift(), saveModelKey: { ok: true } },
    );
    app.$('chat-input').value = 'find flights';
    app.fire(app.$('chat-send'), 'click');
    await settle();
    assert.equal(app.$('chat-model').hidden, false);
    assert.equal(app.$('chat-messages').querySelectorAll('.chat-msg.error').length, 0);
    await saveKey(app, 'sk-ant-1');
    assert.deepEqual(app.bridge.called('saveModelKey')[0], ['anthropic', 'sk-ant-1']);
    assert.equal(app.bridge.called('sendChat').length, 2);
    assert.equal(app.$('chat-model').hidden, true);
  });

  it('reopens the card with the reason when the provider refuses the saved key or model', async () => {
    const refused = { error: 'Your AI provider refused the request (401).', code: 'llm_rejected' };
    const app = await withStatus({ signedIn: true, hasLlmKey: true }, { sendChat: refused });
    app.$('chat-input').value = 'find flights';
    app.fire(app.$('chat-send'), 'click');
    await settle();
    assert.equal(app.$('chat-model').hidden, false);
    assert.equal(app.$('chat-model-error').textContent, refused.error);
    assert.equal(app.$('chat-messages').querySelectorAll('.chat-msg.error').length, 0);
  });

  it('keeps the card open with the reason when the key is refused', async () => {
    const app = await withStatus({ signedIn: true, hasLlmKey: false }, { saveModelKey: { error: 'Bad key' } });
    await saveKey(app, 'nope');
    assert.equal(app.$('chat-model').hidden, false);
    assert.equal(app.$('chat-model-error').textContent, 'Bad key');
    assert.equal(app.bridge.called('sendChat').length, 0);
  });

  it('lets a working key be changed from the Model button, with Cancel', async () => {
    const app = await withStatus({ signedIn: true, hasLlmKey: true });
    app.fire(app.$('chat-model-open'), 'click');
    assert.equal(app.$('chat-model').hidden, false);
    app.fire(app.$('chat-model-cancel'), 'click');
    assert.equal(app.$('chat-model').hidden, true);
  });
});
