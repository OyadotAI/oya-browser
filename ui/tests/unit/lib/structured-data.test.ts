/**
 * Unit tests for shared JSON-LD builders and root layout metadata: entity graph
 * completeness, published offers derivation, script escaping and feed discovery.
 */
import { describe, it, expect, vi } from 'vitest';

vi.mock('next/font/google', () => ({
  DM_Sans: () => ({ variable: 'dm-sans' }),
  Archivo_Black: () => ({ variable: 'archivo-black' }),
}));

import { metadata } from '@/app/layout';
import { PUBLIC_PLANS } from '@/app/_home/pricing-content';
import { SITE_URL } from '@/lib/site';
import {
  ORGANIZATION_SAME_AS,
  breadcrumbNode,
  jsonLd,
  siteStructuredData,
  softwareOffers,
  techArticleNode,
} from '@/lib/structured-data';

describe('root layout metadata', () => {
  it('omits legacy keywords and advertises /feed.xml alongside llms.txt and openapi.json', () => {
    expect(metadata.keywords).toBeUndefined();
    expect(metadata.alternates?.types).toMatchObject({
      'text/plain': '/llms.txt',
      'application/json': '/openapi.json',
      'application/rss+xml': '/feed.xml',
    });
  });
});

describe('siteStructuredData', () => {
  it('links verified organization profiles and derives software offers from PUBLIC_PLANS', () => {
    const graph = siteStructuredData()['@graph'] as Record<string, unknown>[];
    const org = graph.find((node) => node['@type'] === 'Organization');
    const app = graph.find((node) => node['@type'] === 'SoftwareApplication');
    expect(org?.sameAs).toEqual([...ORGANIZATION_SAME_AS]);
    expect(app).toMatchObject({
      operatingSystem: 'macOS, Windows, Linux',
      downloadUrl: `${SITE_URL}/#download`,
    });
    expect(app?.offers).toEqual(softwareOffers(PUBLIC_PLANS));
  });

  it('derives numeric offers for Free, Developer and Startup and skips Custom Enterprise', () => {
    const offers = softwareOffers(PUBLIC_PLANS) as { name: string; price: string; priceCurrency: string }[];
    expect(offers.map(({ name, price, priceCurrency }) => [name, price, priceCurrency])).toEqual([
      ['Free', '0', 'USD'],
      ['Developer', '500', 'USD'],
      ['Startup', '2500', 'USD'],
    ]);
  });
});

describe('structured-data helpers', () => {
  it('escapes "<" so no string can break out of a script tag', () => {
    const serialized = jsonLd({ text: '</script><script>alert(1)</script>' });
    expect(serialized).not.toContain('</');
    expect(JSON.parse(serialized)).toEqual({ text: '</script><script>alert(1)</script>' });
  });

  it('builds 1-based breadcrumbs and optional TechArticle dates', () => {
    const crumbs = breadcrumbNode([
      { name: 'Home', path: '/' },
      { name: 'Pricing', path: '/pricing' },
    ]);
    expect(crumbs.itemListElement).toEqual([
      { '@type': 'ListItem', position: 1, name: 'Home', item: `${SITE_URL}/` },
      { '@type': 'ListItem', position: 2, name: 'Pricing', item: `${SITE_URL}/pricing` },
    ]);
    expect(techArticleNode({ headline: 'A', description: 'B', path: '/a' })).not.toHaveProperty('dateModified');
    expect(
      techArticleNode({
        headline: 'A',
        description: 'B',
        path: '/a',
        datePublished: '2026-09-20',
        dateModified: '2026-10-01',
      }),
    ).toMatchObject({ datePublished: '2026-09-20', dateModified: '2026-10-01' });
  });
});
