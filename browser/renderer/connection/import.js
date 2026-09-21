/**
 * "Import logins" in the connection dialog: bring the sessions of a browser on
 * this computer (Chrome, Firefox, Arc, Brave, Edge) into Oya, so nobody has to
 * sign in to every site again. The main process does the import (main/mirror);
 * this shows the choice, and says plainly how it went.
 */
/* global oyaBrowser, Dom */
/* exported LoginImport */

/** Importing logins from another browser. */
const LoginImport = {
  /** Whether the server is connected: an import needs somewhere to go. */
  connected: false,
  /** Whether an import is running. */
  running: false,

  /** One browser as a choice in the list; the person's default says so. */
  option(source) {
    const option = document.createElement('option');
    option.value = source.id;
    option.textContent = source.isDefault ? `${source.name} (your default browser)` : source.name;
    return option;
  },

  /** Lists the browsers found on this computer, the person's default first. */
  async load() {
    const sources = (await oyaBrowser.importSources().catch(() => [])) || [];
    Dom.byId('import-source').replaceChildren(...sources.map(LoginImport.option));
    Dom.byId('import-controls').hidden = !sources.length;
    LoginImport.render();
  },

  /** The name of the browser chosen, as the list shows it, without the default note. */
  chosenName() {
    const select = Dom.byId('import-source');
    // Array.from: a real NodeList has no find(), and the click then threw before anything was imported.
    const option = Array.from(select.querySelectorAll('option')).find((o) => o.value === select.value);
    return (option?.textContent || 'your browser').replace(/ \(.*\)$/, '');
  },

  /** What stands in the way of an import, or '' when nothing does. */
  blocked() {
    if (Dom.byId('import-controls').hidden) return 'No supported browser found on this computer.';
    return LoginImport.connected ? '' : 'Connect to Oya to import your logins.';
  },

  /** The last thing to tell the person: progress, or how the last import went. Kept across reconnects. */
  note: { text: '', kind: '' },

  /** Shows the button's state, and what stands in the way or, when nothing does, the last note. */
  render() {
    const blocked = LoginImport.blocked();
    const status = Dom.byId('import-status');
    Dom.byId('import-logins').disabled = LoginImport.running || !!blocked;
    status.textContent = blocked || LoginImport.note.text;
    status.dataset.kind = blocked ? '' : LoginImport.note.kind;
  },

  /** Sets the note; `kind` is 'error' for a failure. */
  say(text, kind = '') {
    LoginImport.note = { text, kind };
    LoginImport.render();
  },

  /** The connection came or went. A successful import reconnects, and its summary has to outlive that. */
  onStatus(s) {
    LoginImport.connected = !!s.connected;
    LoginImport.render();
  },

  /** Starts the import of the chosen browser; its progress arrives through onMirrorStatus. */
  async start() {
    LoginImport.running = true;
    LoginImport.say(`Reading your logins from ${LoginImport.chosenName()}…`);
    await oyaBrowser
      .reimportBrowser(Dom.byId('import-source').value)
      .catch((e) => LoginImport.finish(e.message, 'error'));
  },

  /** The import ended: say how, and give the button back. */
  finish(text, kind = '') {
    LoginImport.running = false;
    LoginImport.say(text, kind);
  },

  /** What one finished import brought over, in words. */
  summary(s) {
    const profiles = `${s.profiles} profile${s.profiles === 1 ? '' : 's'}`;
    return `Imported ${s.source}: ${profiles}, ${s.cookies} cookies. You are signed in here now.`;
  },

  /** Progress from the main process: started, or done with a result, an error, or nothing to bring. */
  onMirror(s) {
    if (s.started) return void ((LoginImport.running = true), LoginImport.render());
    if (s.error) return LoginImport.finish(s.error, 'error');
    if (s.empty) return LoginImport.finish('Nothing to import: that browser has no profiles with logins.');
    LoginImport.finish(LoginImport.summary(s));
  },
};

oyaBrowser.onWsStatus(LoginImport.onStatus);
// Asked after subscribing, never before: ws-status is sent once, and it can land between two
// scripts loading. A status asked for earlier by another file left this one "not connected".
oyaBrowser
  .getStatus()
  .then(LoginImport.onStatus)
  .catch(() => {});
oyaBrowser.onMirrorStatus(LoginImport.onMirror);
Dom.byId('import-logins').addEventListener('click', LoginImport.start);
LoginImport.load();
