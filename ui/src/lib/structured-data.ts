/**
 * Shared JSON-LD builders for public pages: one place for schema.org nodes,
 * safe script escaping and published pricing offers so every route agrees on
 * the same organization and software identity.
 */
import { SITE_DESCRIPTION, SITE_NAME, SITE_URL, type QA } from './site';
import { PUBLIC_PLANS, type PublicPlan } from '@/app/_home/pricing-content';

/** Breadcrumb positions are 1-based in schema.org ListItem. */
const BREADCRUMB_OFFSET = 1;

/** Billing period label used by recurring monthly plans in PUBLIC_PLANS. */
const MONTHLY_PERIOD = '/month';

/** ISO 4217 currency code for published USD plans. */
const USD = 'USD';

/** UN/CEFACT unit code for monthly recurring billing in schema.org. */
const MONTH_UNIT_CODE = 'MON';

/** Reference to the root Organization node. */
const ORG_REF = { '@id': `${SITE_URL}/#organization` } as const;

/** Static defaults shared by every TechArticle node. */
const ARTICLE_BASE = { '@type': 'TechArticle', author: ORG_REF, publisher: ORG_REF } as const;

/** Static defaults shared by every published pricing Offer node. */
const OFFER_BASE = { '@type': 'Offer', priceCurrency: USD, url: `${SITE_URL}/pricing` } as const;

/** Verified public profiles for Oya Browser's Organization entity graph. */
export const ORGANIZATION_SAME_AS = [
  'https://github.com/OyadotAI/oya-browser',
  'https://www.npmjs.com/package/@oya-ai/browser',
  'https://pypi.org/project/oya-browser/',
  'https://registry.modelcontextprotocol.io/v0/servers?search=io.github.OyadotAI/oya-browser',
  'https://getoya.ai',
  'https://oya.dev',
  'https://discord.gg/wqSeXJPygn',
  'https://mcptry.com/servers/io.github.OyadotAI/oya-browser',
  'https://glama.ai/mcp/servers/OyadotAI/oya-browser',
  'https://www.youtube.com/@oyaAI',
  'https://x.com/oyadotai',
  'https://x.com/OyaAgents',
  'https://dev.to/oya-browser',
  'https://oyabrowser.hashnode.dev',
  'https://www.reddit.com/user/OyaAI/',
] as const;

/** Capabilities listed in the root SoftwareApplication schema node. */
const SOFTWARE_FEATURES = [
  'Playbooks: record an agent run once, replay it with new inputs and no model in the loop',
  'Every playbook exports as a readable Playwright module you own',
  'A changed page is repaired into a draft playbook a person promotes',
  'Automation inside the browser process, not attached over the debugging protocol',
  'Desktop sign-in that carries cookies and localStorage to every browser on that persona',
  'Personas: fingerprint, cookies and proxy as one identity that never changes',
  'Hash-chained audit trail and host allow-listing',
  'Live view with human takeover mid-run for CAPTCHA and 2FA',
  "Agents set themselves up: an SDK call for their own key, one to pair the person's browser and logins",
  'MCP endpoint for Claude Code, Claude Desktop and Cursor; a skill for OpenClaw, Codex and any SKILL.md agent',
  'REST, WebSocket, JavaScript SDK, CLI and a CDP URL Playwright connects to',
  'Runs on Oya Cloud, Browserbase, Steel, Anchor, Browser Use or your own Chrome',
  'Self-hostable with one installer command',
] as const;

/** Static fields shared by the root SoftwareApplication node. */
const SOFTWARE_BASE = {
  '@type': 'SoftwareApplication',
  '@id': `${SITE_URL}/#app`,
  name: SITE_NAME,
  applicationCategory: 'BrowserApplication',
  applicationSubCategory: 'HIPAA-compliant AI browser and agentic browser',
  keywords:
    'HIPAA-compliant browser, AI browser, agentic browser, browser for AI agents, browser automation, healthcare',
  operatingSystem: 'macOS, Windows, Linux',
  url: SITE_URL,
  downloadUrl: `${SITE_URL}/#download`,
  softwareHelp: `${SITE_URL}/docs`,
  subjectOf: { '@type': 'TechArticle', url: `${SITE_URL}/llms.txt`, name: 'Oya Browser for AI agents' },
  description: SITE_DESCRIPTION,
  featureList: [...SOFTWARE_FEATURES],
  publisher: ORG_REF,
} as const;

/** One step in a page's visible and structured breadcrumb trail. */
export interface BreadcrumbItem {
  /** Visible crumb label. */
  name: string;
  /** Site-relative path starting with '/'. */
  path: string;
}

