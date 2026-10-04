/**
 * The account page of the shell dialog: who this browser is signed in as and
 * whether it is connected (AccountCard), when its logins last reached the
 * server (ProfileSync), which profile it browses as (ProfilePersona), and the
 * device it is (ProfileDevice). Times read as "3 days ago", exact on hover (When).
 */
/* global oyaBrowser, Dom, RendererConstants, ConnectionStatus */
/* exported When, AccountCard, AccountActions, ProfileSync, ProfilePersona, ProfileDevice */

/** Past times in words. */
const When = {
  /** `at` (epoch milliseconds) as people say it: "just now", "5 minutes ago", "3 days ago". */
  ago(at, now = Date.now()) {
    const seconds = Math.round((now - at) / RendererConstants.MS_PER_SECOND);
    if (seconds < RendererConstants.JUST_NOW_SECONDS) return 'just now';
    const units = Object.entries(RendererConstants.TIME_UNIT_SECONDS);
    const [unit, size] = units.find(([, length]) => seconds >= length) || units.at(-1);
    return new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' }).format(-Math.round(seconds / size), unit);
  },

  /** A <time> reading `ago`, with the exact date and time on hover and for assistive technology. */
  element(at) {
    const time = Dom.node('time', When.ago(at));
    time.setAttribute('datetime', new Date(at).toISOString());
    time.title = new Date(at).toLocaleString();
    return time;
  },
};

/** The account header: avatar, name or email, project and plan, and the connection. */
const AccountCard = {
  /** The settings in effect when the page opened (server, whether a key is saved). */
  config: {},
  /** Whether the control socket is connected. */
  connected: false,
  /** The account last shown, or null when only a key is known. */
  account: null,

  /** Asks the main process who this browser is signed in as, and shows it. */
  async load(config, status) {
    Object.assign(AccountCard, { config, connected: !!status.connected });
    AccountCard.render(await oyaBrowser.getAccount().catch(() => null));
  },

  /** Who, in words: the person's name over their email, or that a key alone signs this browser in. */
  who(account) {
    const label = account?.name || account?.email;
    if (label) return { title: label, email: account.name ? account.email || '' : '', initial: label[0] };
    return { title: 'Signed in with an API key', email: '', initial: account?.project?.name?.[0] || 'O' };
  },

  /** Shows `account`: the header, the project and plan, the connection and the log-out label. */
  render(account) {
    AccountCard.account = account;
    const who = AccountCard.who(account);
    Dom.byId('account-name').textContent = who.title;
    Object.assign(Dom.byId('account-email'), { textContent: who.email, hidden: !who.email });
    Dom.byId('account-avatar').textContent = who.initial.toUpperCase();
    AccountCard.meta(account);
    AccountCard.status();
    AccountActions.label(account?.email);
  },

  /** The project this browser belongs to, and the plan as a badge when the server has plans. */
  meta(account) {
    const project = account?.project?.name;
    const text = project ? `Project ${project}` : AccountCard.connected ? '' : 'Connect to see your project';
    const plan = account?.plan ? [Dom.node('span', `${account.plan} plan`, 'plan-badge')] : [];
    Dom.byId('account-meta').replaceChildren(text, ...plan);
  },

  /** The server's host, as people read it. */
  host(serverUrl) {
    try {
      return new URL(String(serverUrl).replace(/^ws/, 'http')).host;
    } catch {
      return 'the server';
    }
  },

  /** Connected, reconnecting (a key is saved, the socket keeps trying) or offline, with the host. */
  status() {
    const host = AccountCard.host(AccountCard.config.serverUrl);
    const state = AccountCard.connected ? 'connected' : AccountCard.config.apiKey ? 'reconnecting' : 'offline';
    const words = { connected: `Connected to ${host}`, reconnecting: `Reconnecting to ${host}…`, offline: 'Offline' };
    Dom.byId('account-status').dataset.state = state;
    Dom.byId('account-status-text').textContent = words[state];
  },

  /** The connection came or went: say so, and learn the account once the server can answer. */
  onStatus(s) {
    AccountCard.connected = !!s.connected;
    AccountCard.status();
    if (s.connected && !AccountCard.account && !Dom.byId('profile-section').hidden)
      oyaBrowser.getAccount().then(AccountCard.render, () => {});
  },
};

/** Log out and Switch account. */
const AccountActions = {
  /** Names the account Log out signs out of, when it is known. */
  label(email) {
    const button = Dom.byId('sign-out');
    button.textContent = email ? `Log out of ${email}` : 'Log out';
    button.title = email ? `Forget this browser's key and sign out of ${email}` : "Forget this browser's key";
  },

  /** Opens the console's connect page: signing in there as someone else links this browser to them. */
  switchAccount() {
    oyaBrowser.openConsole(AccountCard.config.serverUrl);
  },
};

