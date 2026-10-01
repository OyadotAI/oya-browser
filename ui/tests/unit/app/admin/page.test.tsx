/**
 * Unit tests for the admin page: who sees what, the overview, issuing and
 * revoking a license, and looking a person up.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import AdminPage from '@/app/admin/page';
import { adminIssueLicense, adminLicenses, adminLookup, adminOverview, adminRevokeLicense } from '@/lib/api';

const auth = { token: 'jwt' as string | null, user: { email: 'mk@getoya.ai' } as object | null, loading: false };
vi.mock('@/components/auth-provider', () => ({ useAuth: () => auth }));
vi.mock('@/lib/api', () => ({
  adminOverview: vi.fn(),
  adminLicenses: vi.fn(),
  adminIssueLicense: vi.fn(),
  adminRevokeLicense: vi.fn(),
  adminLookup: vi.fn(),
}));

/** The counters every day carries. */
const COUNTERS = [
  ...['signups', 'active', 'agent_steps', 'cloud_seconds', 'browsers_started', 'commands'],
  ...['installers', 'update_checks', 'new_installs', 'revenue_cents'],
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

/** An overview with a little of everything. */
const OVERVIEW = {
  growth: {
    days: [day('2026-03-14', 0, 2), day('2026-03-15', 3, 4)],
    week: { ...each(6, 4, 50), signups: { now: 6, before: 0, change: null } },
    day: each(1, 2, -50),
    reach: { today: 7, week: 31, month: 88 },
  },
  revenue: { enabled: true, mrrCents: 11900, error: '' },
  accounts: { total: 12, signups: [{ day: '2026-03-14', count: 3 }], byPlan: { developer: 2 }, pastDue: 1 },
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
  fleet: { total: 3, cloud: 1, byProvider: { 'oya-cloud': 1, 'oya-desktop': 2 } },
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

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  Object.assign(auth, { token: 'jwt', user: { email: 'mk@getoya.ai' }, loading: false });
});

describe('AdminPage', () => {
  it('shows people reached, MRR, each counter’s week and day change, and the days newest first', async () => {
    loaded();
    expect(await screen.findByText('Active, 30 days (MAU)')).toBeTruthy();
    expect(screen.getByText('88')).toBeTruthy();
    expect(screen.getByText('$119')).toBeTruthy();
    expect(screen.getAllByText(/\+50%/).length).toBeGreaterThan(0);
    expect(screen.getAllByText(/-50%/).length).toBeGreaterThan(0);
    expect(screen.getByText(/new/)).toBeTruthy();
    const table = within(screen.getByText('Day by day (UTC)').closest('section')!);
    expect(table.getAllByText(/^2026-03-1[45]$/).map((n) => n.textContent)).toEqual(['2026-03-15', '2026-03-14']);
  });

  it('says when Stripe could not be read, and still shows the rest', async () => {
    vi.mocked(adminOverview).mockResolvedValue({
      ...OVERVIEW,
      revenue: { enabled: true, mrrCents: 0, error: 'Stripe: down' },
    });
    vi.mocked(adminLicenses).mockResolvedValue({ licenses: [] });
    render(<AdminPage />);
    expect(await screen.findByText('Stripe could not be read: Stripe: down')).toBeTruthy();
    expect(screen.getByText('Active today (DAU)')).toBeTruthy();
  });

  it('asks a signed-out visitor to sign in', () => {
    Object.assign(auth, { token: null, user: null });
    render(<AdminPage />);
    expect(screen.getByText('Sign in')).toBeTruthy();
    expect(adminOverview).not.toHaveBeenCalled();
  });

  it('says the server’s refusal to anyone who is not an admin', async () => {
    vi.mocked(adminOverview).mockRejectedValue(new Error('Admins only'));
    vi.mocked(adminLicenses).mockResolvedValue({ licenses: [] });
    render(<AdminPage />);
    expect(await screen.findByText('Admins only')).toBeTruthy();
  });

  it('shows accounts, plans, installs, downloads, heaviest users and the fleet', async () => {
    loaded();
    expect(await screen.findByText('2 developer')).toBeTruthy();
    expect(screen.getByText('6ed27761')).toBeTruthy();
    expect(screen.getByText('ana@example.com')).toBeTruthy();
    expect(screen.getByText('2.0')).toBeTruthy();
    expect(screen.getByText('oya-desktop')).toBeTruthy();
    expect(screen.getByText('Acme')).toBeTruthy();
  });

  it('issues a license and shows its key once, then reloads the list', async () => {
    vi.mocked(adminIssueLicense).mockResolvedValue({ ...LICENSE, key: 'THE-KEY' });
    loaded([]);
    fireEvent.change(await screen.findByPlaceholderText('Company name'), { target: { value: 'Acme' } });
    fireEvent.click(screen.getByText('Issue'));
    expect(await screen.findByDisplayValue('THE-KEY')).toBeTruthy();
    expect(vi.mocked(adminIssueLicense).mock.calls[0][1]).toMatchObject({ licensee: 'Acme', maxConcurrent: 20 });
    expect(adminLicenses).toHaveBeenCalledTimes(2);
  });

  it('says why a license could not be issued', async () => {
    vi.mocked(adminIssueLicense).mockRejectedValue(new Error('licensee is required'));
    loaded([]);
    fireEvent.click(await screen.findByText('Issue'));
    expect(await screen.findByText('licensee is required')).toBeTruthy();
  });

  it('revokes a license', async () => {
    vi.mocked(adminRevokeLicense).mockResolvedValue({});
    loaded();
    fireEvent.click(await screen.findByText('Revoke'));
    await vi.waitFor(() => expect(adminRevokeLicense).toHaveBeenCalledWith('jwt', 'L1'));
  });

  it('looks a person up by email, with their plan, keys and Stripe customer', async () => {
    vi.mocked(adminLookup).mockResolvedValue({
      standing: { plan: 'developer', status: 'active', since: '2026-03-10T00:00:00Z' },
      used: { cloud_seconds: 3600, agent_steps: 7 },
      subscription: { stripe_customer_id: 'cus_1' },
      keys: [{ prefix: 'abc', label: 'CI', created_at: '2026-03-01T00:00:00Z', last_used_at: null }],
    });
    loaded();
    fireEvent.change(await screen.findByPlaceholderText('email@example.com'), {
      target: { value: ' ana@example.com ' },
    });
    fireEvent.click(screen.getByText('Look up'));
    expect(await screen.findByText('developer (active)')).toBeTruthy();
    expect(screen.getByText('abc…')).toBeTruthy();
    expect(screen.getByText('Open in Stripe').getAttribute('href')).toContain('cus_1');
    expect(adminLookup).toHaveBeenCalledWith('jwt', 'ana@example.com');
  });

  it('says when nobody has that email', async () => {
    vi.mocked(adminLookup).mockRejectedValue(new Error('Account not found'));
    loaded();
    fireEvent.click(await screen.findByText('Look up'));
    expect(await screen.findByText('Account not found')).toBeTruthy();
  });
});
