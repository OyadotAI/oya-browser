/**
 * Unit tests for the admin page: who sees what, its tabs, the overview,
 * issuing and revoking a license, and looking a person up and logging in as them.
 */
import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import AdminPage from '@/app/admin/page';
import { KEEL_SNAPSHOT } from '../../components/admin/keel-fixture';
import {
  adminImpersonate,
  adminIssueLicense,
  adminLicenses,
  adminLookup,
  adminOverview,
  adminRevenue,
  adminKeel,
  adminRevokeLicense,
} from '@/lib/api';

const auth = { token: 'jwt' as string | null, user: { email: 'mk@getoya.ai' } as object | null, loading: false };
vi.mock('@/components/auth-provider', () => ({ useAuth: () => auth }));
vi.mock('@/lib/api', () => ({
  adminOverview: vi.fn(),
  adminRevenue: vi.fn(),
  adminKeel: vi.fn(),
  adminLicenses: vi.fn(),
  adminIssueLicense: vi.fn(),
  adminRevokeLicense: vi.fn(),
  adminLookup: vi.fn(),
  adminImpersonate: vi.fn(),
}));

/** The counters every day carries. */
const COUNTERS = [
  ...['signups', 'active', 'agent_steps', 'cloud_seconds', 'browsers_started', 'commands'],
  ...['installers', 'updates', 'update_checks', 'new_installs'],
];

/** A day with every counter at `n`, signups at `signups`. */
const day = (d: string, n: number, signups: number) => ({
  day: d,
  ...Object.fromEntries(COUNTERS.map((c) => [c, n])),
  signups,
});

/** The same comparison for every counter. */
const each = (now: number, before: number, change: number | null) =>
  Object.fromEntries(COUNTERS.map((c) => [c, { now, before, change }]));

/** Both feeds describe the same completed UTC periods. */
const PERIODS = {
  usage: { from: '2026-02-14', to: '2026-03-15', end: '2026-03-16T00:00:00Z' },
  week: { from: '2026-03-09', to: '2026-03-15', end: '2026-03-16T00:00:00Z' },
  previousWeek: { from: '2026-03-02', to: '2026-03-08', end: '2026-03-09T00:00:00Z' },
  month: { from: '2026-03-01T00:00:00Z', to: '2026-03-16T12:00:00Z' },
  reporting: { from: '2026-03-09T12:00:00Z', to: '2026-03-16T12:00:00Z' },
};
/** Finance is measured independently, preserving currency and coverage state. */
const FINANCE = {
  generated_at: '2026-03-16T12:00:00Z',
  periods: PERIODS,
  mrr: {
    state: 'complete' as const,
    values: [{ currency: 'usd', amountMinor: 11900, minorUnitExponent: 2 }],
    reason: '',
  },
  paidInvoices: { state: 'complete' as const, values: [], previousValues: [], days: [], reason: '' },
};

beforeEach(() => {
  vi.mocked(adminRevenue).mockResolvedValue(FINANCE);
  vi.mocked(adminKeel).mockResolvedValue(KEEL_SNAPSHOT);
});

/** An overview with a little of everything. */
const OVERVIEW = {
  version: 2,
  generated_at: '2026-03-16T12:00:00Z',
  periods: PERIODS,
  growth: {
    days: [day('2026-03-14', 0, 2), day('2026-03-15', 3, 4)],
    week: { ...each(6, 4, 50), signups: { now: 6, before: 0, change: null } },
    day: each(1, 2, -50),
    reach: { today: 7, week: 31, month: 88 },
  },
  accounts: {
    total: 12,
    signups: [{ day: '2026-03-14', count: 3 }],
    byPlan: { developer: 2 },
    pastDue: 1,
    statuses: [{ plan: 'developer', active: 2, trialing: 1, past_due: 1 }],
  },
  top: { cloud: [{ userId: 'u1', email: 'ana@example.com', cloud_seconds: 7200, agent_steps: 40 }], steps: [] },
  installs: {
    total: 1,
    active: 1,
    overCap: 1,
    list: [
      {
        install_id: '6ed27761-8487',
        version: '1.0.135',
        browsers: 2,
        peak_cloud: 9,
        license_id: null,
        pings: 4,
        last_seen: '2026-03-14T00:00:00Z',
      },
    ],
  },
  downloads: [{ day: '2026-03-14', kind: 'installer', platform: 'mac', count: 5 }],
  fleet: {
    scope: 'responding_server' as const,
    generated_at: '2026-03-16T12:00:00Z',
    total: 3,
    cloud: 1,
    byProvider: { 'oya-cloud': 1, 'oya-desktop': 2 },
  },
};

