/**
 * Unit tests for the webhook panel: it loads the endpoint, saves it with the
 * chosen events, shows a minted secret once, disables, replays, and reports
 * failures.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (importActual) => ({
  ...(await importActual<typeof import('@/lib/api-client')>()),
  api: vi.fn(),
}));

import { api } from '@/lib/api-client';
import WebhookSection from '@/components/dashboard/webhook-section';
import { Row } from '@/components/dashboard/settings/fields';
import { toggled } from '@/components/dashboard/settings/use-webhook';
import { ToastProvider } from '@/components/dashboard/toast';

const apiMock = vi.mocked(api);

const EVENTS = ['run.failed', 'run.done'];
const HOOK = { id: 'h', url: 'https://x/hook', types: ['run.failed'], enabled: true };
const DELIVERY = { id: 'd/1', state: 'pending', attempts: 2, at: Date.now(), type: 'run.failed' };

/** Answers the webhook endpoints; GET returns `hook`. */
function serve(hook: typeof HOOK | null, secret?: string) {
  apiMock.mockImplementation(async (_path, opts) => {
    if (opts?.method === 'PUT') return { ...HOOK, secret };
    if (opts?.method) return {};
    return { hook, events: EVENTS, deliveries: hook ? [DELIVERY] : [] };
  });
}

/** Renders the panel. */
function setup() {
  render(
    <ToastProvider>
      <WebhookSection apiKey="k" Row={Row} />
    </ToastProvider>,
  );
}

describe('WebhookSection', () => {
  afterEach(cleanup);

  it('shows a load failure as the whole panel', async () => {
    apiMock.mockRejectedValueOnce(new Error('No access'));
    setup();
    expect((await screen.findByRole('alert')).textContent).toBe('No access');
  });

  it('saves the trimmed URL with the chosen events and shows the new secret once', async () => {
    serve(null, 'whsec_1');
    setup();
    await userEvent.type(await screen.findByLabelText('Endpoint URL'), ' https://x/hook ');
    await userEvent.click(screen.getByRole('checkbox', { name: 'run.done' }));
    await userEvent.click(screen.getByRole('button', { name: 'Save' }));
    const body = { url: 'https://x/hook', types: ['run.done'], roll: false };
    expect(apiMock).toHaveBeenCalledWith('/control/webhook', { key: 'k', method: 'PUT', body });
    expect(await screen.findByText('whsec_1')).toBeTruthy();
  });

  it('fills the form from the saved endpoint and lists its deliveries', async () => {
    serve(HOOK);
    setup();
    expect(((await screen.findByLabelText('Endpoint URL')) as HTMLInputElement).value).toBe(HOOK.url);
    expect((screen.getByRole('checkbox', { name: 'run.failed' }) as HTMLInputElement).checked).toBe(true);
    expect(screen.getByText(/pending · 2 tries/)).toBeTruthy();
    expect(screen.getByText('Receiving events.')).toBeTruthy();
  });

  it('replays a delivery by its encoded id', async () => {
    serve(HOOK);
    setup();
    await userEvent.click(await screen.findByRole('button', { name: 'Replay' }));
    expect(apiMock).toHaveBeenCalledWith('/control/deliveries/d%2F1/replay', { key: 'k', method: 'POST' });
    expect(await screen.findByText('Delivery queued')).toBeTruthy();
  });

  it('disables an active endpoint', async () => {
    serve(HOOK);
    setup();
    await userEvent.click(await screen.findByRole('button', { name: 'Disable' }));
    expect(apiMock).toHaveBeenCalledWith('/control/webhook', { key: 'k', method: 'DELETE' });
  });

  it('shows why saving failed and keeps the form', async () => {
    serve(HOOK);
    setup();
    await screen.findByLabelText('Endpoint URL');
    apiMock.mockRejectedValueOnce(new Error('Must be HTTPS'));
    await userEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect((await screen.findByRole('alert')).textContent).toBe('Must be HTTPS');
  });

  it('sends a test event to the saved endpoint and says it arrived', async () => {
    serve(HOOK);
    setup();
    await screen.findByLabelText('Endpoint URL');
    apiMock.mockResolvedValueOnce({ delivered: true });
    await userEvent.click(screen.getByRole('button', { name: 'Send test event' }));
    expect(apiMock).toHaveBeenCalledWith('/control/webhook/test', { key: 'k', method: 'POST' });
    expect(await screen.findByText(/Test event delivered/)).toBeTruthy();
  });

  it('says why a test event did not arrive', async () => {
    serve(HOOK);
    setup();
    await screen.findByLabelText('Endpoint URL');
    apiMock.mockResolvedValueOnce({ delivered: false, error: 'connect ECONNREFUSED' });
    await userEvent.click(screen.getByRole('button', { name: 'Send test event' }));
    expect((await screen.findByRole('alert')).textContent).toBe('Test event not delivered: connect ECONNREFUSED.');
  });

  it('offers no test until an endpoint is saved', async () => {
    serve(null);
    setup();
    await screen.findByLabelText('Endpoint URL');
    expect(screen.queryByRole('button', { name: 'Send test event' })).toBeNull();
  });

  it('documents the payload of every event the endpoint can subscribe to', async () => {
    serve(HOOK);
    setup();
    await userEvent.click(await screen.findByText('What each event looks like'));
    expect(screen.getByText('run.failed', { selector: 'td' })).toBeTruthy();
    expect(screen.getByText('{ runId, owner, error }')).toBeTruthy();
  });

  it('toggles an event type in or out', () => {
    expect(toggled(['a'], 'b')).toEqual(['a', 'b']);
    expect(toggled(['a', 'b'], 'a')).toEqual(['b']);
  });
});
