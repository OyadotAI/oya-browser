/** Public plan choices expose their existing costs and navigate without subscribing a visitor. */
import { afterEach, expect, it } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import { Pricing } from '@/app/_home/pricing';
import { FAQ, foundersCall } from '@/app/_home/content';
import { BEST_INFRA } from '@/app/_answers/content';
import { BROWSERBASE, OYA } from '@/app/_answers/compare';
import nextConfig from '../../../../next.config';

afterEach(cleanup);

it('keeps public Oya plan prices consistent without changing a competitor’s published price', () => {
  const answers = [
    FAQ.find(({ q }) => q === 'How much does it cost?')?.a,
    BEST_INFRA.faq?.find(({ q }) => q === 'How much does Oya cost?')?.a,
  ];
  for (const answer of answers) {
    expect(answer).toContain('Developer is $500 a month');
    expect(answer).toContain('Startup is $2,500 a month');
  }
  expect(OYA.cells.price).toContain('$500 a month');
  expect(BROWSERBASE.cells.price).toBe('1 browser hour free; $20 a month for 100 hours');
});

it('shows prices, monthly allowances and additional usage without requiring a disclosure', () => {
  render(<Pricing />);
  for (const [name, price, allowances] of [
    ['Free', '$0', ['500 agent steps a month', '1 cloud browser hour']],
    [
      'Developer',
      '$500',
      ['100 cloud browser hours', '1 GB of proxy', '5,000 agent steps', 'Additional browser hours: $0.10/hour'],
    ],
    [
      'Startup',
      '$2,500',
      ['500 cloud browser hours', '5 GB of proxy', '50,000 agent steps', 'Additional browser hours: $0.08/hour'],
    ],
  ] as const) {
    const plan = within(screen.getByRole('article', { name }));
    expect(plan.getByText(price, { exact: false })).toBeTruthy();
    for (const allowance of allowances) expect(plan.getByText(allowance)).toBeTruthy();
  }
  expect(
    screen.getByText(/Paid-plan usage beyond these allowances: \$8\/GB of proxy and \$0.002\/agent step/),
  ).toBeTruthy();
  expect(screen.getByText('Self-hosting is free up to 5 cloud browsers at once.')).toBeTruthy();
  expect(screen.getByText(/For protected health information \(PHI\), use Oya Cloud or the enterprise/)).toBeTruthy();
  expect(screen.queryByRole('button')).toBeNull();
});

it('uses existing download, dashboard and sales destinations without starting checkout', () => {
  render(<Pricing />);
  expect(screen.getAllByRole('article').map((plan) => plan.getAttribute('aria-labelledby'))).toEqual([
    'plan-Free',
    'plan-Developer',
    'plan-Startup',
    'plan-Enterprise',
  ]);
  const enterprise = within(screen.getByRole('article', { name: 'Enterprise' }));
  expect(enterprise.getByText('Custom', { exact: false })).toBeTruthy();
  expect(enterprise.getByText('Self-hosting above 5 cloud browsers')).toBeTruthy();
  expect(enterprise.getByRole('link', { name: 'Book a meeting' }).getAttribute('href')).toBe(foundersCall);
  for (const [name, href] of [
    ['Download free', '/#download'],
    ['Choose Developer', '/dashboard'],
    ['Choose Startup', '/dashboard'],
    ['Book a meeting', foundersCall],
    ['Review security and compliance', '/trust'],
  ])
    expect(screen.getByRole('link', { name }).getAttribute('href')).toBe(href);
});

it('redirects a direct pricing visit to the visible plans anchor', async () => {
  expect(await nextConfig.redirects!()).toContainEqual({
    source: '/pricing',
    destination: '/#pricing',
    permanent: false,
  });
});
