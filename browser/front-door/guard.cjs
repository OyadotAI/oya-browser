/**
 * What a CDP client may not ask this browser for. Either kind of client
 * already drives the pages; what it must not reach is the person's computer.
 * Every client is held to the rule the address bar and the desktop's own
 * commands follow: only web addresses open. A relay, which is a remote caller
 * the Oya server forwards, is also kept away from the filesystem, the app's
 * life and the machine's permissions. A local client is a process of the
 * person's own and already has all three, so refusing it would only break
 * their own upload and download scripts.
 */
const { isWebAddress, NOT_A_WEB_ADDRESS } = require('../main/tabs/navigation.cjs');
const { LOCAL_FILES_UNAVAILABLE, REMOTE_UNAVAILABLE, AROUND_THE_DOOR } = require('../constants.cjs');

/** Refuses with `message`. */
const refuse = (message) => ({ error: message });

/** Refuses unless `url` is a web address. */
const webOnly = (url) => (isWebAddress(url) ? null : refuse(NOT_A_WEB_ADDRESS));

/** A command that would reach Chromium around this door and its rules; refused for every client. */
const AROUND = () => refuse(AROUND_THE_DOOR);

/** A command that reaches the computer rather than the page; refused for a relay. */
const PAST_THE_PAGE = () => refuse(REMOTE_UNAVAILABLE);

/** Rules for every client: method -> answer for its params, null to forward. */
const FOR_EVERYONE = {
  'Page.navigate': (params) => webOnly(params?.url),
  'Target.createTarget': (params) => webOnly(params?.url || 'about:blank'),
  /** Gives a page a binding that speaks CDP straight to Chromium, around this guard. */
  'Target.exposeDevToolsProtocol': AROUND,
  /** Wraps a command in a string this guard cannot read. Playwright and Puppeteer never send it. */
  'Target.sendMessageToTarget': AROUND,
};

/** Rules for a relay only, on top of those. */
const FOR_RELAY = {
  /** Uploads read a file from this computer; so do dropped files. */
  'DOM.setFileInputFiles': () => refuse(LOCAL_FILES_UNAVAILABLE),
  'Input.dispatchDragEvent': (params) => (params?.data?.files?.length ? refuse(LOCAL_FILES_UNAVAILABLE) : null),
  /** Writes downloads into a folder of the caller's naming. */
  'Page.setDownloadBehavior': () => refuse(LOCAL_FILES_UNAVAILABLE),
  /**
   * Playwright sends it on connect, so it is answered, not refused, and never
   * reaches Chromium with the caller's folder. Electron ignores this setting
   * anyway: its own download handler asks the person where to save.
   */
  'Browser.setDownloadBehavior': () => ({ result: {} }),
  /** Close or crash the person's whole app. */
  'Browser.close': PAST_THE_PAGE,
  'Browser.crash': PAST_THE_PAGE,
  'Browser.crashGpuProcess': PAST_THE_PAGE,
  /** Would lift the app's own refusal of location and clipboard reads. */
  'Browser.grantPermissions': PAST_THE_PAGE,
  'Browser.setPermission': PAST_THE_PAGE,
  /** Binds a port on this computer. */
  'Tethering.bind': PAST_THE_PAGE,
};

/** The rule in `rules` for `method`, applied to `params`; null when there is none or it lets the command through. */
const applied = (rules, { method, params }) => (Object.hasOwn(rules, method) ? rules[method](params) : null);

/**
 * How the door answers `msg` itself: `{ error }` to refuse, `{ result }` to
 * answer without Chromium, or null to forward it.
 */
function answerFor(msg, relay) {
  return applied(FOR_EVERYONE, msg) || (relay ? applied(FOR_RELAY, msg) : null);
}

module.exports = { answerFor };
