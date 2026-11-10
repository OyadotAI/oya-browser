/**
 * Every sentence the popup and settings page show, and which one fits a
 * result. Each message carries the one action that helps next.
 */
import { IMPORT_CAP_ERROR, NO_RESPONSE, Status, UNKNOWN_KEY_ERROR } from './constants.js';

/** What the person can do after a message: open settings, open the console, try again, close, or nothing. */
export const Action = Object.freeze({
  SETTINGS: 'settings',
  CONSOLE: 'console',
  RETRY: 'retry',
  DONE: 'done',
  NONE: 'none',
});

/** The fixed messages, by state. */
export const Messages = Object.freeze({
  noKey: { text: 'Add your Oya API key to start.', action: Action.SETTINGS },
  loading: { text: 'Loading profiles…', action: Action.NONE },
  noProfiles: { text: 'This key has no profiles yet. Create one in the console.', action: Action.CONSOLE },
  notWebsite: { text: 'Open a website to send its logins.', action: Action.NONE },
  denied: { text: 'Oya was not allowed to read this site. Nothing was sent.', action: Action.RETRY },
  sending: { text: 'Sending…', action: Action.NONE },
  nothing: { text: 'No logins on this site. Sign in here first.', action: Action.NONE },
  viewer: { text: 'This key can only read. Use a key with member access.', action: Action.SETTINGS },
  unknownKey: { text: 'Oya does not recognise this key.', action: Action.SETTINGS },
  expired: { text: 'Your key has expired or is missing. Add a new one in settings.', action: Action.SETTINGS },
  gone: {
    text: 'The project this key belonged to was deleted. Add a key from another project.',
    action: Action.SETTINGS,
  },
  busy: { text: 'Too many requests. Try again in a minute.', action: Action.RETRY },
  noProfile: { text: 'That profile no longer exists. Pick another one.', action: Action.RETRY },
  tooMany: { text: 'This site has too many logins to send at once. Nothing was sent.', action: Action.NONE },
  askAccess: {
    text: 'Firefox will ask to let Oya read this site. If this window closes, open it again and press Send.',
    action: Action.NONE,
  },
  offline: { text: 'Could not reach Oya. Nothing was sent.', action: Action.RETRY },
  failed: { text: 'Oya had a problem. Nothing was sent. Try again.', action: Action.RETRY },
});

/** The message for a refused call, by status. Anything not listed is Oya failing: try again. */
/** A 400 is only the import cap when the server says so (pool.ts importRefusal); any other 400 is a failure. */
const capOrFailure = (reason) => (reason?.startsWith(IMPORT_CAP_ERROR) ? Messages.tooMany : Messages.failed);

const BY_STATUS = {
  [Status.UNAUTHORIZED]: () => Messages.expired,
  [Status.FORBIDDEN]: (reason) => (reason === UNKNOWN_KEY_ERROR ? Messages.unknownKey : Messages.viewer),
  [Status.GONE]: () => Messages.gone,
  [Status.NOT_FOUND]: () => Messages.noProfile,
  [Status.BAD_REQUEST]: capOrFailure,
  [Status.PAYLOAD_TOO_LARGE]: () => Messages.tooMany,
  [Status.TOO_MANY_REQUESTS]: () => Messages.busy,
  [NO_RESPONSE]: () => Messages.offline,
};

/** The message for an OyaError: the status's own, else "Oya had a problem". Anything else thrown never reached Oya. */
export function errorMessage(error) {
  const status = error?.status ?? NO_RESPONSE;
  const pick = Object.hasOwn(BY_STATUS, status) ? BY_STATUS[status] : () => Messages.failed;
  return pick(error?.reason);
}

/** "1 login" or "12 logins". */
function logins(count) {
  return `${count} ${count === 1 ? 'login' : 'logins'}`;
}

/** What a finished import says: how many went to which profile, and how many were skipped. */
export function sentMessage({ imported, skipped }, profileName) {
  if (!skipped) {
    return {
      text: `${logins(imported)} sent to ${profileName}. Open the site in Oya and you are signed in.`,
      action: Action.DONE,
    };
  }
  return { text: `${imported} sent, ${skipped} skipped (expired or unusable).`, action: Action.DONE };
}

/** What saving a key says once the key has listed its profiles. */
export function keySavedMessage(count) {
  return `Key saved. ${count} ${count === 1 ? 'profile' : 'profiles'} found.`;
}

/** What saving a key says when Oya refused it, or could not be reached. */
export function keyRefusedMessage(error) {
  return error?.status === NO_RESPONSE ? 'Could not reach Oya. Check your connection.' : 'Oya did not accept this key.';
}
