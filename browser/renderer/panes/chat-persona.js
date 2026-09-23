/**
 * The Ask pane's profile picker: the project's personas, with the one this
 * browser runs as selected. Choosing another reconnects as it; the server then
 * sends its device and cookies, and the tabs reopen in that persona's own jar.
 * Offline, or while the agent is working, it cannot be changed.
 */
/* global oyaBrowser, Dom, Chat */
/* exported ChatPersona */

/** The words the picker shows. */
const CHAT_PERSONA_TEXT = { default: 'Default profile' };

/** The profile picker. */
const ChatPersona = {
  /** Whether the list came from the server, so a choice can be made. */
  ready: false,

  /** Fills the picker from the server; offline, only the default shows and it is locked. */
  async load() {
    const { personas, active } = await oyaBrowser.listPersonas().catch(() => ({ personas: [], active: 'default' }));
    const named = personas.filter((p) => !p.isDefault).map((p) => ChatPersona.option(p.id, p.name));
    const select = Dom.byId('chat-persona');
    select.replaceChildren(ChatPersona.option('default', CHAT_PERSONA_TEXT.default), ...named);
    select.value = active;
    ChatPersona.ready = personas.length > 0;
    ChatPersona.sync();
  },

  /** One choice. */
  option(value, label) {
    const option = Dom.node('option', label);
    option.value = value;
    return option;
  },

  /** Locks the picker while offline or while a message is on its way. */
  sync() {
    Dom.byId('chat-persona').disabled = !ChatPersona.ready || Chat.sending;
  },

  /** The connection changed: connected loads the list, offline locks the picker. */
  status(status) {
    if (status.connected) return ChatPersona.load();
    ChatPersona.ready = false;
    ChatPersona.sync();
  },

  /** Reconnects as the chosen persona. */
  change() {
    ChatPersona.ready = false;
    ChatPersona.sync();
    oyaBrowser.saveConfig({ persona: Dom.byId('chat-persona').value });
  },
};

Dom.byId('chat-persona').addEventListener('change', ChatPersona.change);
oyaBrowser.onWsStatus(ChatPersona.status);
oyaBrowser.onFingerprintChanged(() => ChatPersona.load());
// A window that loads after the socket connected has missed that ws-status.
ChatPersona.load();