/** Input for a TechArticle schema.org node on editorial and answer pages. */
export interface TechArticleInput {
  /** Article headline. */
  headline: string;
  /** Plain-text summary matching the page description. */
  description: string;
  /** Canonical path starting with '/'. */
  path: string;
  /** ISO YYYY-MM-DD publish date, when known. */
  datePublished?: string;
  /** ISO YYYY-MM-DD editorial modification date, when known; source checks and experiment dates do not qualify. */
  dateModified?: string;
}

/** Serializes JSON-LD safely for inline script tags by escaping '<' so strings cannot close </script>. */
export const jsonLd = (data: object): string => JSON.stringify(data).replace(/</g, '\\u003c');

/** Wraps one or more schema.org nodes into a single @graph document. */
export const jsonLdGraph = (nodes: readonly object[]) => ({
  '@context': 'https://schema.org',
  '@graph': nodes,
});

/** Builds a BreadcrumbList node from site-relative items. */
export const breadcrumbNode = (items: readonly BreadcrumbItem[]) => ({
  '@type': 'BreadcrumbList',
  itemListElement: items.map(({ name, path }, index) => ({
    '@type': 'ListItem',
    position: index + BREADCRUMB_OFFSET,
    name,
    item: `${SITE_URL}${path}`,
  })),
});

/** Optional publish and modified date fields for a TechArticle node. */
const articleDates = (datePublished?: string, dateModified?: string) => ({
  ...(datePublished ? { datePublished } : {}),
  ...(dateModified ? { dateModified } : {}),
});

/** Canonical URL and mainEntityOfPage pair for a TechArticle node. */
const articleIdentity = (path: string) => {
  const url = `${SITE_URL}${path}`;
  return { url, mainEntityOfPage: { '@type': 'WebPage', '@id': url } };
};

/** Builds a TechArticle node linked to the site's Organization and WebSite ids. */
export function techArticleNode({ headline, description, path, datePublished, dateModified }: TechArticleInput) {
  return {
    ...ARTICLE_BASE,
    headline,
    description,
    ...articleIdentity(path),
    ...articleDates(datePublished, dateModified),
  };
}

/** Builds a schema.org FAQPage node (without @context, for use inside @graph). */
export const faqNode = (questions: readonly QA[]) => ({
  '@type': 'FAQPage',
  mainEntity: questions.map(({ q, a }) => ({
    '@type': 'Question',
    name: q,
    acceptedAnswer: { '@type': 'Answer', text: a },
  })),
});

/** Extracts a plain numeric USD string from a published price label like "$500" or "$2,500", or null for "Custom". */
function numericPrice(price: string): string | null {
  const match = /^\$([\d,]+(?:\.\d+)?)$/.exec(price.trim());
  return match ? match[1].replace(/,/g, '') : null;
}

/** Builds a monthly UnitPriceSpecification when the plan recurs monthly. */
const recurringSpec = (price: string, period: string) =>
  period === MONTHLY_PERIOD
    ? {
        priceSpecification: { '@type': 'UnitPriceSpecification', price, priceCurrency: USD, unitCode: MONTH_UNIT_CODE },
      }
    : {};

/** Converts one numeric public plan into a schema.org Offer, or null when pricing is custom. */
function planOffer(plan: PublicPlan): object | null {
  const price = numericPrice(plan.price);
  if (price === null) return null;
  const spec = recurringSpec(price, plan.period);
  return { ...OFFER_BASE, name: plan.name, description: plan.description, price, ...spec };
}

/** Derives schema.org Offer entries from published numeric plans without inventing custom enterprise prices. */
export const softwareOffers = (plans: readonly PublicPlan[]): object[] =>
  plans.map(planOffer).filter((offer): offer is object => offer !== null);

/** Root WebSite node. */
const websiteNode = () => ({
  '@type': 'WebSite',
  '@id': `${SITE_URL}/#website`,
  url: SITE_URL,
  name: SITE_NAME,
  description: SITE_DESCRIPTION,
  publisher: ORG_REF,
});

/** Root Organization node with verified public profiles. */
const organizationNode = () => ({
  '@type': 'Organization',
  '@id': `${SITE_URL}/#organization`,
  name: 'Oya',
  url: SITE_URL,
  logo: `${SITE_URL}/icon.png`,
  sameAs: [...ORGANIZATION_SAME_AS],
});

/** Root SoftwareApplication node with OS support, download link and published offers. */
const softwareNode = () => ({ ...SOFTWARE_BASE, offers: softwareOffers(PUBLIC_PLANS) });

/** The site-wide WebSite, Organization and SoftwareApplication graph emitted in the root layout. */
export function siteStructuredData() {
  return jsonLdGraph([websiteNode(), organizationNode(), softwareNode()]);
}
