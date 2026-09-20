/**
 * One place for the canonical origin.
 *
 * Metadata, sitemap, robots and JSON-LD all have to agree on it — a mismatch
 * between them is how a site ends up with two identities in an index. The env
 * var lets a self-hosted deployment point them all at its own domain.
 */
/** The canonical origin, without a trailing slash. */
export const SITE_URL = (process.env.NEXT_PUBLIC_SITE_URL || 'https://browser.getoya.ai').replace(/\/$/, '');

/** The product name shown in titles and structured data. */
export const SITE_NAME = 'Oya Browser';

/** The one-line promise, used in titles. */
export const SITE_TAGLINE = 'Run it once. Make it repeatable.';

/** The description every page and card shares unless it has its own. */
export const SITE_DESCRIPTION =
  'The browser for AI agents. Turn browser tasks into reusable playbooks, replay with new inputs, ' +
  'review repairs, and step in through live view. Choose your browser provider with one API.';
