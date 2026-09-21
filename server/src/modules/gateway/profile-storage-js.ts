/**
 * Page-side scripts that read and replay a profile's origin storage
 * (localStorage and sessionStorage).
 */

/** Dumps the current origin's storage, or null where storage is not accessible. */
export const ORIGIN_STORAGE_JS = `(() => {
  const dump = (s) => { const o = {}; for (let i = 0; i < s.length; i++) { const k = s.key(i); o[k] = s.getItem(k); } return o; };
  try { return { origin: location.origin, local: dump(localStorage), session: dump(sessionStorage) }; }
  catch { return null; }
})()`;

/** Writes saved storage back into the current origin. */
export const restoreStorageJS = (data) => `(() => {
  try {
    for (const [k, v] of Object.entries(${JSON.stringify(data.local || {})})) localStorage.setItem(k, v);
    for (const [k, v] of Object.entries(${JSON.stringify(data.session || {})})) sessionStorage.setItem(k, v);
    return true;
  } catch { return false; }
})()`;
