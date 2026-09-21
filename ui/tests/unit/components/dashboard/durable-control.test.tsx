/**
 * Unit tests for Project operations: loading, the session inventory and its
 * actions, and the settings form's JSON check.
 */
import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (importActual) => ({
  ...(await importActual<typeof import('@/lib/api-client')>()),
  api: vi.fn(),
}));

import { api } from '@/lib/api-client';
import DurableControl from '@/components/dashboard/durable-control';

/** An administrator's overview with one ready and one stopped session. */
const OVERVIEW = {
  project: {
    id: 'proj-1',
    name: 'Test project',
    settings: { maxConcurrent: 2, budgetUsd: null, recordingDays: 7, auditDays: 30, rates: {}, policy: {} },
  },
  sessions: [
    { id: 'ready-1', provider: 'oya', state: 'ready', managed: true, costUsd: 0, control: { mode: 'agent' } },
    { id: 'gone-1', provider: 'oya', state: 'stopped', managed: true, costUsd: 0, control: { mode: 'agent' } },
  ],
  draining: false,
  events: [],
  credentials: [{ id: 'c1', role: 'viewer', label: 'ci', revokedAt: null }],
  webhooks: [],
  deliveries: [],
};

/** Renders the section and lets the first poll land. */
async function setup() {
  render(<DurableControl apiKey="k" />);
  await act(async () => {});
}

describe('DurableControl', () => {
  beforeEach(() => {
    vi.mocked(api)
      .mockReset()
      .mockImplementation((async (path: string) =>
        path === '/control/members'
          ? { members: [{ userId: 'member-test', role: 'operator' }] }
          : OVERVIEW) as typeof api);
  });
  afterEach(cleanup);

  it('says it is loading until the overview arrives', () => {
    vi.mocked(api).mockImplementationOnce(() => new Promise(() => {}));
    render(<DurableControl apiKey="k" />);
    expect(screen.getByRole('status').textContent).toBe('Loading durable session inventory…');
  });

  it('shows the project, its members and the headline counts', async () => {
    await setup();
    expect(screen.getByRole('heading', { name: 'Test project' })).toBeTruthy();
    expect(screen.getByText('member-test · operator')).toBeTruthy();
    expect(screen.getByText('Active and reserved').nextSibling?.textContent).toBe('1');
  });

  it('filters the inventory by state', async () => {
    await setup();
    await userEvent.selectOptions(screen.getByLabelText('Filter sessions by state'), 'queued');
    expect(screen.getByText('No sessions match this view.')).toBeTruthy();
  });

  it('hands a ready session to a human with Take control', async () => {
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Take control' }));
    expect(api).toHaveBeenCalledWith('/control/sessions/ready-1/control', {
      key: 'k',
      method: 'POST',
      body: { action: 'acquire' },
    });
  });

  it('offers to replace a stopped managed session from its profile', async () => {
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Replace from profile' }));
    expect(api).toHaveBeenCalledWith('/control/sessions/gone-1/recover', {
      key: 'k',
      method: 'POST',
      body: { replace: true },
    });
  });

  it('shows a once-shown secret until dismissed', async () => {
    await setup();
    vi.mocked(api).mockResolvedValueOnce({ token: 'tok-1' });
    await userEvent.click(screen.getByRole('button', { name: 'Create' }));
    expect(screen.getByText('tok-1')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Dismiss secret' }));
    expect(screen.queryByText('tok-1')).toBeNull();
  });

  it('refuses to save settings whose rate cards are not JSON', async () => {
    await setup();
    const rates = screen.getByLabelText('Rate cards · USD per browser hour');
    fireEvent.change(rates, { target: { value: '{bad' } });
    fireEvent.submit(screen.getByRole('button', { name: 'Save settings' }).closest('form')!);
    expect(screen.getByRole('alert').textContent).toBe('Rate cards and policy must be valid JSON');
    expect(api).not.toHaveBeenCalledWith('/control/project', expect.anything());
  });
});
