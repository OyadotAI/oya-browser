/**
 * One place for the canonical origin.
 *
 * Metadata, sitemap, robots and JSON-LD all have to agree on it — a mismatch
 * between them is how a site ends up with two identities in an index. The env
 * var lets a self-hosted deployment point them all at its own domain.
 */
export const SITE_URL = (process.env.NEXT_PUBLIC_SITE_URL || 'https://browser.getoya.ai').replace(/\/$/, '');

export const SITE_NAME = 'Oya Browser';

export const SITE_TAGLINE = 'Run it once. Make it repeatable.';

export const SITE_DESCRIPTION =
  'The browser for AI agents. Turn browser tasks into reusable playbooks, replay with new inputs, '
  + 'review repairs, and step in through live view. Choose your browser provider with one API.';
