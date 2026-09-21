/**
 * Unit tests for the Control tab: views switch by tab, each shows the key's
 * data, actions hit the gateway API, and failures appear as an alert.
 */
import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (importActual) => ({
  ...(await importActual<typeof import('@/lib/api-client')>()),
  api: vi.fn(),
}));

import { api } from '@/lib/api-client';
import ControlTab from '@/components/dashboard/control-tab';

/** A fleet with one provider and a quiet hour. */
const FLEET = {
  at: '2026-01-01T00:00:00Z',
  uptimeSeconds: 90,
  browsers: { total: 2, byClient: { oya: 2 }, byProvider: {} },
  sessions: { total: 1, attached: 1, recording: 0 },
  routing: {
    strategy: 'priority',
    queueDepth: 0,
    capacity: 5,
    active: 0,
    healthy: 1,
    providers: [
      {
        name: 'mine',
        type: 'cdp',
        active: 0,
        maxConcurrent: 5,
        priority: 1,
        weight: 1,
        healthy: true,
        available: true,
        latencyMs: 12,
        cooldownMsRemaining: 0,
        totalSessions: 3,
        totalFailures: 0,
      },
    ],
  },
  usage: { hour: '2026-01-01T00:00:00Z', openBrowsers: 2, commands: 100, command_errors: 1 },
  limits: { commandsPerMinute: { limit: 60, burst: 10, remaining: 9 } },
  quotas: {},
};

/** GET answers by path. */
const ANSWERS: Record<string, unknown> = {
  '/fleet': FLEET,
  '/gateway/sessions': {
    sessions: [
      {
        id: 'sess-123456789',
        provider: 'mine',
        profile: null,
        connected: true,
        seconds: 5,
        bytesUp: 0,
        bytesDown: 0,
        recording: false,
      },
    ],
  },
  '/audit?limit=200': { events: [] },
  '/gateway/recordings': { recordings: [] },
  '/providers': { providers: [] },
};

/** Renders the tab and lets the first poll land. */
async function setup() {
  render(<ControlTab apiKey="k" />);
  await act(async () => {});
}

describe('ControlTab', () => {
  beforeEach(() => {
    vi.mocked(api)
      .mockReset()
      .mockImplementation((async (path: string) => ANSWERS[path] ?? {}) as typeof api);
  });
  afterEach(cleanup);

  it('opens on the Overview with the hour’s headline figures', async () => {
    await setup();
    expect(screen.getByRole('heading', { name: 'Overview' })).toBeTruthy();
    expect(screen.getByText('1.0%')).toBeTruthy();
    expect(screen.getByText('Commands per minute')).toBeTruthy();
  });

  it('switches view by tab and marks the selected one', async () => {
    await setup();
    await userEvent.click(screen.getByRole('tab', { name: 'Usage' }));
    expect(screen.getByRole('heading', { name: 'Usage' })).toBeTruthy();
    expect(screen.getByRole('tab', { name: 'Usage' }).getAttribute('aria-selected')).toBe('true');
    expect(screen.getByText('Browsers open now')).toBeTruthy();
  });

  it('ends a CDP session through the gateway', async () => {
    await setup();
    await userEvent.click(screen.getByRole('tab', { name: 'CDP sessions' }));
    await userEvent.click(screen.getByTitle('End session'));
    expect(api).toHaveBeenCalledWith('/gateway/sessions/sess-123456789', { key: 'k', method: 'DELETE' });
  });

  it('explains how recordings are made when there are none', async () => {
    await setup();
    await userEvent.click(screen.getByRole('tab', { name: 'Recordings' }));
    expect(screen.getByText(/No recordings\./)).toBeTruthy();
  });

  it('adds a CDP provider and confirms it was saved', async () => {
    await setup();
    await userEvent.click(screen.getByRole('tab', { name: 'Providers' }));
    await userEvent.click(screen.getByRole('button', { name: 'Add provider' }));
    const form = screen.getByRole('form', { name: 'Add provider' });
    await userEvent.type(screen.getByPlaceholderText('my-chrome'), 'local');
    await userEvent.type(screen.getByPlaceholderText(/ws:\/\/127/), 'ws://h');
    await userEvent.click(form.querySelector('button[type=submit]')!);
    expect(api).toHaveBeenCalledWith('/gateway/providers', expect.objectContaining({ method: 'POST' }));
    expect((await screen.findByRole('status')).textContent).toMatch(/Provider saved/);
    expect(screen.queryByRole('form', { name: 'Add provider' })).toBeNull();
  });

  it('shows a failed action as an alert', async () => {
    await setup();
    await userEvent.click(screen.getByRole('tab', { name: 'Providers' }));
    vi.mocked(api).mockRejectedValueOnce(new Error('in use'));
    await userEvent.click(screen.getByRole('button', { name: 'Remove mine' }));
    expect((await screen.findByRole('alert')).textContent).toBe('in use');
  });

  it('shows a failed load as an alert', async () => {
    vi.mocked(api).mockRejectedValueOnce(new Error('offline'));
    await setup();
    expect(screen.getByRole('alert').textContent).toBe('Could not load the fleet: offline');
  });
});
