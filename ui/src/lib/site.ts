/**
 * One place for the canonical origin.
 *
 * Metadata, sitemap, robots and JSON-LD all have to agree on it: a mismatch
 * between them is how a site ends up with two identities in an index. The env
 * var lets a self-hosted deployment point them all at its own domain.
 */
/** The canonical origin, without a trailing slash. */
export const SITE_URL = (process.env.NEXT_PUBLIC_SITE_URL || 'https://oyabrowser.com').replace(/\/$/, '');

/** The product name shown in titles and structured data. */
export const SITE_NAME = 'Oya Browser';

/** The one-line promise, used in titles: who it is for and what it is, after the name. */
export const SITE_TAGLINE = 'the browser control plane for healthcare and regulated work';

/** The description every page and card shares unless it has its own. */
export const SITE_DESCRIPTION =
  'Browser agents for payer portals, EHRs and registries, with every action on a tamper-evident audit trail. ' +
  'HIPAA compliant. We sign BAAs. Self-host or Oya Cloud.';