/** A license as listed. */
const LICENSE = {
  id: 'L1',
  licensee: 'Acme',
  max_concurrent: 20,
  expires_at: '2027-01-01T00:00:00Z',
  created_by: 'mk@getoya.ai',
  revoked_at: null,
};

/** Loads the page with the overview and these licenses. */
function loaded(licenses = [LICENSE]) {
  vi.mocked(adminOverview).mockResolvedValue(OVERVIEW);
  vi.mocked(adminLicenses).mockResolvedValue({ licenses });
  render(<AdminPage />);
}

/** A person as the lookup answers. */
const found = (email: string, extra = {}) => ({
  generated_at: '2026-03-16T12:00:00Z',
  granularity: 'hourly_buckets' as const,
  profile: { id: `u-${email}`, email },
  standing: { plan: 'free', status: null, since: '2026-03-10T00:00:00Z' },
  used: {},
  subscription: null,
  keys: [],
  ...extra,
});

/** Opens a tab by its label. */
const openTab = async (name: string) => fireEvent.click(await screen.findByRole('tab', { name }));

/** Types an email in the top bar and looks it up. */
async function search(email: string) {
  fireEvent.change(await screen.findByLabelText('Customer email'), { target: { value: email } });
  fireEvent.click(screen.getByText('Look up'));
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.unstubAllGlobals();
  sessionStorage.clear();
  window.history.replaceState(null, '', '/');
  Object.assign(auth, { token: 'jwt', user: { email: 'mk@getoya.ai' }, loading: false });
});

