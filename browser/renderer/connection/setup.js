/**
 * The first-run welcome screen: what the browser is for, and one panel that
 * switches between three views: start (one-click "Sign in with Oya", which the
 * dashboard answers with a pairing link), waiting (for that sign-in to finish)
 * and manual (server address, API key and browser name, for self-hosters).
 */
/* global oyaBrowser, Dom, ShellState, RendererConstants */
/* exported Setup */

/** The setup screen. */
const Setup = {
  /** A usable server address: ws:// or wss:// and no spaces. */
  SERVER_URL: /^wss?:\/\/[^\s]+$/,

  /** Shows one of the panel's views: 'start', 'waiting' or 'manual'. */
  show(view) {
    Setup.error('');
    document.querySelector('.setup-screen').dataset.view = view;
    const panel = document.querySelector(`.welcome-view[data-view="${view}"]`);
    (panel.querySelector('input') || panel.querySelector('button')).focus();
  },

  /** Shows `text` under the form ('' clears it). */
  error(text) {
    Dom.byId('setup-error').textContent = text;
  },

  /** Why the entered settings cannot be used, or '' when they can. */
  problem(server, key) {
    if (!Setup.SERVER_URL.test(server)) return 'Enter a valid ws:// or wss:// server address.';
    if (!key) return 'API key is required';
    return '';
  },

  /** Shows the Connect button as working or idle. */
  busy(on) {
    const btn = Dom.byId('btn-connect');
    btn.disabled = on;
    btn.textContent = on ? 'Connecting...' : 'Connect';
  },

  /** Saves the settings; the connection status switches modes, or the timeout reports failure. */
  async connect() {
    const [server, key, name] = ['cfg-server', 'cfg-key', 'cfg-name'].map((id) => Dom.byId(id).value.trim());
    Setup.error(Setup.problem(server, key));
    if (Setup.problem(server, key)) return;
    Setup.busy(true);
    const settings = { serverUrl: server, apiKey: key, browserName: name || undefined };
    const saved = await oyaBrowser.saveConfig(settings).then(() => true, Setup.failed);
    // Wait for ws-status callback to switch modes, but timeout after 5s
    if (saved === true) setTimeout(Setup.timedOut, RendererConstants.CONNECT_TIMEOUT_MS);
  },

  /** Saving the settings failed. */
  failed(error) {
    Setup.error(error.message || 'Could not save connection settings.');
    Setup.busy(false);
  },

  /** Opens the dashboard, which sends back a pairing link once the person is signed in. */
  async signIn() {
    Setup.error('');
    const url = await oyaBrowser.openConsole(Dom.byId('cfg-server').value);
    if (!url) return Setup.error('The server address is not valid. Fix it under "Connect with an API key".');
    Setup.show('waiting');
  },

  /** No connection yet after the wait. */
  timedOut() {
    Setup.busy(false);
    if (!ShellState.connected) Setup.error('Could not connect, check URL and API key');
  },
};

Dom.byId('open-console').addEventListener('click', () => oyaBrowser.openConsole(Dom.byId('cfg-server').value));
Dom.byId('btn-connect').addEventListener('click', Setup.connect);
Dom.byId('btn-signin').addEventListener('click', Setup.signIn);
Dom.byId('btn-signin-again').addEventListener('click', Setup.signIn);
Dom.byId('btn-manual').addEventListener('click', () => Setup.show('manual'));
document.querySelectorAll('[data-view-go]').forEach((btn) => {
  btn.addEventListener('click', () => Setup.show(btn.dataset.viewGo));
});
Dom.byId('btn-skip').addEventListener('click', () => {
  oyaBrowser.enterBrowsing();
});
// Enter in setup fields triggers connect
document.querySelectorAll('.welcome-panel input').forEach((input) => {
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') Dom.byId('btn-connect').click();
  });
});
