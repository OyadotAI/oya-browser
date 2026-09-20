/**
 * Unit tests for the proxies dialog: listing, adding, checking, and the
 * two-click remove.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<typeof import('@/lib/api-client')>()), api: vi.fn() }));

import { api } from '@/lib/api-client';
import ProxiesDialog from '@/components/dashboard/proxies-dialog';
import { ToastProvider } from '@/components/dashboard/toast';

const mockApi = vi.mocked(api);
const row = {
  id: 'px1',
  label: 'home',
  kind: 'residential',
  geo: 'US',
  shared: false,
  healthy: true,
  available: true,
  exitIp: null,
  lastCheckedAt: null,
  assigned: 0,
  maxPersonas: 1,
};

/** Answers the proxy endpoints; `check` sets what POST /proxies/check reports. */
function route(rows: unknown[], check = [{ id: 'px1', ok: true }]) {
  mockApi.mockImplementation(async (path: string, opts?: { method?: string }) => {
    if (path === '/proxies/check') return { results: check };
    if (path === '/proxies' && opts?.method === 'POST') return { ...row, label: 'new-one' };
    return path === '/proxies' ? { proxies: rows } : {};
  });
}

/** Renders the open dialog. */
function setup() {
  const onChanged = vi.fn();
  render(
    <ToastProvider>
      <ProxiesDialog open onClose={vi.fn()} apiKey="k" onChanged={onChanged} />
    </ToastProvider>,
  );
  return { onChanged };
}

describe('ProxiesDialog', () => {
  beforeEach(() => route([row]));
  afterEach(cleanup);

  it('says profiles connect directly when there are no proxies', async () => {
    route([]);
    setup();
    expect(await screen.findByText('No proxies yet. Profiles connect directly until you add one.')).toBeTruthy();
    expect(screen.queryByRole('button', { name: /Check all/ })).toBeNull();
  });

  it('adds a proxy only once a URL is typed, then reloads', async () => {
    const { onChanged } = setup();
    const add = screen.getByRole('button', { name: 'Add proxy' }) as HTMLButtonElement;
    expect(add.disabled).toBe(true);
    await userEvent.type(screen.getByLabelText('Proxy URL'), 'http://u:p@h:1');
    await userEvent.type(screen.getByLabelText('Country'), 'de');
    await userEvent.click(add);
    expect(await screen.findByText('Added new-one')).toBeTruthy();
    expect(mockApi).toHaveBeenCalledWith('/proxies', {
      key: 'k',
      method: 'POST',
      body: { label: undefined, url: 'http://u:p@h:1', geo: 'DE', kind: 'residential', maxPersonas: 1 },
    });
    expect(onChanged).toHaveBeenCalled();
    expect((screen.getByLabelText('Proxy URL') as HTMLInputElement).value).toBe('');
  });

  it('reports how many proxies failed a check', async () => {
    route([row], [{ id: 'px1', ok: false }]);
    setup();
    await userEvent.click(await screen.findByRole('button', { name: /Check all/ }));
    expect(await screen.findByText('1 of 1 proxies failed')).toBeTruthy();
  });

  it('asks before removing, and removes on the second click', async () => {
    const { onChanged } = setup();
    await userEvent.click(await screen.findByRole('button', { name: 'Remove home' }));
    expect(screen.getByText('Remove?')).toBeTruthy();
    expect(mockApi).not.toHaveBeenCalledWith('/proxies/px1', expect.anything());
    await userEvent.click(screen.getByRole('button', { name: 'Remove home' }));
    await waitFor(() => expect(onChanged).toHaveBeenCalled());
    expect(mockApi).toHaveBeenCalledWith('/proxies/px1', { key: 'k', method: 'DELETE' });
    expect(screen.queryByText('Remove?')).toBeNull();
  });

  it('offers no remove for a shared proxy', async () => {
    route([{ ...row, shared: true }]);
    setup();
    expect(await screen.findByText('residential · shared')).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Remove home' })).toBeNull();
  });

  it('shows why the list could not be loaded', async () => {
    mockApi.mockRejectedValueOnce(new Error('Proxies unavailable'));
    setup();
    expect(await screen.findByText('Proxies unavailable')).toBeTruthy();
  });
});
