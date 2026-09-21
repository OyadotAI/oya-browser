/**
 * The connection settings dialog, opened from the profile dialog while
 * browsing: edit the server, key and name, then save and reconnect.
 */
/* global oyaBrowser, Dom, ShellState, RendererConstants, Setup, Dialogs, ShellDialog */
/* exported Reconnect */

/** The reconnect dialog. */
const Reconnect = {
  /** Reports a failed connection after the wait. */
  timer: undefined,

  /** Shows the dialog over the page, filled with the current settings. */
  async open() {
    await oyaBrowser.showOverlay();
    try {
      Reconnect.present(await oyaBrowser.getConfig());
    } catch (error) {
      oyaBrowser.hideOverlay();
      Setup.error(error.message || 'Could not load connection settings.');
    }
  },

  /** Fills the dialog, opens it and focuses the server field. */
  present(cfg) {
    Reconnect.fill(cfg);
    Dom.byId('reconnect-overlay').classList.add('open');
    Dialogs.activate(Dom.byId('reconnect-overlay'), Reconnect.close);
    Dom.byId('reconn-server').focus();
  },

  /** Puts the saved settings in the fields and resets the form. */
  fill(cfg) {
    Dom.byId('reconn-server').value = cfg.serverUrl || '';
    Dom.byId('reconn-key').value = cfg.apiKey || '';
    Dom.byId('reconn-name').value = cfg.browserName || '';
    Dom.byId('reconn-error').textContent = '';
    Reconnect.idle('Save & Reconnect');
  },

  /** The save button, enabled, reading `text`. */
  idle(text) {
    Dom.byId('reconn-save').disabled = false;
    Dom.byId('reconn-save').textContent = text;
  },

  /** Hides the dialog and gives the page back. */
  close() {
    clearTimeout(Reconnect.timer);
    Dom.byId('reconnect-overlay').classList.remove('open');
    oyaBrowser.hideOverlay();
    Dialogs.deactivate();
  },

  /** Saves the settings and waits for the connection to report back. */
  async save() {
    const [server, key, name] = ['reconn-server', 'reconn-key', 'reconn-name'].map((id) => Dom.byId(id).value.trim());
    const message = Dom.byId('reconn-error');
    if (!Setup.SERVER_URL.test(server) || !key) {
      message.textContent = 'Enter a ws:// or wss:// server address and an API key.';
      return;
    }
    Reconnect.saving(message);
    await Reconnect.apply(server, key, name, message);
  },

  /** The button and message while saving. */
  saving(message) {
    const button = Dom.byId('reconn-save');
    button.disabled = true;
    button.textContent = 'Connecting…';
    message.textContent = '';
    clearTimeout(Reconnect.timer);
  },

  /** Saves and shows the new settings in the profile, or says why it could not. */
  async apply(server, key, name, message) {
    try {
      await oyaBrowser.saveConfig({ serverUrl: server, apiKey: key, browserName: name || undefined });
      Reconnect.saved(server, name, message);
    } catch (error) {
      message.textContent = error.message || 'Could not save connection settings.';
      Reconnect.idle('Retry connection');
    }
  },

  /** The settings were saved: show them in the profile and wait for the connection. */
  saved(server, name, message) {
    Dom.byId('profile-server').textContent = server;
    Dom.byId('profile-title').textContent = name || 'This browser';
    Reconnect.timer = setTimeout(() => Reconnect.timedOut(message), RendererConstants.CONNECT_TIMEOUT_MS);
  },

  /** No connection after the wait: offer a retry. */
  timedOut(message) {
    Reconnect.idle('Retry connection');
    if (!ShellState.connected)
      message.textContent = 'Could not connect. Check the server address and API key, then try again.';
  },

  /** Connected: reset both forms and close the dialog if it is open. */
  connected() {
    clearTimeout(Reconnect.timer);
    Reconnect.idle('Save & Reconnect');
    Setup.error('');
    Setup.busy(false);
    // Close reconnect dialog if open
    if (Dom.byId('reconnect-overlay').classList.contains('open')) Reconnect.close();
  },
};

Dom.byId('connection-edit').addEventListener('click', () => {
  ShellDialog.close();
  Reconnect.open();
});
Dom.byId('reconn-cancel').addEventListener('click', Reconnect.close);
Dom.byId('reconn-save').addEventListener('click', Reconnect.save);
Dom.byId('reconnect-overlay').addEventListener('click', (e) => {
  if (e.target.id === 'reconnect-overlay') Reconnect.close();
});
