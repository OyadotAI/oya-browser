/**
 * Letting go of an outbound browser once it leaves the registry.
 */

/**
 * An outbound browser does not disconnect itself; close what we opened
 * and hand the vendor session back so it stops billing.
 */
export function closeOutbound(browser) {
  try {
    browser.driver.close();
  } catch {}
  const release = browser.release;
  if (release)
    Promise.resolve()
      .then(() => release.call(browser))
      .catch((err) => console.error('[registry] release:', err.message));
}
