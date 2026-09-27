/**
 * Unit tests for the billing page: the plan and its period, use against what
 * the plan includes, the next invoice, the invoice history, and the ways out
 * to Stripe.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import BillingPage from '@/app/dashboard/billing/page';
import { billingCheckout, billingInvoices, billingPortal, billingUpcoming, getBilling } from '@/lib/api';

const auth = { token: 'jwt' as string | null, user: { email: 'ana@example.com' } as object | null, loading: false };
vi.mock('@/components/auth-provider', () => ({ useAuth: () => auth }));
vi.mock('@/lib/api', () => ({
  getBilling: vi.fn(),
  billingInvoices: vi.fn(),
  billingUpcoming: vi.fn(),
  billingCheckout: vi.fn(),
  billingPortal: vi.fn(),
}));

/** A Developer plan halfway through its hours. */
const DEVELOPER = {
  enabled: true,
  plan: 'developer',
  status: 'active',
  since: '2026-03-10T00:00:00Z',
  until: '2026-04-10T00:00:00Z',
  included: { cloudSeconds: 360000, steps: 5000, proxyBytes: 1073741824, llmMicroUsd: 0, overage: true },
  used: {
    cloud_seconds: 180000,
    agent_steps: 6000,
    residential_proxy_bytes: 536870912,
    hosted_llm_microusd: 1_500_000,
  },
};

/** Loads the page with this plan, these invoices and this next invoice. */
function loaded(billing: object = DEVELOPER, invoices: object[] = [], upcoming: object | null = null) {
  vi.mocked(getBilling).mockResolvedValue(billing);
  vi.mocked(billingInvoices).mockResolvedValue({ invoices });
  vi.mocked(billingUpcoming).mockResolvedValue({ upcoming });
  render(<BillingPage />);
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  Object.assign(auth, { token: 'jwt', user: { email: 'ana@example.com' }, loading: false });
});

describe('BillingPage', () => {
  it('shows the plan, when it renews, and this period’s use against what it includes', async () => {
    loaded();
    expect(await screen.findByText('developer')).toBeTruthy();
    expect(screen.getByText(/^Renews /)).toBeTruthy();
    expect(screen.getByText('50 of 100 hours')).toBeTruthy();
    expect(screen.getByText('6,000 of 5,000 steps')).toBeTruthy();
    expect(screen.getByText('0.5 of 1 GB')).toBeTruthy();
    expect(screen.getByText('$1.50')).toBeTruthy();
  });

  it('shows the next invoice line by line, and the invoices already issued with their links', async () => {
    const upcoming = {
      total: 2350,
      currency: 'usd',
      date: '2026-04-10T00:00:00Z',
      lines: [{ description: '1000 × Agent steps', quantity: 1000, amount: 200 }],
    };
    const invoices = [
      {
        id: 'in_1',
        number: 'OYA-1',
        status: 'paid',
        created: '2026-03-10T00:00:00Z',
        currency: 'usd',
        total: 2000,
        url: 'https://pay/in_1',
        pdf: 'https://pdf/in_1',
      },
    ];
    loaded(DEVELOPER, invoices, upcoming);
    expect(await screen.findByText(/^Next invoice: \$23\.50/)).toBeTruthy();
    expect(screen.getByText('1000 × Agent steps')).toBeTruthy();
    expect(screen.getByText('OYA-1')).toBeTruthy();
    expect(screen.getByText('PDF').getAttribute('href')).toBe('https://pdf/in_1');
  });

  it('offers a paying person the Stripe page for card, receipts and cancelling', async () => {
    vi.mocked(billingPortal).mockRejectedValue(new Error('Could not open billing'));
    loaded();
    fireEvent.click(await screen.findByText('Change card, see receipts or cancel'));
    expect(await screen.findByText('Could not open billing')).toBeTruthy();
  });

  it('offers a Free person the plans, and says why Checkout could not open', async () => {
    vi.mocked(billingCheckout).mockRejectedValue(new Error('The startup plan is not on sale yet.'));
    loaded({
      ...DEVELOPER,
      plan: 'free',
      status: null,
      until: null,
      included: { cloudSeconds: 3600, steps: 500, proxyBytes: 0, llmMicroUsd: 500000, overage: false },
    });
    fireEvent.click(await screen.findByText('Startup $99/mo'));
    expect(await screen.findByText('The startup plan is not on sale yet.')).toBeTruthy();
    expect(screen.getByText(/^This month since /)).toBeTruthy();
  });

  it('says so when the payment failed', async () => {
    loaded({ ...DEVELOPER, status: 'past_due' });
    expect(await screen.findByText(/last payment failed/)).toBeTruthy();
  });

  it('asks a visitor with no account to sign in', () => {
    Object.assign(auth, { token: null, user: null });
    render(<BillingPage />);
    expect(screen.getByText('Sign in')).toBeTruthy();
  });

  it('says a self-hosted server has no plans, and says a failure to load', async () => {
    loaded({ enabled: false });
    expect(await screen.findByText(/self-hosted/)).toBeTruthy();
    cleanup();
    vi.mocked(getBilling).mockRejectedValue(new Error('Could not load your plan'));
    render(<BillingPage />);
    expect(await screen.findByText('Could not load your plan')).toBeTruthy();
  });
});
