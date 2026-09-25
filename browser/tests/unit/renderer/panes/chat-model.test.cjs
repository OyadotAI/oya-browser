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

/** A value from the renderer's own realm, as plain data this realm can compare. */
const plain = (value) => JSON.parse(JSON.stringify(value));

/** A project on OpenRouter's Claude Sonnet 5, with models from two vendors. */
const ON_OPENROUTER = {
  ...ON_OPENAI,
  provider: 'openrouter',
  model: 'anthropic/claude-sonnet-5',
  catalog: ON_OPENAI.catalog.map((p) =>
    p.id === 'openrouter'
      ? {
          ...p,
          models: [
            { id: 'anthropic/claude-sonnet-5', label: 'Anthropic: Claude Sonnet 5' },
            { id: 'anthropic/claude-opus-5.5', label: 'Anthropic: Claude Opus 5.5' },
            { id: 'x-ai/grok-4.7', label: 'xAI: Grok 4.7' },
          ],
        }
      : p,
  ),
};

/** `status` with OpenRouter offering only `models`. */
const withModels = (status, models) => ({
  ...status,
  catalog: status.catalog.map((p) => (p.id === 'openrouter' ? { ...p, models } : p)),
});

/** The provider chip for `id`. */
const chip = (app, id) => app.$('chat-model-provider').querySelector(`[data-provider="${id}"]`);

/** The provider whose chip is checked. */
const checkedProvider = (app) => app.$('chat-model-provider').querySelector('[aria-checked="true"]')?.dataset.provider;

/** Opens the model list and types `query` into its search. */
function pickModel(app, query) {
  app.fire(app.$('chat-model-model'), 'click');
  app.$('chat-model-search').value = query;
  app.fire(app.$('chat-model-search'), 'input');
}

/** The ids the list offers now, a typed id included. */
const optionIds = (app) => [...app.$('chat-model-list').querySelectorAll('[role="option"]')].map((o) => o.dataset.id);

/** Opens the card from the Model button, which re-reads the server first. */
async function openCard(app) {
  app.fire(app.$('chat-model-open'), 'click');
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
    assert.deepEqual(plain(app.bridge.called('saveModelKey')[0]), [{ provider: '', model: '', key: 'sk-ant-1' }]);
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
    await openCard(app);
    assert.equal(app.$('chat-model').hidden, false);
    app.fire(app.$('chat-model-cancel'), 'click');
    assert.equal(app.$('chat-model').hidden, true);
  });

  it('opens on the provider and model the server runs on, not a default', async () => {
    const app = await withStatus(ON_OPENAI);
    await openCard(app);
    assert.equal(checkedProvider(app), 'openai');
    assert.equal(app.$('chat-model-name').textContent, 'GPT-4.1');
    assert.equal(app.$('chat-model-title').textContent, 'AI model');
    assert.match(app.$('chat-model-key-help').textContent, /saved key is kept/);
  });

  it('saves a model picked from the list without a key, on the provider it stays on', async () => {
    const app = await withStatus(ON_OPENAI, { saveModelKey: { ok: true } });
    await openCard(app);
    pickModel(app, 'gpt-4o');
    app.key(app.$('chat-model-search'), 'Enter');
    await saveKey(app, '');
    assert.deepEqual(plain(app.bridge.called('saveModelKey')[0]), [
      { provider: 'openai', model: 'gpt-4o-mini', key: '' },
    ]);
  });

  it('offers a changed provider’s models on its default, and asks for its key', async () => {
    const app = await withStatus(ON_OPENAI);
    await openCard(app);
    app.fire(chip(app, 'openrouter'), 'click', { bubbles: true });
    assert.equal(checkedProvider(app), 'openrouter');
    assert.equal(app.$('chat-model-name').textContent, 'Claude Sonnet 5');
    assert.equal(app.$('chat-model-key').placeholder, 'sk-or-...');
  });

  it('shows a model the list does not have as a custom id', async () => {
    const app = await withStatus({ ...ON_OPENAI, model: 'ft:gpt-4.1:acme' });
    await openCard(app);
    assert.equal(app.$('chat-model-name').textContent, 'ft:gpt-4.1:acme');
    assert.equal(app.$('chat-model-id').textContent, 'Custom model id');
  });

  it('follows a change made in the console while the card is closed', async () => {
    let status = ON_OPENAI;
    const app = await withStatus(() => status);
    status = { ...ON_OPENAI, model: 'gpt-4o-mini' };
    app.bridge.emit('SettingsChanged', { type: 'settings_changed', scope: 'llm' });
    await settle();
    await openCard(app);
    assert.equal(app.$('chat-model-name').textContent, 'GPT-4o mini');
  });
});

describe('the model search', () => {
  it('matches every word typed against name and id, and marks the matches', async () => {
    const app = await withStatus(ON_OPENROUTER);
    await openCard(app);
    pickModel(app, 'claude son');
    assert.deepEqual(optionIds(app), ['anthropic/claude-sonnet-5', 'claude son']);
    assert.deepEqual(
      [...app.$('chat-model-list').querySelectorAll('mark')].map((m) => m.textContent),
      ['Claude', 'Son'],
    );
  });

  it('groups OpenRouter’s models by vendor, and says how many there are', async () => {
    const app = await withStatus(ON_OPENROUTER);
    await openCard(app);
    app.fire(app.$('chat-model-model'), 'click');
    const groups = [...app.$('chat-model-list').querySelectorAll('.model-group')].map((g) => g.textContent);
    assert.deepEqual(groups, ['Anthropic', 'xAI']);
    assert.equal(app.$('chat-model-count').textContent, '3 models');
  });

  it('moves with the arrow keys and chooses with Enter, wrapping at the ends', async () => {
    const app = await withStatus(ON_OPENROUTER);
    await openCard(app);
    app.fire(app.$('chat-model-model'), 'click');
    app.key(app.$('chat-model-search'), 'ArrowUp');
    app.key(app.$('chat-model-search'), 'Enter');
    assert.equal(app.$('chat-model-name').textContent, 'Grok 4.7');
    assert.equal(app.$('chat-model-menu').hidden, true);
  });

  it('uses an id the list lacks, as typed', async () => {
    const app = await withStatus(ON_OPENROUTER, { saveModelKey: { ok: true } });
    await openCard(app);
    pickModel(app, 'meta-llama/llama-5');
    app.key(app.$('chat-model-search'), 'Enter');
    assert.equal(app.$('chat-model-name').textContent, 'meta-llama/llama-5');
    await saveKey(app, '');
    assert.equal(plain(app.bridge.called('saveModelKey')[0])[0].model, 'meta-llama/llama-5');
  });

  it('closes on Escape without changing the model', async () => {
    const app = await withStatus(ON_OPENROUTER);
    await openCard(app);
    pickModel(app, 'grok');
    app.key(app.$('chat-model-search'), 'Escape');
    assert.equal(app.$('chat-model-menu').hidden, true);
    assert.equal(app.$('chat-model-name').textContent, 'Claude Sonnet 5');
  });

  it('shows a model name from the server as text, never as markup', async () => {
    const hostile = { id: 'x/evil', label: 'Evil: <img src=x onerror=alert(1)>' };
    const app = await withStatus(withModels(ON_OPENROUTER, [hostile]));
    await openCard(app);
    app.fire(app.$('chat-model-model'), 'click');
    assert.equal(app.$('chat-model-list').querySelectorAll('img').length, 0);
    assert.match(app.$('chat-model-list').textContent, /<img src=x/);
  });
});
