/**
 * Unit tests for the Profiles tab: its rows, opening a profile, and the empty
 * state.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<typeof import('@/lib/api-client')>()), api: vi.fn() }));

import { api } from '@/lib/api-client';
import PersonasTab from '@/components/dashboard/personas-tab';
import { ToastProvider } from '@/components/dashboard/toast';
import type { Persona } from '@/components/dashboard/types';

const persona = {
  id: 'p1',
  name: 'ops',
  isDefault: true,
  createdAt: new Date().toISOString(),
  lastUsedAt: null,
  activeBrowsers: 2,
  maxConcurrent: 2,
  proxy: { geo: 'DE' },
  exit: null,
  prefs: null,
  fingerprint: { platform: 'MacIntel', timezone: 'UTC', locale: 'en', screen: '1x1', webgl: 'gpu' },
  mfa: { configured: true, type: 'totp' },
  login: { cookies: 1, sites: ['a.com', 'b.com'], updatedAt: null },
} as unknown as Persona;

/** Renders the tab with `personas`, loaded unless `status` says otherwise. */
function setup(personas: Persona[] = [persona], status: 'loading' | 'ready' | 'failed' = 'ready') {
  const onOpen = vi.fn();
  const refresh = vi.fn();
  const tab = (now: 'loading' | 'ready' | 'failed') => (
    <ToastProvider>
      <PersonasTab
        apiKey="k"
        browsers={[]}
        personas={personas}
        status={now}
        refresh={refresh}
        openId={null}
        onOpen={onOpen}
        onShowBrowsers={vi.fn()}
        now={0}
      />
    </ToastProvider>
  );
  const view = render(tab(status));
  return { onOpen, refresh, rerenderWith: (next: 'loading' | 'ready' | 'failed') => view.rerender(tab(next)) };
}

describe('PersonasTab', () => {
  beforeEach(() => vi.mocked(api).mockResolvedValue({ proxies: [], platforms: [], timezones: {}, locales: {} }));
  afterEach(cleanup);

  it('shows each profile with its sites, cap, device and auto exit', () => {
    setup();
    const table = screen.getByRole('table', { name: 'Profiles' });
    expect(within(table).getByText('a.com, b.com')).toBeTruthy();
    expect(within(table).getByText('default')).toBeTruthy();
    expect(within(table).getByText('macOS · 1x1')).toBeTruthy();
    expect(within(table).getByText('DE (auto)')).toBeTruthy();
    expect(within(table).getByText('totp')).toBeTruthy();
  });

  it('opens a profile when its row is clicked', async () => {
    const { onOpen } = setup();
    await userEvent.click(screen.getByRole('button', { name: 'ops' }));
    expect(onOpen).toHaveBeenCalledTimes(1);
    expect(onOpen).toHaveBeenCalledWith('p1');
  });

  it('offers to create the first profile when there are none', async () => {
    setup([]);
    expect(screen.getByText('No profiles yet')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Create one' }));
    expect(await screen.findByRole('dialog', { name: 'New profile' })).toBeTruthy();
  });

  it('says it is loading, or that it could not load, instead of "No profiles yet"', async () => {
    const view = setup([], 'loading');
    expect(screen.getByText('Loading profiles…')).toBeTruthy();
    expect(screen.queryByText('No profiles yet')).toBeNull();
    view.rerenderWith('failed');
    expect(screen.getByText('Could not load your profiles')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Try again' }));
    expect(view.refresh).toHaveBeenCalled();
  });

  it('opens the proxies dialog', async () => {
    setup();
    await userEvent.click(screen.getByRole('button', { name: 'Proxies' }));
    expect(await screen.findByRole('dialog', { name: 'Proxies' })).toBeTruthy();
  });
});