describe('AdminPage', () => {
  it('opens on the overview: headline numbers, each counter’s week and day change, and the days folded away', async () => {
    loaded();
    expect((await screen.findByRole('tab', { name: 'Overview' })).getAttribute('aria-selected')).toBe('true');
    expect(screen.getByText('88 distinct accounts in 30 completed days', { exact: false })).toBeTruthy();
    expect(screen.getByText('USD 119.00')).toBeTruthy();
    expect(screen.getByText('Subscription status · now')).toBeTruthy();
    expect(screen.queryByText('Paying: 2 developer')).toBeNull();
    expect(screen.getAllByText(/\+50%/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/-50%/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/new/).length).toBeGreaterThan(0);
    const fold = screen.getByText('Day by day (UTC)').closest('details')!;
    expect(fold.open).toBe(false);
    expect(
      within(fold)
        .getAllByText(/^2026-03-1[45]$/)
        .map((n) => n.textContent),
    ).toEqual(['2026-03-15', '2026-03-14']);
  });

  it('calls out a failed payment, an unlicensed install over the cap and an unreadable Stripe', async () => {
    vi.mocked(adminOverview).mockResolvedValue(OVERVIEW);
    vi.mocked(adminRevenue).mockRejectedValue(new Error('Stripe: down'));
    vi.mocked(adminLicenses).mockResolvedValue({ licenses: [] });
    render(<AdminPage />);
    expect(await screen.findByText(/Finance could not be loaded. Stripe: down/)).toBeTruthy();
    expect(screen.getByText('1 subscriptions past due')).toBeTruthy();
    expect(screen.getByText(/above the free cap/)).toBeTruthy();
  });

  it('opens the tab the address names, and keeps the open tab in the address', async () => {
    window.history.replaceState(null, '', '/admin?tab=fleet');
    loaded();
    expect(await screen.findByText('oya-desktop')).toBeTruthy();
    await openTab('Self-hosted');
    expect(window.location.search).toBe('?tab=self-hosted');
    expect(screen.getByText('6ed27761-8487')).toBeTruthy();
    expect(screen.getByText('Acme')).toBeTruthy();
    expect(screen.queryByText('oya-desktop')).toBeNull();
  });

  it('asks a signed-out visitor to sign in', () => {
    Object.assign(auth, { token: null, user: null });
    render(<AdminPage />);
    expect(screen.getByText('Sign in')).toBeTruthy();
    expect(adminOverview).not.toHaveBeenCalled();
  });

  it('loads Keel independently when unrelated overview data fails', async () => {
    window.history.replaceState(null, '', '/admin?tab=keel');
    vi.mocked(adminOverview).mockRejectedValue(new Error('Overview data unavailable'));
    vi.mocked(adminLicenses).mockRejectedValue(new Error('License service unavailable'));
    vi.mocked(adminKeel).mockResolvedValue(KEEL_SNAPSHOT);
    render(<AdminPage />);
    expect(await screen.findByText('117 installations seen in total.')).toBeTruthy();
    expect(screen.getByRole('tab', { name: 'Keel' }).getAttribute('aria-selected')).toBe('true');
    expect(window.location.search).toBe('?tab=keel');
    await openTab('Overview');
    expect(await screen.findByText(/Overview data unavailable/)).toBeTruthy();
    expect(screen.getByRole('tab', { name: 'Keel' })).toBeTruthy();
  });

  it('says the server’s refusal to anyone who is not an admin', async () => {
    vi.mocked(adminOverview).mockRejectedValue(new Error('Admins only'));
    vi.mocked(adminLicenses).mockResolvedValue({ licenses: [] });
    render(<AdminPage />);
    expect(await screen.findByText('Admins only')).toBeTruthy();
  });

  it('issues a license and shows its key once, then reloads the list', async () => {
    vi.mocked(adminIssueLicense).mockResolvedValue({ ...LICENSE, key: 'THE-KEY' });
    loaded([]);
    await openTab('Self-hosted');
    fireEvent.change(screen.getByPlaceholderText('Company name'), { target: { value: 'Acme' } });
    fireEvent.click(screen.getByText('Issue'));
    expect(await screen.findByDisplayValue('THE-KEY')).toBeTruthy();
    expect(vi.mocked(adminIssueLicense).mock.calls[0][1]).toMatchObject({ licensee: 'Acme', maxConcurrent: 20 });
    // The reload runs in an effect after the key shows, so it is waited for, not assumed.
    await waitFor(() => expect(adminLicenses).toHaveBeenCalledTimes(2));
  });

  it('says why a license could not be issued', async () => {
    vi.mocked(adminIssueLicense).mockRejectedValue(new Error('licensee is required'));
    loaded([]);
    await openTab('Self-hosted');
    fireEvent.click(screen.getByText('Issue'));
    expect(await screen.findByText('licensee is required')).toBeTruthy();
  });

  it('revokes a license', async () => {
    vi.mocked(adminRevokeLicense).mockResolvedValue({});
    loaded();
    await openTab('Self-hosted');
    fireEvent.click(screen.getByText('Revoke'));
    await vi.waitFor(() => expect(adminRevokeLicense).toHaveBeenCalledWith('jwt', 'L1'));
  });

  it('searches from the top bar, opening Customers with the plan, keys and Stripe customer', async () => {
    vi.mocked(adminLookup).mockResolvedValue(
      found('ana@example.com', {
        standing: { plan: 'developer', status: 'active', since: '2026-03-10T00:00:00Z' },
        used: { cloud_seconds: 3600, agent_steps: 7 },
        subscription: { stripe_customer_id: 'cus_1' },
        keys: [{ prefix: 'abc', label: 'CI', created_at: '2026-03-01T00:00:00Z', last_used_at: null }],
      }),
    );
    loaded();
    await search(' ana@example.com ');
    expect(await screen.findByText('developer (active)')).toBeTruthy();
    expect(screen.getByRole('tab', { name: 'Customers' }).getAttribute('aria-selected')).toBe('true');
    expect(screen.getByText('abc…')).toBeTruthy();
    expect(screen.getByText('Open in Stripe').getAttribute('href')).toContain('cus_1');
    expect(adminLookup).toHaveBeenCalledWith('jwt', 'ana@example.com');
  });

  it('looks up one of the heaviest users when clicked', async () => {
    vi.mocked(adminLookup).mockResolvedValue(found('ana@example.com'));
    loaded();
    await openTab('Customers');
    expect(screen.getByText('2.0')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'ana@example.com' }));
    await vi.waitFor(() => expect(adminLookup).toHaveBeenCalledWith('jwt', 'ana@example.com'));
    expect((screen.getByLabelText('Customer email') as HTMLInputElement).value).toBe('ana@example.com');
  });

  it('sends an admin without a second factor to the code page, and back here after', async () => {
    const assign = vi.fn();
    vi.stubGlobal('location', { ...window.location, assign });
    vi.mocked(adminOverview).mockRejectedValue(
      Object.assign(new Error('Sign in with a second factor'), { code: 'mfa_required' }),
    );
    vi.mocked(adminLicenses).mockResolvedValue({ licenses: [] });
    render(<AdminPage />);
    await vi.waitFor(() => expect(assign).toHaveBeenCalledWith('/account/mfa?next=%2Fadmin'));
  });

  it('logs in as the person found: stores the token and opens their dashboard', async () => {
    const replace = vi.fn();
    vi.stubGlobal('location', { ...window.location, replace });
    vi.mocked(adminLookup).mockResolvedValue(found('ana@example.com'));
    vi.mocked(adminImpersonate).mockResolvedValue({ impersonate_token: 'imp', email: 'ana@example.com' });
    loaded();
    await search('ana@example.com');
    fireEvent.click(await screen.findByText('Login as'));
    await vi.waitFor(() => expect(replace).toHaveBeenCalledWith('/dashboard'));
    expect(adminImpersonate).toHaveBeenCalledWith('jwt', 'u-ana@example.com');
    expect(JSON.parse(sessionStorage.getItem('oya_impersonation') || '{}')).toEqual({
      token: 'imp',
      email: 'ana@example.com',
    });
  });

  it('says when Login as is refused', async () => {
    vi.mocked(adminLookup).mockResolvedValue(found('b@getoya.ai'));
    vi.mocked(adminImpersonate).mockRejectedValue(new Error('Cannot log in as another admin'));
    loaded();
    await search('b@getoya.ai');
    fireEvent.click(await screen.findByText('Login as'));
    expect(await screen.findByText('Cannot log in as another admin')).toBeTruthy();
  });

  it('says when nobody has that email', async () => {
    vi.mocked(adminLookup).mockRejectedValue(new Error('Account not found'));
    loaded();
    await search('nobody@example.com');
    expect(await screen.findByText('Account not found')).toBeTruthy();
  });
});

