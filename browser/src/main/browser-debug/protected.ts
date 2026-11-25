/** Challenge frames retain their existing exclusion from general page inspection. */
const CHALLENGE =
  /^https:\/\/([^/]+\.)?(recaptcha\.net|hcaptcha\.com|challenges\.cloudflare\.com)\/|^https:\/\/(www\.)?google\.com\/recaptcha\//;
/** The dedicated challenge workflow owns these documents. */
export function requireInspectableDebugUrl(url: string): void {
  if (CHALLENGE.test(url)) throw Error('Protected challenge frame; use its dedicated workflow.');
}
