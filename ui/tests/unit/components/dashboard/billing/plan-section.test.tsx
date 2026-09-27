/**
 * Unit tests for the plan in the account dialog: hidden where there are no
 * plans, upgrade buttons on Free, "Manage billing" on a paid plan, and a
 * refusal to open Stripe said in the dialog.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import PlanSection from '@/components/dashboard/billing/plan-section';
import { hours } from '@/components/dashboard/billing/use-plan';
import { billingCheckout, billingPortal, getBilling } from '@/lib/api';

vi.mock('@/components/auth-provider', () => ({ useAuth: () => ({ token: 'jwt' }) }));
vi.mock('@/lib/api', () => ({ getBilling: vi.fn(), billingCheckout: vi.fn(), billingPortal: vi.fn() }));

/** A plan as the server answers it. */
const plan = (name: string, extra = {}) => ({
  enabled: true,
  plan: name,
  status: name === 'free' ? null : 'active',
  included: { cloudSeconds: 3600, steps: 500 },
  used: { cloud_seconds: 1800, agent_steps: 12 },
  ...extra,
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

describe('PlanSection', () => {
  it('shows nothing on a self-hosted server, which has no plans', async () => {
    vi.mocked(getBilling).mockResolvedValue({ enabled: false });
    const { container } = render(<PlanSection open />);
    await Promise.resolve();
    expect(container.innerHTML).toBe('');
  });

  it('shows a Free person their use this month and the plans to upgrade to', async () => {
    vi.mocked(getBilling).mockResolvedValue(plan('free'));
    render(<PlanSection open />);
    expect(await screen.findByText('Developer $20/mo')).toBeTruthy();
    expect(screen.getByText('Startup $99/mo')).toBeTruthy();
    expect(screen.getByText('0.5 / 1')).toBeTruthy();
    expect(screen.getByText('12 / 500')).toBeTruthy();
  });

  it('opens Checkout for the plan clicked', async () => {
    vi.mocked(getBilling).mockResolvedValue(plan('free'));
    vi.mocked(billingCheckout).mockRejectedValue(new Error('The startup plan is not on sale yet.'));
    render(<PlanSection open />);
    fireEvent.click(await screen.findByText('Startup $99/mo'));
    expect(await screen.findByText('The startup plan is not on sale yet.')).toBeTruthy();
    expect(billingCheckout).toHaveBeenCalledWith('jwt', 'startup');
  });

  it('offers a paid person the billing page instead, and says when a payment failed', async () => {
    vi.mocked(getBilling).mockResolvedValue(plan('developer', { status: 'past_due' }));
    vi.mocked(billingPortal).mockRejectedValue(new Error('Could not open billing'));
    render(<PlanSection open />);
    fireEvent.click(await screen.findByText('Manage billing'));
    expect(await screen.findByText('Could not open billing')).toBeTruthy();
    expect(screen.getByText(/payment failed/)).toBeTruthy();
  });

  it('loads nothing while the dialog is closed', () => {
    render(<PlanSection open={false} />);
    expect(getBilling).not.toHaveBeenCalled();
  });

  it('shows hours to one decimal', () => {
    expect(hours(5400)).toBe(1.5);
    expect(hours()).toBe(0);
  });
});
