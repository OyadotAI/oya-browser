/**
 * Unit tests for the profile drawer: saving only what changed, pinning,
 * second factors, sign-ins, clone and delete, and the running list.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<typeof import('@/lib/api-client')>()), api: vi.fn() }));
vi.mock('@/components/dashboard/config', () => ({ desktopSignInUrl: vi.fn() }));

import { api } from '@/lib/api-client';
import PersonaDrawer from '@/components/dashboard/persona-drawer';
import { ToastProvider } from '@/components/dashboard/toast';
import type { BrowserRow, Persona } from '@/components/dashboard/types';

const mockApi = vi.mocked(api);
const persona = {
  id: 'p1',
  name: 'ops',
  isDefault: false,
  createdAt: '2026-01-01T00:00:00Z',
  lastUsedAt: null,
  activeBrowsers: 0,
  maxConcurrent: 2,
  proxy: { geo: 'US' },
  exit: null,
  prefs: null,
  fingerprint: { platform: 'Win32', timezone: 'UTC', locale: 'en-US', screen: '1x1', webgl: 'gpu' },
  mfa: { configured: true, type: 'totp' },
  sites: { mfa: [{ domain: 'a.com', type: 'sms' }], credentials: [{ domain: 'b.com', username: 'bob' }] },
  login: { cookies: 0, sites: [], updatedAt: null },
} as unknown as Persona;

/** A browser running as the persona. */
const browser = (i: number) =>
  ({ id: `b${i}`, name: `browser ${i}`, persona: 'p1', health: 'ok', currentUrl: 'https://x.com' }) as BrowserRow;

/** Renders the drawer for `p` with `browsers` running. */
function setup(p: Persona = persona, browsers: BrowserRow[] = []) {
  const props = { onClose: vi.fn(), onChanged: vi.fn(), onShowBrowsers: vi.fn() };
  render(
    <ToastProvider>
      <PersonaDrawer persona={p} apiKey="k" browsers={browsers} now={Date.now()} {...props} />
    </ToastProvider>,
  );
  return props;
}