it('moves tab focus with arrows and activates only with Enter', async () => {
  loaded();
  const user = userEvent.setup();
  const overview = await screen.findByRole('tab', { name: 'Overview' });
  overview.focus();
  await user.keyboard('{ArrowRight}');
  expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'Customers' }));
  expect(overview.getAttribute('aria-selected')).toBe('true');
  await user.keyboard('{Enter}');
  expect(screen.getByRole('tab', { name: 'Customers' }).getAttribute('aria-selected')).toBe('true');
  expect(window.location.search).toBe('?tab=customers');
  expect(screen.getAllByRole('tab').filter((tab) => tab.tabIndex === 0)).toHaveLength(1);
});

it('preserves a selected customer across core refresh failure', async () => {
  vi.mocked(adminLookup).mockResolvedValue(found('ana@example.com'));
  loaded();
  await search('ana@example.com');
  expect(await screen.findByText('ana@example.com', { selector: 'span' })).toBeTruthy();
  vi.mocked(adminOverview).mockRejectedValueOnce(new Error('Core offline'));
  fireEvent.click(screen.getByRole('button', { name: 'Refresh Customers' }));
  expect(await screen.findByText(/Core offline/)).toBeTruthy();
  expect(screen.getByText('ana@example.com', { selector: 'span' })).toBeTruthy();
});

it('clears a one-time license key immediately when the authenticated credential changes', async () => {
  window.history.replaceState(null, '', '/admin?tab=self-hosted');
  vi.mocked(adminIssueLicense).mockResolvedValue({ ...LICENSE, key: 'ONE-TIME-KEY' });
  vi.mocked(adminOverview).mockResolvedValue(OVERVIEW);
  vi.mocked(adminLicenses).mockResolvedValue({ licenses: [] });
  const view = render(<AdminPage />);
  fireEvent.click(await screen.findByText('Issue'));
  expect(await screen.findByDisplayValue('ONE-TIME-KEY')).toBeTruthy();
  auth.token = 'replacement-token';
  view.rerender(<AdminPage />);
  expect(screen.queryByDisplayValue('ONE-TIME-KEY')).toBeNull();
});
