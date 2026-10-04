/**
 * "Imported logins" on the account page: bring the sessions of a browser on
 * this computer (Chrome, Firefox, Arc, Brave, Edge) into Oya, so nobody has to
 * sign in to every site again. The main process does the import (main/mirror)
 * and keeps the last few (config `imports`); this shows the latest, the ones
 * before it, the choice of browser, and says plainly how it went.
 */
/* global oyaBrowser, Dom, When, RendererConstants */
/* exported LoginImport */

/** Importing logins from another browser. */
const LoginImport = {
  /** Whether the server is connected: an import needs somewhere to go. */
  connected: false,
  /** Whether an import is running. */
  running: false,
  /** Finished imports, newest first: { source, at, sites, cookies, profiles }. */
  history: [],
  /** The browsers found on this computer, by name, for the empty state. */
  names: [],

  /** Takes the import history the main process kept, and shows it. */
  showHistory(imports) {
    LoginImport.history = Array.isArray(imports) ? imports : [];
    LoginImport.renderHistory();
  },

  /** How much one import brought: sites when it counted them, else cookies. */
  amount(record) {
    const [count, what] = record.sites ? [record.sites, 'site'] : [record.cookies || 0, 'cookie'];
    return `${count} ${what}${count === 1 ? '' : 's'}`;
  },

  /** The latest import (or the empty state), the earlier ones, and the button's label. */
  renderHistory() {
    const [latest, ...earlier] = LoginImport.history;
    Dom.byId('import-logins').textContent = latest ? 'Import again' : 'Import';
    if (!latest) return LoginImport.renderEmpty();
    Dom.byId('import-latest').textContent = `Imported from ${latest.source}`;
    Dom.byId('import-detail').replaceChildren(When.element(latest.at), ` · ${LoginImport.amount(latest)}`);
    LoginImport.renderEarlier(earlier.slice(0, RendererConstants.EARLIER_IMPORTS_SHOWN));
  },

  /** No import yet: say what an import is for, naming the browsers this computer has. */
  renderEmpty() {
    const names = LoginImport.names.length ? LoginImport.names : ['Chrome', 'Arc', 'Firefox'];
    const list = names.length > 1 ? `${names.slice(0, -1).join(', ')} or ${names.at(-1)}` : names[0];
    Dom.byId('import-latest').textContent = 'No imports yet';
    Dom.byId('import-detail').textContent = `Bring your logins from ${list}, so you are already signed in here.`;
    LoginImport.renderEarlier([]);
  },

  /** The imports before the latest, one line each. */
  renderEarlier(records) {
    const item = (r) => {
      const li = Dom.node('li', `${r.source} · `);
      li.append(When.element(r.at), ` · ${LoginImport.amount(r)}`);
      return li;
    };
    Dom.byId('import-history').replaceChildren(...records.map(item));
    Dom.byId('import-history').hidden = !records.length;
  },

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
    LoginImport.names = sources.map((source) => source.name);
    LoginImport.render();
    LoginImport.renderHistory();
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

  /** A finished import goes to the top of the history the page shows, as the main process kept it. */
  remember(s) {
    const record = { ...s, at: s.at || Date.now() };
    LoginImport.showHistory([record, ...LoginImport.history].slice(0, RendererConstants.EARLIER_IMPORTS_SHOWN + 1));
  },

  /** Progress from the main process: started, or done with a result, an error, or nothing to bring. */
  onMirror(s) {
    if (s.started) return void ((LoginImport.running = true), LoginImport.render());
    if (s.error) return LoginImport.finish(s.error, 'error');
    if (s.empty) return LoginImport.finish('Nothing to import: that browser has no profiles with logins.');
    LoginImport.remember(s);
    LoginImport.finish('Done. You are signed in to those sites here now.');
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
oyaBrowser.getConfig().then((config) => LoginImport.showHistory(config?.imports));
LoginImport.load();
