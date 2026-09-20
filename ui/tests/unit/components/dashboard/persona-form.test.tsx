/**
 * Unit tests for the new-profile dialog: the device choices, the debounced
 * preview, and creation with and without a second factor.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<typeof import('@/lib/api-client')>()), api: vi.fn() }));

import { api } from '@/lib/api-client';
import PersonaForm from '@/components/dashboard/persona-form';
import { ToastProvider } from '@/components/dashboard/toast';
import { PREVIEW_DEBOUNCE_MS } from '@/components/dashboard/personas/constants';

const mockApi = vi.mocked(api);
const options = { platforms: ['Win32'], timezones: { Win32: ['Europe/Berlin'] }, locales: { Win32: ['de-DE'] } };
const fingerprint = {
  platform: 'Win32',
  timezone: 'Europe/Berlin',
  locale: 'de-DE',
  screen: '1920x1080',
  webgl: 'ANGLE',
  hardwareConcurrency: 8,
  deviceMemory: 16,
  canvasSeed: 1,
};

/** Answers each endpoint the form calls; `create` overrides POST /personas. */
function route(create: () => Promise<unknown> = async () => ({ id: 'p9', name: 'acme' })) {
  mockApi.mockImplementation(async (path: string) => {
    if (path === '/personas/options') return options;
    if (path === '/personas/preview') return { fingerprint };
    if (path === '/personas') return create();
    return {};
  });
}

/** Renders the open dialog. */
function setup() {
  const onCreated = vi.fn();
  const onClose = vi.fn();
  render(
    <ToastProvider>
      <PersonaForm open onClose={onClose} apiKey="k" onCreated={onCreated} />
    </ToastProvider>,
  );
  return { onCreated, onClose };
}

describe('PersonaForm', () => {
  beforeEach(() => route());
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it('asks for the preview only after the choices settle', async () => {
    vi.useFakeTimers();
    setup();
    const previews = () => mockApi.mock.calls.filter(([p]) => p === '/personas/preview');
    expect(previews()).toHaveLength(0);
    await act(async () => vi.advanceTimersByTime(PREVIEW_DEBOUNCE_MS));
    expect(previews()).toHaveLength(1);
    expect(screen.getByText('ANGLE')).toBeTruthy();
  });

  it('locks timezone and locale until a platform is chosen', async () => {
    setup();
    expect((screen.getByLabelText('Timezone') as HTMLSelectElement).disabled).toBe(true);
    await userEvent.selectOptions(
      await screen.findByLabelText('Platform'),
      await screen.findByRole('option', { name: 'Windows' }),
    );
    expect((screen.getByLabelText('Timezone') as HTMLSelectElement).disabled).toBe(false);
    expect(screen.getByRole('option', { name: 'Europe/Berlin' })).toBeTruthy();
  });

  it('puts timezone back on auto when the platform changes', async () => {
    setup();
    await userEvent.selectOptions(
      screen.getByLabelText('Platform'),
      await screen.findByRole('option', { name: 'Windows' }),
    );
    await userEvent.selectOptions(screen.getByLabelText('Timezone'), 'Europe/Berlin');
    await userEvent.selectOptions(screen.getByLabelText('Platform'), 'auto');
    expect((screen.getByLabelText('Timezone') as HTMLSelectElement).value).toBe('auto');
  });

  it('creates the profile with its cap and geo, then closes', async () => {
    const { onCreated, onClose } = setup();
    await userEvent.type(screen.getByLabelText('Name'), 'acme');
    await userEvent.type(screen.getByLabelText('Proxy geo'), 'us');
    await userEvent.click(screen.getByRole('button', { name: 'Create profile' }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(mockApi).toHaveBeenCalledWith('/personas', {
      key: 'k',
      method: 'POST',
      body: { name: 'acme', prefs: {}, maxConcurrent: 2, proxy: { geo: 'US' } },
    });
    expect(onCreated).toHaveBeenCalledWith({ id: 'p9', name: 'acme' });
    expect(mockApi.mock.calls.some(([p]) => String(p).endsWith('/mfa'))).toBe(false);
  });

  it('stores a filled-in second factor right after creating', async () => {
    const { onClose } = setup();
    await userEvent.selectOptions(screen.getByLabelText('MFA type'), 'totp');
    await userEvent.type(screen.getByLabelText('Secret'), 'SEED');
    await userEvent.click(screen.getByRole('button', { name: 'Create profile' }));
    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(mockApi).toHaveBeenCalledWith('/personas/p9/mfa', {
      key: 'k',
      method: 'PUT',
      body: { type: 'totp', secret: 'SEED' },
    });
  });

  it('shows why creation failed and stays open', async () => {
    route(() => Promise.reject(new Error('Name taken')));
    const { onClose } = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Create profile' }));
    expect(await screen.findByText('Name taken')).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('shows why the options could not be loaded', async () => {
    mockApi.mockRejectedValueOnce(new Error('No options'));
    setup();
    expect(await screen.findByText('No options')).toBeTruthy();
  });
});
