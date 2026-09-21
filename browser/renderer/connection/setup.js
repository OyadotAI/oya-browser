/**
 * The first-run setup screen: server address, API key and browser name, then
 * Connect (or Browse offline).
 */
/* global oyaBrowser, Dom, ShellState, RendererConstants */
/* exported Setup */

/** The setup screen. */
const Setup = {
  /** A usable server address: ws:// or wss:// and no spaces. */
  SERVER_URL: /^wss?:\/\/[^\s]+$/,

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

  /** No connection yet after the wait. */
  timedOut() {
    Setup.busy(false);
    if (!ShellState.connected) Setup.error('Could not connect, check URL and API key');
  },
};

Dom.byId('open-console').addEventListener('click', () => oyaBrowser.openConsole(Dom.byId('cfg-server').value));
Dom.byId('btn-connect').addEventListener('click', Setup.connect);
Dom.byId('btn-skip').addEventListener('click', () => {
  oyaBrowser.enterBrowsing();
});
// Enter in setup fields triggers connect
document.querySelectorAll('.setup-card input').forEach((input) => {
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') Dom.byId('btn-connect').click();
  });
});
