/**
 * Connection status: the toolbar pill, the switch between setup and browsing
 * modes, and the saved settings and status loaded at start.
 */
/* global oyaBrowser, Dom, ShellState, Reconnect */
/* exported ConnectionStatus */

/** The connection pill and mode. */
const ConnectionStatus = {
  /** Shows connected (with the browser id) or offline. */
  updatePill(isConnected, browserId) {
    ShellState.connected = isConnected;
    Dom.byId('save-profile').disabled = !isConnected;
    const [pill, label] = [Dom.byId('conn-pill'), Dom.byId('conn-label')];
    pill.className = isConnected ? 'conn-pill ok' : 'conn-pill';
    label.textContent = isConnected ? 'Connected' : 'Offline';
    pill.title = isConnected ? 'Connected' + (browserId ? `, ${browserId}` : '') : 'Not connected, click to configure';
  },

  /** A status report from the main process. */
  onStatus(s) {
    ConnectionStatus.updatePill(s.connected, s.browserId);
    if (s.connected) Reconnect.connected();
  },

  /** Fills the setup form; a configured browser shows "Connecting…" until the status arrives. */
  loadConfig(cfg) {
    Dom.byId('cfg-server').value = cfg.serverUrl || '';
    Dom.byId('cfg-key').value = cfg.apiKey || '';
    Dom.byId('cfg-name').value = cfg.browserName || '';
    // If already configured, show setup briefly, ws-status will switch to browsing
    if (!cfg.apiKey) return;
    Dom.byId('conn-pill').className = 'conn-pill trying';
    Dom.byId('conn-label').textContent = 'Connecting…';
  },

  /** The status at start: the mode, the page address and the pill. */
  loadStatus(s) {
    if (s.browsing) document.body.className = 'mode-browsing';
    if (s.url) Dom.byId('url-bar').value = s.url;
    ConnectionStatus.updatePill(s.connected, s.browserId);
  },
};

oyaBrowser.onWsStatus(ConnectionStatus.onStatus);
// Main process tells us to switch modes
oyaBrowser.onModeChanged((mode) => {
  document.body.className = mode === 'browsing' ? 'mode-browsing' : 'mode-setup';
});
