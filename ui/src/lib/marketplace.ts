/** A nonsecret cookie hint returns account authentication to AWS setup without exposing buyer identity. */

/** The server owns the encrypted purchase cookie; this companion only chooses navigation. */
const PENDING_COOKIE = 'oya_aws_pending';
/** The fixed return destination cannot be supplied by an untrusted URL. */
export const MARKETPLACE_PATH = '/marketplace/aws';

/** Reads no credentials or buyer metadata. */
export function pendingMarketplace(): boolean {
  return (
    typeof document !== 'undefined' && document.cookie.split(';').some((part) => part.trim() === `${PENDING_COOKIE}=1`)
  );
}

/** Stops redirect loops after setup expires or account association has completed. */
export function forgetMarketplace() {
  if (typeof document !== 'undefined') document.cookie = `${PENDING_COOKIE}=; Max-Age=0; Path=/; SameSite=Lax`;
}
