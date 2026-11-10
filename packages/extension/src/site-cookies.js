/**
 * Reads the cookies a site's pages are sent, in the shape Oya's cookie jar
 * keeps (the same fields the desktop app syncs, browser/src/main/sync/cookie-sync.ts).
 */

/** The cookie fields the server keeps, in the order it gets them. Store ids and partitions stay here. */
export const SLIM_FIELDS = Object.freeze([
  'name',
  'value',
  'domain',
  'path',
  'secure',
  'httpOnly',
  'sameSite',
  'expirationDate',
  'hostOnly',
]);

/** One browser cookie as the server's cookie, with no field the jar does not keep. */
function slimCookie(cookie) {
  const slim = Object.fromEntries(SLIM_FIELDS.filter((f) => cookie[f] !== undefined).map((f) => [f, cookie[f]]));
  slim.sameSite = cookie.sameSite || 'unspecified';
  return slim;
}

/** True on Firefox, whose cookie query refuses to search every first-party jar unless told to. */
export function isFirefox(api) {
  return api.runtime.getURL('').startsWith('moz-extension:');
}

/**
 * The cookie store the tab uses: a Firefox container names it on the tab;
 * Chrome lists which tabs each store (normal, incognito) serves. Without it
 * the query reads the default store, another account's logins.
 */
async function storeOf(api, tab) {
  if (tab?.cookieStoreId) return tab.cookieStoreId;
  const stores = await api.cookies.getAllCookieStores();
  return stores.find((s) => s.tabIds.includes(tab?.id))?.id;
}

/**
 * The cookies the browser would send to this page, from the tab's own store.
 * Firefox is asked across first-party jars (`firstPartyDomain: null`), which
 * Chrome would reject as an unknown field. Partitioned cookies belong to
 * embeds on other sites and are not this site's login.
 */
export async function readSiteCookies(api, url, tab) {
  const storeId = await storeOf(api, tab);
  const query = { url, ...(storeId && { storeId }), ...(isFirefox(api) && { firstPartyDomain: null }) };
  const cookies = await api.cookies.getAll(query);
  return cookies.filter((c) => !c.partitionKey).map(slimCookie);
}