/** When this browser's logins last reached the server, how many sites it keeps, and Sync now. */
const ProfileSync = {
  /** When the logins last went to the server (epoch milliseconds, 0 for never). */
  at: 0,
  /** Sites the server said it keeps at the last Sync now, or null before one. */
  sites: null,
  /** Whether the control socket is connected. */
  connected: false,
  /** Gives the button back if the sync is never confirmed. */
  timer: undefined,

  /** Takes the saved sync record and the live status. */
  load(config, status) {
    ProfileSync.at = Math.max(config.lastSync?.at || 0, status.syncedAt || 0);
    ProfileSync.sites = config.lastSync ? config.lastSync.sites : null;
    ProfileSync.onStatus(status);
  },

  /** The sync line: when, and how many sites, or what has to happen first. */
  render() {
    const { at, sites, connected } = ProfileSync;
    const count = sites == null ? [] : [` · ${sites} site${sites === 1 ? '' : 's'}`];
    const when = at ? [connected ? 'Logins synced ' : 'Last synced ', When.element(at), ...count] : null;
    const idle = connected ? 'Logins sync while you are connected' : 'Logins sync when you reconnect';
    Dom.byId('sync-title').replaceChildren(...(when || [idle]));
    Dom.byId('sync-now').disabled = !connected || !!ProfileSync.timer;
  },

  /** The connection came or went. */
  onStatus(s) {
    ProfileSync.connected = !!s.connected;
    ProfileSync.render();
  },

  /** Sends every login to the server now; the confirmation arrives separately. */
  async save() {
    Dom.byId('profile-save-status').textContent = 'Syncing…';
    ProfileSync.awaitConfirmation();
    await oyaBrowser.saveProfile().catch((error) => ProfileSync.finish(error.message));
  },

  /** Without a confirmation in time, say so and give the button back. */
  awaitConfirmation() {
    clearTimeout(ProfileSync.timer);
    const unconfirmed = () => ProfileSync.finish('Sync not confirmed. Try again.');
    ProfileSync.timer = setTimeout(unconfirmed, RendererConstants.PROFILE_SAVE_TIMEOUT_MS);
    ProfileSync.render();
  },

  /** Shows the outcome and gives the button back. */
  finish(text) {
    clearTimeout(ProfileSync.timer);
    ProfileSync.timer = undefined;
    Dom.byId('profile-save-status').textContent = text;
    ProfileSync.render();
  },

  /** The server confirmed a sync: the line now says just now, and how many sites it keeps. */
  saved(state) {
    if (!state.error) Object.assign(ProfileSync, { at: Date.now(), sites: state.sites?.length || 0 });
    ProfileSync.finish(state.error || '');
  },
};

/** Which profile this browser browses as: its cookies and logins, in plain words. */
const ProfilePersona = {
  /** Names the profile from the connection's status. */
  onStatus(s) {
    const name = s.profileName && s.profileName !== 'Default' ? s.profileName : '';
    Dom.byId('persona-name').textContent = name ? `“${name}”` : 'Oya’s own profile';
    Dom.byId('fp-content').textContent = name
      ? 'Oya signs in to sites with this profile’s cookies and logins.'
      : 'Logins you make here are kept in this profile. Import to bring your own browser’s.';
  },
};

/** This device: its name (saved as it is changed) and what sites see it as. */
const ProfileDevice = {
  /** What people call each platform. */
  platforms: { Win32: 'Windows', MacIntel: 'Mac', 'Linux x86_64': 'Linux' },

  /** Shows the persona's device, or that there is none yet. */
  render(device) {
    const label = Dom.byId('profile-device');
    if (!device) return void (label.textContent = 'This computer, until you connect');
    const known = Object.hasOwn(ProfileDevice.platforms, device.platform || '');
    const platform = known ? ProfileDevice.platforms[device.platform] : device.platform;
    label.textContent = [platform, device.timezone, device.locale, device.screen].filter(Boolean).join(' · ');
    label.title = 'What sites see this browser as';
  },

  /** What a saved name says. */
  saved: () => 'Name saved.',

  /** Saves a new name for this browser; the server learns it as the browser reconnects. */
  async rename() {
    const input = Dom.byId('profile-name');
    const [name, saved] = [input.value.trim(), AccountCard.config.browserName || ''];
    if (!name || name === saved) return void (input.value = saved);
    AccountCard.config = { ...AccountCard.config, browserName: name };
    const said = await oyaBrowser.saveConfig({ browserName: name }).then(ProfileDevice.saved, (e) => e.message);
    Dom.byId('profile-name-status').textContent = said;
  },
};

oyaBrowser
  .getFingerprint()
  .then(ProfileDevice.render)
  .catch(() => ProfileDevice.render(null));
oyaBrowser.onFingerprintChanged(ProfileDevice.render);
for (const part of [AccountCard, ProfileSync, ProfilePersona]) oyaBrowser.onWsStatus(part.onStatus);
oyaBrowser.onProfileSaved(ProfileSync.saved);
Dom.byId('sync-now').addEventListener('click', ProfileSync.save);
Dom.byId('sign-out').addEventListener('click', () => oyaBrowser.signOut());
Dom.byId('switch-account').addEventListener('click', AccountActions.switchAccount);
Dom.byId('profile-name').addEventListener('change', ProfileDevice.rename);
Dom.byId('profile-name').addEventListener('input', () => (Dom.byId('profile-name-status').textContent = ''));
Dom.byId('profile-name').addEventListener('keydown', (event) => event.key === 'Enter' && event.target.blur());

// ── Init ──
oyaBrowser.getConfig().then((config) => {
  ConnectionStatus.loadConfig(config);
  AccountCard.config = config;
});
// The app may have connected before this page was listening, and ws-status is not sent again:
// the status at start goes to the account page as well as the pill.
oyaBrowser.getStatus().then((status) => {
  ConnectionStatus.loadStatus(status);
  for (const part of [AccountCard, ProfileSync, ProfilePersona]) part.onStatus(status);
});
