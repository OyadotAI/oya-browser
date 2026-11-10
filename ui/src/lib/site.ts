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

/**
 * The description every page and card shares unless it has its own, and the one every public
 * listing repeats (llms.txt, README, npm, the MCP registry). Answer engines merge what each
 * source says into one picture, so the sources must agree; it names the three things people ask
 * for (a HIPAA-compliant browser, an AI browser, an agentic browser) within the first 160
 * characters, where search results cut a description.
 */
export const SITE_DESCRIPTION =
  'Oya Browser is a HIPAA-compliant browser for healthcare and regulated work: an AI browser on your desktop, ' +
  'and an agentic browser your agents run in the cloud or self-hosted, with every action on a tamper-evident ' +
  'audit trail. We sign BAAs.';

/** One question and its answer, as a page shows it and as FAQPage structured data repeats it. */
export interface QA {
  /** The question, phrased the way people ask an assistant. */
  q: string;
  /** The answer, in plain text. */
  a: string;
}

/**
 * FAQPage structured data for a page's visible questions. The strings must be the ones the
 * page shows: search engines treat structured data that differs from the page as spam.
 */
export function faqJsonLd(questions: readonly QA[]) {
  const mainEntity = questions.map(({ q, a }) => ({ '@type': 'Question', name: q, acceptedAnswer: answer(a) }));
  return { '@context': 'https://schema.org', '@type': 'FAQPage', mainEntity };
}

/** One answer, as schema.org writes it. */
const answer = (text: string) => ({ '@type': 'Answer', text });
