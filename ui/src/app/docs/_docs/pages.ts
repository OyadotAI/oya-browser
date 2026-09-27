/**
 * The docs' parts, in page order. The HTML page renders one <section> per
 * part, and each part also has a Markdown twin at /docs/<slug>.md cut from
 * that section, so this list is the one place both read from.
 */

/** One part of the docs. */
export interface DocPage {
  /** Its name in /docs/<slug>.md and on its <section data-doc-page>. */
  slug: string;
  /** Its title, the first line of its Markdown. */
  title: string;
}

/** Every part, in the order the page shows them. */
export const DOC_PAGES: DocPage[] = [
  { slug: 'getting-started', title: 'Get started' },
  { slug: 'playbooks', title: 'Playbooks' },
  { slug: 'sdk', title: 'SDK' },
  { slug: 'cli', title: 'CLI' },
  { slug: 'mcp', title: 'MCP setup' },
  { slug: 'mcp-tools', title: 'MCP tools' },
  { slug: 'identity', title: 'Identity' },
  { slug: 'anonymity', title: 'Stealth and anonymity' },
  { slug: 'self-hosting', title: 'Self-hosting' },
  { slug: 'control-plane', title: 'Control plane' },
  { slug: 'dashboard', title: 'Dashboard' },
  { slug: 'api', title: 'REST and WebSocket API' },
];

/** The part a slug names, if any. */
export function docPage(slug: string): DocPage | undefined {
  return DOC_PAGES.find((page) => page.slug === slug);
}
