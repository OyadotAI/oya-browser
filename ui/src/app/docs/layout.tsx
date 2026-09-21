/**
 * The docs route's layout, which exists to give the (client-rendered) docs
 * page its own metadata.
 */
import type { PropsWithChildren } from 'react';
import type { Metadata } from 'next';
import { SITE_NAME, SITE_URL } from '@/lib/site';

/**
 * The docs page is a client component and cannot export metadata itself.
 * It is also the page most worth citing, so it gets its own title, description
 * and canonical rather than inheriting the home page's.
 */
export const metadata: Metadata = {
  title: 'Docs, API, personas, MCP and self-hosting',
  description:
    `${SITE_NAME} documentation: quickstart, REST and WebSocket APIs, the command surface ` +
    '(navigate, click, type, scroll, wait, screenshot, tabs), personas and fingerprinting, ' +
    'proxy and rotation, provider routing and failover, MCP setup for Claude Code, Claude ' +
    'Desktop and Cursor, live view, CAPTCHA and MFA handling, and self-hosting.',
  alternates: { canonical: '/docs' },
  openGraph: {
    type: 'article',
    url: `${SITE_URL}/docs`,
    title: `Docs · ${SITE_NAME}`,
    description: 'Quickstart, API reference, personas, MCP integration and self-hosting.',
  },
};

/** Renders the page as is. */
export default function DocsLayout({ children }: PropsWithChildren) {
  return children;
}
