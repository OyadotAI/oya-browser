/**
 * Unit tests for the dedicated /pricing page: canonical metadata, zero-JS plan
 * cards with h2 hierarchy, always-visible FAQ and matching JSON-LD.
 */
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import PricingPage, { metadata } from '@/app/pricing/page';
import { PRICING_FAQ, PUBLIC_PLANS } from '@/app/_home/pricing-content';
import { SITE_URL } from '@/lib/site';

afterEach(cleanup);

describe('/pricing page', () => {
  it('exposes canonical /pricing metadata and page-specific cards', () => {
    expect(metadata.title).toBe('Pricing');
    expect(metadata.alternates?.canonical).toBe('/pricing');
    expect(String(metadata.description).length).toBeLessThanOrEqual(160);
    expect(metadata.openGraph).toMatchObject({ url: '/pricing' });
  });

  it('renders one h1, h2 headings for all published plans and every FAQ answer visible without disclosure', () => {
    const { container } = render(<PricingPage />);
    expect(screen.getAllByRole('heading', { level: 1 }).map((h) => h.textContent)).toEqual(['Pricing']);
    for (const plan of PUBLIC_PLANS) {
      const card = within(screen.getByRole('article', { name: plan.name }));
      expect(card.getByRole('heading', { level: 2, name: plan.name })).toBeTruthy();
      expect(card.getByText(plan.price, { exact: false })).toBeTruthy();
    }
    expect(PRICING_FAQ).toHaveLength(4);
    for (const { q, a } of PRICING_FAQ) {
      expect(screen.getByRole('heading', { level: 3, name: q })).toBeTruthy();
      expect(screen.getByText(a)).toBeTruthy();
    }
    expect(container.querySelector('#faq [hidden]')).toBeNull();
  });

  it('emits BreadcrumbList and FAQPage structured data matching the visible questions verbatim', () => {
    const { container } = render(<PricingPage />);
    const scripts = container.querySelectorAll('script[type="application/ld+json"]');
    expect(scripts).toHaveLength(1);
    const graph = JSON.parse(scripts[0].textContent ?? '')['@graph'] as Record<string, unknown>[];
    const crumbs = graph.find((n) => n['@type'] === 'BreadcrumbList') as {
      itemListElement: { name: string; item: string }[];
    };
    expect(crumbs.itemListElement.map((i) => [i.name, i.item])).toEqual([
      ['Home', `${SITE_URL}/`],
      ['Pricing', `${SITE_URL}/pricing`],
    ]);
    const faq = graph.find((n) => n['@type'] === 'FAQPage') as {
      mainEntity: { name: string; acceptedAnswer: { text: string } }[];
    };
    expect(faq.mainEntity.map((item) => [item.name, item.acceptedAnswer.text])).toEqual(
      PRICING_FAQ.map(({ q, a }) => [q, a]),
    );
  });
});
