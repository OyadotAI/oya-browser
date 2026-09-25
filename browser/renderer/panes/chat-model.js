/**
 * The Ask pane's setup cards, so Ask works without a trip to the web console:
 * "Sign in" while this browser has no project, and "Connect an AI model" to
 * pick the project's provider, model and key. The card always shows what the
 * server runs on: it is built from the server's catalog, re-read whenever it
 * opens, and again when the server says the settings changed elsewhere. A
 * question asked before a model was set is sent again once it is.
 */
/* global oyaBrowser, Dom, Chat, ModelPicker */
/* exported ChatModel */

/** The setup cards. */
const ChatModel = {
  /** A question is waiting for a model, and goes again once one is saved. */
  pending: false,

  /** What the server said last: its provider catalog, the provider and model in use, and whether a key is set. */
  status: { catalog: [], provider: '', model: '', hasLlmKey: true },

  /** Reads the project's model from the server; unsure counts as signed in with a key. */
  async load() {
    const status = await oyaBrowser.modelStatus().catch(() => ({ signedIn: true, hasLlmKey: true }));
    const { catalog = [], provider = '', model = '', hasLlmKey } = status;
    ChatModel.status = { catalog, provider, model, hasLlmKey };
    return status;
  },

  /** Shows whichever card this browser needs, if any, from what the server says now. */
  async refresh() {
    const status = await ChatModel.load();
    Dom.byId('chat-signin').hidden = status.signedIn;
    Dom.byId('chat-model-open').hidden = !status.signedIn;
    if (!status.signedIn) Dom.byId('chat-model').hidden = true;
    else if (!status.hasLlmKey) ChatModel.show(false);
    else if (Dom.byId('chat-model-cancel').hidden) Dom.byId('chat-model').hidden = true;
  },

  /** The settings changed elsewhere: re-read them, and redraw the card unless the person is editing it. */
  async changed() {
    const editing = !Dom.byId('chat-model').hidden;
    await ChatModel.refresh();
    if (!editing) ChatModel.render();
  },

  /** The "Model" button: re-read the server first, so the card opens on what it runs now. */
  async open() {
    await ChatModel.load();
    ChatModel.show(true);
  },

  /** The server refused a question for want of a working key (`reason` when the provider refused it): ask for one, then send it again. */
  needed(reason) {
    ChatModel.pending = true;
    Dom.byId('chat-messages').querySelector('.chat-thinking')?.remove();
    ChatModel.show(false);
    if (reason) ChatModel.error(reason);
  },

  /** Shows the model card on the server's current choice (not while signed out); `optional` offers Cancel. */
  show(optional) {
    const card = Dom.byId('chat-model');
    if (!Dom.byId('chat-signin').hidden || !card.hidden) return;
    ChatModel.render();
    card.hidden = false;
    Dom.byId('chat-model-cancel').hidden = !optional;
    ChatModel.error('');
    Dom.byId('chat-model-key').focus();
  },

  /** The provider picked in the card. */
  picked: '',

  /** Draws the card on the server's choice: its providers, then its models on the model in use. */
  render() {
    const { catalog, provider, model, hasLlmKey } = ChatModel.status;
    const known = catalog.some((p) => p.id === provider);
    Dom.byId('chat-model-title').textContent = hasLlmKey ? 'AI model' : 'Connect an AI model';
    Dom.byId('chat-model-provider').replaceChildren(...catalog.map(ChatModel.chip));
    ChatModel.pick(known ? provider : catalog[0]?.id || '', known ? model : '');
  },

  /** One provider as a chip: its initial, its name, and a tick when picked. */
  chip(p) {
    const chip = Dom.node('button', null, 'provider-chip');
    Object.assign(chip, { type: 'button' });
    chip.setAttribute('role', 'radio');
    chip.dataset.provider = p.id;
    chip.append(Dom.node('span', p.label.slice(0, 1), 'provider-mark'), Dom.node('span', p.label, 'provider-name'));
    return chip;
  },

  /** Picks provider `id` and offers its models, on `model` or else its default. */
  pick(id, model = '') {
    ChatModel.picked = id;
    const chips = Dom.byId('chat-model-provider').querySelectorAll('.provider-chip');
    chips.forEach((c) => c.setAttribute('aria-checked', String(c.dataset.provider === id)));
    const entry = ChatModel.entry();
    ModelPicker.set(entry?.models || [], model || entry?.model || '');
    ChatModel.keyHint();
  },

  /** The catalog entry for the provider picked. */
  entry() {
    return ChatModel.status.catalog.find((p) => p.id === ChatModel.picked);
  },

  /** Says whether the saved key carries over (same provider) or a new one is needed, and in what shape. */
  keyHint() {
    const entry = ChatModel.entry();
    const keeps = entry?.id === ChatModel.status.provider && ChatModel.status.hasLlmKey;
    Dom.byId('chat-model-key').placeholder = keeps ? '••••••••  saved' : entry?.hint || 'API key';
    const help = keeps
      ? 'Your saved key is kept. Paste a new one only to replace it.'
      : `Paste your ${entry?.label || ''} key.`;
    Dom.byId('chat-model-key-help').textContent = help;
  },

  /** A chip was clicked: pick its provider, on its default model. */
  provider(e) {
    const chip = e.target.closest('.provider-chip');
    if (chip && chip.dataset.provider !== ChatModel.picked) ChatModel.pick(chip.dataset.provider);
  },

  /** The model chosen: a listed one, or an id typed into the search. */
  chosenModel() {
    return ModelPicker.value.trim();
  },

  /** Hides the model card. */
  hide() {
    Dom.byId('chat-model').hidden = true;
    Dom.byId('chat-model-key').value = '';
  },

  /** Shows `message` on the card, or clears it. */
  error(message) {
    const note = Dom.byId('chat-model-error');
    note.textContent = message;
    note.hidden = !message;
  },

  /** Saves the provider, model and key to the project, then sends a waiting question again. */
  async save(e) {
    e.preventDefault();
    const choice = { provider: ChatModel.picked, model: ChatModel.chosenModel() };
    const saved = await oyaBrowser.saveModelKey({ ...choice, key: Dom.byId('chat-model-key').value });
    if (saved?.error) return ChatModel.error(saved.error);
    ChatModel.hide();
    await ChatModel.load();
    if (ChatModel.pending) Chat.turn();
    ChatModel.pending = false;
  },

  /** Opens the picked provider's key page in a new tab. */
  getKey() {
    const url = ChatModel.entry()?.keysUrl;
    if (url) oyaBrowser.newTab(url);
  },
};

Dom.byId('chat-model').addEventListener('submit', ChatModel.save);
Dom.byId('chat-model-provider').addEventListener('click', ChatModel.provider);
Dom.byId('chat-model-get').addEventListener('click', ChatModel.getKey);
Dom.byId('chat-model-cancel').addEventListener('click', ChatModel.hide);
Dom.byId('chat-model-open').addEventListener('click', () => ChatModel.open());
Dom.byId('chat-signin-button').addEventListener('click', () => oyaBrowser.openConsole());
oyaBrowser.onWsStatus(() => ChatModel.refresh());
oyaBrowser.onSettingsChanged(() => ChatModel.changed());
ChatModel.refresh();
