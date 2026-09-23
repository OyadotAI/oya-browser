/**
 * The Ask pane's setup cards, so Ask works without a trip to the web console:
 * "Sign in" while this browser has no project, and "Connect an AI model" while
 * the project has no model key. A question asked before the key was saved is
 * sent again once it is.
 */
/* global oyaBrowser, Dom, Chat */
/* exported ChatModel */

/** Each provider's key format and where to get a key. */
const CHAT_MODEL_PROVIDERS = {
  anthropic: { hint: 'sk-ant-...', keys: 'https://console.anthropic.com/settings/keys' },
  openai: { hint: 'sk-...', keys: 'https://platform.openai.com/api-keys' },
  gemini: { hint: 'AIza...', keys: 'https://aistudio.google.com/apikey' },
};

/** The setup cards. */
const ChatModel = {
  /** A question is waiting for a key, and goes again once one is saved. */
  pending: false,

  /** Shows whichever card this browser needs, if any. */
  async refresh() {
    const status = await oyaBrowser.modelStatus().catch(() => ({ signedIn: true, hasLlmKey: true }));
    Dom.byId('chat-signin').hidden = status.signedIn;
    Dom.byId('chat-model-open').hidden = !status.signedIn;
    if (!status.signedIn) Dom.byId('chat-model').hidden = true;
    else if (!status.hasLlmKey) ChatModel.show(false);
    else if (Dom.byId('chat-model-cancel').hidden) Dom.byId('chat-model').hidden = true;
  },

  /** The server refused a question for want of a working key (`reason` when the provider refused it): ask for one, then send it again. */
  needed(reason) {
    ChatModel.pending = true;
    Dom.byId('chat-messages').querySelector('.chat-thinking')?.remove();
    ChatModel.show(false);
    if (reason) ChatModel.error(reason);
  },

  /** Shows the model card (not while signed out); `optional` offers Cancel, for changing a key that works. */
  show(optional) {
    const card = Dom.byId('chat-model');
    if (!Dom.byId('chat-signin').hidden || !card.hidden) return;
    card.hidden = false;
    Dom.byId('chat-model-cancel').hidden = !optional;
    ChatModel.error('');
    Dom.byId('chat-model-key').focus();
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

  /** Saves the key to the project, then sends a waiting question again. */
  async save(e) {
    e.preventDefault();
    const provider = Dom.byId('chat-model-provider').value;
    const saved = await oyaBrowser.saveModelKey(provider, Dom.byId('chat-model-key').value);
    if (saved?.error) return ChatModel.error(saved.error);
    ChatModel.hide();
    if (ChatModel.pending) Chat.turn();
    ChatModel.pending = false;
  },

  /** The provider changed: its key format as the placeholder. */
  provider() {
    Dom.byId('chat-model-key').placeholder = CHAT_MODEL_PROVIDERS[Dom.byId('chat-model-provider').value].hint;
  },

  /** Opens the chosen provider's key page in a new tab. */
  getKey() {
    oyaBrowser.newTab(CHAT_MODEL_PROVIDERS[Dom.byId('chat-model-provider').value].keys);
  },
};

Dom.byId('chat-model').addEventListener('submit', ChatModel.save);
Dom.byId('chat-model-provider').addEventListener('change', ChatModel.provider);
Dom.byId('chat-model-get').addEventListener('click', ChatModel.getKey);
Dom.byId('chat-model-cancel').addEventListener('click', ChatModel.hide);
Dom.byId('chat-model-open').addEventListener('click', () => ChatModel.show(true));
Dom.byId('chat-signin-button').addEventListener('click', () => oyaBrowser.openConsole());
oyaBrowser.onWsStatus(() => ChatModel.refresh());
ChatModel.refresh();
