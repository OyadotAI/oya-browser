/**
 * What the popup does, without the page: work out what to show when it
 * opens, and send this site's logins when the button is pressed. `api` is the
 * browser's extension namespace (chrome or browser); `fetchFn` is fetch.
 * Both are passed in so the rules can be tested without a browser.
 */
import { StorageKey, VIEWER_ROLE } from './constants.js';
import { Messages, errorMessage, sentMessage } from './messages.js';
import { oyaClient } from './oya.js';
import { isFirefox, readSiteCookies } from './site-cookies.js';
import { siteOf } from './site.js';

/** The saved API key and last profile, from this browser's local storage only. */
async function saved(api) {
  const stored = await api.storage.local.get([StorageKey.API_KEY, StorageKey.LAST_PROFILE]);
  return { key: stored[StorageKey.API_KEY] || '', lastProfile: stored[StorageKey.LAST_PROFILE] || '' };
}

/** The site of the tab the popup was opened over, with that tab, or null when it is not a website. */
async function activeSite(api) {
  const [tab] = await api.tabs.query({ active: true, currentWindow: true });
  const site = siteOf(tab?.url ?? '');
  return site && { ...site, tab: { id: tab.id, cookieStoreId: tab.cookieStoreId } };
}

/**
 * The ready state: the site, the profiles, the one to preselect, and a hint
 * when Firefox has yet to give access, because its prompt closes the popup
 * and the person has to open it again to send.
 */
async function readyState(api, site, profiles, lastProfile) {
  const selected = profiles.some((p) => p.id === lastProfile) ? lastProfile : profiles[0].id;
  const granted = await api.permissions.contains({ origins: [site.pattern] });
  const hint = !granted && isFirefox(api) ? Messages.askAccess : null;
  return { ready: true, site, profiles, selected, granted, hint };
}

/** A failed call as a value, so the caller can show its message instead of throwing. */
const asFailure = (error) => ({ error });

/**
 * What the popup shows on opening: a message alone (no key, not a website,
 * no profiles, a refused key), or the ready state with its profiles.
 */
export async function openPopup(api, fetchFn) {
  const [{ key, lastProfile }, site] = await Promise.all([saved(api), activeSite(api)]);
  if (!key) return { message: Messages.noKey, site };
  if (!site) return { message: Messages.notWebsite, site };
  const profiles = await oyaClient(key, fetchFn).listProfiles().catch(asFailure);
  if (profiles.error) return { message: errorMessage(profiles.error), site };
  if (!profiles.length) return { message: Messages.noProfiles, site };
  return { ...(await readyState(api, site, profiles, lastProfile)), key };
}

/** Reads and sends the site's cookies once access is given; a viewer key is stopped before any read. */
async function sendGranted(api, fetchFn, { key, site, profile }) {
  const client = oyaClient(key, fetchFn);
  if ((await client.whoami()).role === VIEWER_ROLE) return Messages.viewer;
  const cookies = await readSiteCookies(api, site.url, site.tab);
  if (!cookies.length) return Messages.nothing;
  const result = await client.sendCookies(profile.id, cookies);
  await api.storage.local.set({ [StorageKey.LAST_PROFILE]: profile.id });
  return sentMessage(result, profile.name);
}

/**
 * Sends this site's logins to the profile. Asks for access to this one site
 * first, as the first thing the click does, because the browser shows that
 * prompt only from a fresh click. Denied access reads and sends nothing.
 */
export async function sendLogins(api, fetchFn, request) {
  const granted = await api.permissions.request({ origins: [request.site.pattern] }).catch(() => false);
  if (!granted) return Messages.denied;
  return sendGranted(api, fetchFn, request).catch(errorMessage);
}

/**
 * Checks a key by listing its profiles, and saves it only when Oya accepts
 * it. Answers the profile count, or throws the OyaError that refused it.
 */
export async function saveKey(api, fetchFn, key) {
  const profiles = await oyaClient(key, fetchFn).listProfiles();
  await api.storage.local.set({ [StorageKey.API_KEY]: key });
  return profiles.length;
}