describe('PersonaDrawer', () => {
  beforeEach(() =>
    mockApi.mockImplementation(async (path: string) =>
      path === '/proxies' ? { proxies: [{ id: 'x1', label: 'home', geo: 'US', assigned: 0, maxPersonas: 1 }] } : {},
    ),
  );
  afterEach(cleanup);

  it('renders nothing without a persona', () => {
    render(
      <ToastProvider>
        <PersonaDrawer
          persona={null}
          apiKey="k"
          browsers={[]}
          now={0}
          onClose={vi.fn()}
          onChanged={vi.fn()}
          onShowBrowsers={vi.fn()}
        />
      </ToastProvider>,
    );
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('enables Save only once something saveable changed, and saves it', async () => {
    const { onChanged } = setup();
    const save = screen.getByRole('button', { name: 'Save' }) as HTMLButtonElement;
    expect(save.disabled).toBe(true);
    await userEvent.clear(screen.getByLabelText('Proxy geo hint'));
    await userEvent.type(screen.getByLabelText('Proxy geo hint'), 'de');
    await userEvent.click(save);
    await waitFor(() => expect(onChanged).toHaveBeenCalled());
    expect(mockApi).toHaveBeenCalledWith('/personas/p1', {
      key: 'k',
      method: 'PUT',
      body: { name: 'ops', maxConcurrent: 2, proxy: { geo: 'DE' } },
    });
    expect(screen.getByText('Saved')).toBeTruthy();
  });

  it('pins the persona to a proxy it offers', async () => {
    setup();
    await userEvent.selectOptions(
      screen.getByLabelText('Exit proxy'),
      await screen.findByRole('option', { name: /^home/ }),
    );
    await userEvent.click(screen.getByRole('button', { name: 'Apply' }));
    await waitFor(() =>
      expect(mockApi).toHaveBeenCalledWith('/personas/p1/proxy', { key: 'k', method: 'PUT', body: { proxyId: 'x1' } }),
    );
  });

  it('removes the default factor and a per-site factor separately', async () => {
    setup();
    const [all, site] = screen.getAllByRole('button', { name: 'Remove' });
    await userEvent.click(all);
    await waitFor(() => expect(mockApi).toHaveBeenCalledWith('/personas/p1/mfa', { key: 'k', method: 'DELETE' }));
    await userEvent.click(site);
    await waitFor(() =>
      expect(mockApi).toHaveBeenCalledWith('/personas/p1/mfa?domain=a.com', { key: 'k', method: 'DELETE' }),
    );
  });

  it('stores a factor and empties the draft even when storing fails', async () => {
    mockApi.mockImplementation(async (path: string) => {
      if (path === '/personas/p1/mfa') throw new Error('Bad seed');
      return { proxies: [] };
    });
    setup();
    const store = screen.getByRole('button', { name: 'Store factor' }) as HTMLButtonElement;
    expect(store.disabled).toBe(true);
    await userEvent.type(screen.getByLabelText('Secret'), 'SEED');
    await userEvent.click(store);
    expect(await screen.findByText('Bad seed')).toBeTruthy();
    expect((screen.getByLabelText('Secret') as HTMLInputElement).value).toBe('');
  });

  it('stores a sign-in once all three fields are filled, then clears them', async () => {
    setup();
    const store = screen.getByRole('button', { name: 'Store sign-in' }) as HTMLButtonElement;
    await userEvent.type(screen.getByLabelText('Site'), 'c.com');
    await userEvent.type(screen.getByLabelText('Username'), 'amy');
    expect(store.disabled).toBe(true);
    await userEvent.type(screen.getByLabelText('Password'), 'pw');
    await userEvent.click(store);
    await waitFor(() => expect((screen.getByLabelText('Site') as HTMLInputElement).value).toBe(''));
    expect(mockApi).toHaveBeenCalledWith('/personas/p1/credentials', {
      key: 'k',
      method: 'PUT',
      body: { domain: 'c.com', username: 'amy', password: 'pw' },
    });
  });

  it('removes a stored sign-in by its domain', async () => {
    setup();
    await userEvent.click(screen.getAllByRole('button', { name: 'Remove' })[2]);
    await waitFor(() =>
      expect(mockApi).toHaveBeenCalledWith('/personas/p1/credentials?domain=b.com', { key: 'k', method: 'DELETE' }),
    );
  });

  it('clones into a new identity and says so', async () => {
    mockApi.mockImplementation(async (path: string) =>
      String(path).endsWith('/clone') ? { name: 'ops 2' } : { proxies: [] },
    );
    setup();
    await userEvent.click(screen.getByRole('button', { name: 'Clone as new profile' }));
    expect(await screen.findByText('Created ops 2, same kind of device, new identity')).toBeTruthy();
  });

  it('deletes only after confirming, then closes', async () => {
    const { onClose } = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Delete' }));
    const confirm = screen.getByRole('dialog', { name: 'Delete ops?' });
    await userEvent.click(within(confirm).getByRole('button', { name: 'Delete persona' }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(mockApi).toHaveBeenCalledWith('/personas/p1', { key: 'k', method: 'DELETE' });
  });

  it('will not delete while its browsers run', () => {
    setup(persona, [browser(1)]);
    expect((screen.getByRole('button', { name: 'Delete' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('lists a handful of running browsers and links to the rest', async () => {
    const { onShowBrowsers } = setup(persona, [1, 2, 3, 4, 5, 6, 7, 8].map(browser));
    expect(screen.getByText('browser 6')).toBeTruthy();
    expect(screen.queryByText('browser 7')).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'and 2 more, show in fleet' }));
    expect(onShowBrowsers).toHaveBeenCalledWith('p1');
  });
});
