/**
 * Unit tests for the Playbooks tab, through what the user sees and clicks: the
 * list, its empty and error states, and the promote, delete and rename edits.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));
vi.mock('@/components/ui/syntax-code', () => ({ default: ({ code }: { code: string }) => <code>{code}</code> }));

import { api } from '@/lib/api-client';
import PlaybooksTab from '@/components/dashboard/playbooks-tab';
import { ToastProvider } from '@/components/dashboard/toast';
import { PLAYBOOKS_POLL_MS } from '@/components/dashboard/playbooks/constants';
import type { PlaybookInfo } from '@/components/dashboard/playbooks/types';

const apiMock = vi.mocked(api);
const NOW = Date.parse('2026-01-01T00:10:00Z');
const order: PlaybookInfo = {
  name: 'order',
  variables: ['name'],
  defaults: { name: 'Ada' },
  steps: 4,
  code: 'export default async () => {}',
  createdAt: '2026-01-01T00:00:00Z',
  promotedAt: null,
  draft: {
    name: 'order',
    variables: [],
    defaults: {},
    steps: 6,
    code: 'draft code',
    healedAt: '2026-01-01T00:05:00Z',
    healedFrom: 2,
  },
};

/** Answers GET /playbooks with `list`, and anything else with `{}`. */
function serve(list: PlaybookInfo[]) {
  apiMock.mockImplementation(async (path) => (path === '/playbooks' ? { playbooks: list } : {}));
}

/** Renders the tab and waits for the first load. */
async function setup() {
  render(
    <ToastProvider>
      <PlaybooksTab apiKey="k" browsers={[]} personas={[]} now={NOW} />
    </ToastProvider>,
  );
  await act(async () => {});
}

describe('PlaybooksTab', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it('lists each playbook with its variables, steps and healed draft', async () => {
    serve([order]);
    await setup();
    const row = screen.getByRole('row', { name: /order/ });
    expect(within(row).getByText('name')).toBeTruthy();
    expect(within(row).getByText('4')).toBeTruthy();
    expect(within(row).getByText('6 steps · 5m')).toBeTruthy();
    expect(within(row).getByTitle('The replay broke at step 3; the agent finished it.')).toBeTruthy();
    expect(screen.getByRole('heading', { name: /Playbooks/ }).textContent).toBe('Playbooks 1');
  });

  it('shows how to make one when there are none', async () => {
    serve([]);
    await setup();
    expect(screen.getByText('No playbooks yet')).toBeTruthy();
  });

  it('says why the list could not load', async () => {
    apiMock.mockRejectedValueOnce(new Error('forbidden'));
    await setup();
    expect(screen.getByRole('alert').textContent).toBe('Could not load playbooks: forbidden');
    expect(screen.queryByText('No playbooks yet')).toBeNull();
  });

  it('refreshes the list on an interval', async () => {
    vi.useFakeTimers();
    serve([]);
    await setup();
    await act(() => vi.advanceTimersByTimeAsync(PLAYBOOKS_POLL_MS));
    expect(apiMock.mock.calls.filter(([p]) => p === '/playbooks')).toHaveLength(2);
  });

  it('cannot record without a running browser', async () => {
    serve([]);
    await setup();
    const record = screen.getByRole('button', { name: /Record a flow/ }) as HTMLButtonElement;
    expect(record.disabled).toBe(true);
    expect(record.title).toBe('Start a browser first');
  });

  it('promotes a healed draft', async () => {
    serve([order]);
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Promote' }));
    expect(apiMock).toHaveBeenCalledWith('/playbooks/order/promote', { key: 'k', method: 'POST', body: {} });
    expect(await screen.findByText('order now uses the healed steps')).toBeTruthy();
  });

  it('deletes a playbook only after confirmation', async () => {
    serve([order]);
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Delete order' }));
    expect(screen.getByText('Delete order?')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Delete' }));
    expect(apiMock).toHaveBeenCalledWith('/playbooks/order', { key: 'k', method: 'DELETE' });
    expect(await screen.findByText('Deleted order')).toBeTruthy();
  });

  it('discarding a draft deletes only the draft', async () => {
    serve([order]);
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Discard' }));
    expect(screen.getByText('Discard the healed draft?')).toBeTruthy();
    await userEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: 'Discard' }));
    expect(apiMock).toHaveBeenCalledWith('/playbooks/order%3Adraft', { key: 'k', method: 'DELETE' });
    expect(await screen.findByText('Draft discarded')).toBeTruthy();
  });

  it('renames only to a valid, different name', async () => {
    serve([order]);
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Rename order' }));
    const rename = screen.getByRole('button', { name: 'Rename' }) as HTMLButtonElement;
    expect(rename.disabled).toBe(true);
    const field = screen.getByLabelText('Name');
    await userEvent.clear(field);
    await userEvent.type(field, 'bad name');
    expect(rename.disabled).toBe(true);
    await userEvent.clear(field);
    await userEvent.type(field, 'checkout{Enter}');
    expect(apiMock).toHaveBeenCalledWith('/playbooks/order', { key: 'k', method: 'PATCH', body: { name: 'checkout' } });
    expect(await screen.findByText('Renamed to checkout')).toBeTruthy();
  });

  it('a failed rename keeps the dialog open and says why', async () => {
    serve([order]);
    await setup();
    apiMock.mockRejectedValueOnce(new Error('name taken'));
    await userEvent.click(screen.getByRole('button', { name: 'Rename order' }));
    const field = screen.getByLabelText('Name');
    await userEvent.type(field, '2{Enter}');
    expect(await screen.findByText('name taken')).toBeTruthy();
    expect(screen.getByText('Rename order')).toBeTruthy();
  });

  it('shows the Playwright code for a playbook', async () => {
    serve([order]);
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Playwright code for order' }));
    expect(screen.getByText('export default async () => {}')).toBeTruthy();
  });
});
