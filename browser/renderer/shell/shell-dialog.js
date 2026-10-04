/**
 * The shell dialog: the command palette, or the account page (who this browser
 * is signed in as, sync, imports, the device, and the server and browser id
 * under Advanced).
 */
/* global oyaBrowser, Dom, Dialogs, CommandPalette, AccountCard, LoginImport, ProfileSync */
/* exported ShellDialog */

/** The commands and account dialog. */
const ShellDialog = {
  /** This browser's id on the server ('' while offline). */
  browserId: '',

  /** Hides the dialog and gives the page back. */
  close() {
    Dom.byId('shell-overlay').hidden = true;
    oyaBrowser.hideOverlay('shell');
    Dialogs.deactivate();
  },

  /** Opens the dialog on the account page, or on the command palette. */
  async open(profile = false) {
    const overlay = Dom.byId('shell-overlay');
    // A pairing link or a reconnect can change the server after start, so read it afresh.
    if (profile) await ShellDialog.refresh();
    await oyaBrowser.showOverlay('shell');
    overlay.hidden = false;
    ShellDialog.showPage(overlay, profile);
    Dialogs.activate(overlay, ShellDialog.close);
    if (profile) Dom.byId('shell-dialog-close').focus();
    else CommandPalette.reset();
  },

  /** Reads the settings, status and account again, so the page says what is true now. */
  async refresh() {
    const [config, status] = await Promise.all([oyaBrowser.getConfig(), oyaBrowser.getStatus()]);
    ShellDialog.loadConfig(config);
    Dom.byId('profile-advanced').open = false;
    LoginImport.showHistory(config.imports);
    ProfileSync.load(config, status);
    await AccountCard.load(config, status);
  },

  /** Shows one page of the dialog and names the dialog after it. */
  showPage(overlay, profile) {
    Dom.byId('commands-section').hidden = profile;
    Dom.byId('profile-section').hidden = !profile;
    Dom.byId('shell-dialog-title').textContent = profile ? 'Account' : 'Commands';
    overlay.setAttribute('aria-label', profile ? 'Account and connection' : 'Commands and settings');
  },

  /** The saved server and this browser's name. */
  loadConfig(value) {
    Dom.byId('profile-server').textContent = value.serverUrl || 'Not configured';
    Dom.byId('profile-name').value = value.browserName || '';
  },

  /** The browser id, whenever the connection reports it. */
  updateProfile(value) {
    ShellDialog.browserId = value.browserId || '';
    Dom.byId('profile-browser-id').textContent = ShellDialog.browserId || 'Not connected';
    Dom.byId('copy-browser-id').disabled = !ShellDialog.browserId;
  },

  /** Copies `text`, saying what was copied, or how to copy it by hand. */
  async copy(text, what) {
    const status = Dom.byId('copy-status');
    try {
      await navigator.clipboard.writeText(text);
      status.textContent = `${what[0].toUpperCase()}${what.slice(1)} copied.`;
    } catch {
      status.textContent = `Could not copy. Select the ${what} to copy it manually.`;
    }
  },
};

Dom.byId('shell-dialog-close').addEventListener('click', ShellDialog.close);
Dom.byId('shell-overlay').addEventListener('click', (event) => {
  if (event.target === Dom.byId('shell-overlay')) ShellDialog.close();
});
Dom.byId('conn-pill').addEventListener('click', () => ShellDialog.open(true));
Dom.byId('btn-commands').addEventListener('click', () => ShellDialog.open());
oyaBrowser.getConfig().then(ShellDialog.loadConfig);
oyaBrowser.onWsStatus(ShellDialog.updateProfile);
oyaBrowser.getStatus().then(ShellDialog.updateProfile);
Dom.byId('copy-browser-id').addEventListener('click', () => ShellDialog.copy(ShellDialog.browserId, 'browser ID'));
Dom.byId('copy-server').addEventListener('click', () =>
  ShellDialog.copy(Dom.byId('profile-server').textContent, 'server address'),
);
