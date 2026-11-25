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

  it('can record with no browser running, since recording starts one', async () => {
    serve([]);
    await setup();
    const record = screen.getByRole('button', { name: /Record a flow/ }) as HTMLButtonElement;
    expect(record.disabled).toBe(false);
    expect(record.title).toBe('Do the task yourself in a live browser and keep it as a playbook');
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

  it('leaves a playbook’s code to its editor, with no separate code button on the row', async () => {
    serve([order]);
    await setup();
    expect(screen.getByRole('button', { name: 'Edit order' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Playwright code for order' })).toBeNull();
  });

  it('shows a healed draft’s code to review before promoting it', async () => {
    serve([order]);
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Review' }));
    expect(screen.getByText('draft code')).toBeTruthy();
  });

  it('downloads a playbook as an export file another environment can import', async () => {
    const doc = { format: 'oya-playbook', version: 1, playbook: { name: 'order' } };
    apiMock.mockImplementation(async (path) =>
      path === '/playbooks' ? { playbooks: [order] } : path === '/playbooks/order/export' ? doc : {},
    );
    const created = vi.fn(() => 'blob:x');
    Object.assign(URL, { createObjectURL: created, revokeObjectURL: vi.fn() });
    const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Export order' }));
    expect(click).toHaveBeenCalled();
    expect((click.mock.contexts[0] as HTMLAnchorElement).download).toBe('order.oya-playbook.json');
  });

  it('imports a chosen export file and says so', async () => {
    serve([]);
    await setup();
    apiMock.mockImplementation(async (path) =>
      path === '/playbooks/import' ? { name: 'order', steps: 4 } : { playbooks: [] },
    );
    const file = new File([JSON.stringify({ format: 'oya-playbook', version: 1, playbook: {} })], 'order.json');
    await userEvent.upload(screen.getByLabelText('Playbook export file'), file);
    await act(async () => {});
    expect(apiMock).toHaveBeenCalledWith('/playbooks/import', expect.objectContaining({ method: 'POST' }));
    expect(await screen.findByText('Imported order')).toBeTruthy();
  });

  it('says what to do when an imported name is already taken, and refuses a file that is not JSON', async () => {
    serve([]);
    await setup();
    const { ApiError } = await vi.importActual<typeof import('@/lib/api-client')>('@/lib/api-client');
    apiMock.mockImplementation(async (path) => {
      if (path === '/playbooks/import') throw new ApiError('exists', 409, null);
      return { playbooks: [] };
    });
    await userEvent.upload(screen.getByLabelText('Playbook export file'), new File(['{}'], 'a.json'));
    expect(await screen.findByText(/already exists here\. Rename or delete it/)).toBeTruthy();
    await userEvent.upload(screen.getByLabelText('Playbook export file'), new File(['nope'], 'b.json'));
    expect(await screen.findByText('b.json is not a playbook export (not JSON)')).toBeTruthy();
  });
  describe('run history', () => {
    const RUNS = [
      {
        status: 'failed',
        startedAt: '2026-01-01T00:02:00Z',
        endedAt: '2026-01-01T00:03:05Z',
        browserId: 'b-1',
        error: 'No field <b>x</b>',
      },
      { status: 'succeeded', startedAt: '2026-01-01T00:00:00Z', endedAt: '2026-01-01T00:00:42Z', browserId: 'b-1' },
    ];
    /** Answers the list with `order`, and its runs with `runs`, or a refusal. */
    function serveRuns(runs: unknown) {
      apiMock.mockImplementation(async (path) => {
        if (path === '/playbooks') return { playbooks: [order] };
        if (path === '/playbooks/order/runs') {
          if (runs instanceof Error) throw runs;
          return { runs };
        }
        return {};
      });
    }
    /** Opens the order playbook's runs. */
    const openRuns = async () => {
      const toggle = screen.getByRole('button', { name: 'Runs of order' });
      await userEvent.click(toggle);
      await act(async () => {});
      return toggle;
    };

    it('opens under the row with each run’s words, duration and browser, and a failed run’s error as plain text', async () => {
      serveRuns(RUNS);
      await setup();
      const toggle = await openRuns();
      expect(toggle.getAttribute('aria-expanded')).toBe('true');
      const runs = within(screen.getByRole('region', { name: 'Runs of order' }));
      expect(runs.getByText('Failed')).toBeTruthy();
      expect(runs.getByText('Succeeded')).toBeTruthy();
      expect(runs.getByText(/1m 5s · on b-1/)).toBeTruthy();
      expect(runs.getByText(/42s · on b-1/)).toBeTruthy();
      expect(runs.getByText('No field <b>x</b>')).toBeTruthy();
    });

    it('an open history reloads when the list sees a newer run', async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      let lastRun: unknown = null;
      let runs: unknown[] = [];
      apiMock.mockImplementation(async (path) => {
        if (path === '/playbooks') return { playbooks: [{ ...order, lastRun }] };
        if (path === '/playbooks/order/runs') return { runs };
        return {};
      });
      await setup();
      await openRuns();
      expect(screen.getByText('No runs yet. Run it from here, the SDK or the desktop app.')).toBeTruthy();
      runs = [RUNS[1]];
      lastRun = { status: 'succeeded', endedAt: RUNS[1].endedAt };
      await act(() => vi.advanceTimersByTimeAsync(PLAYBOOKS_POLL_MS));
      expect(screen.getByText('Succeeded')).toBeTruthy();
    });

    it('says when a playbook has not run yet', async () => {
      serveRuns([]);
      await setup();
      await openRuns();
      expect(screen.getByText('No runs yet. Run it from here, the SDK or the desktop app.')).toBeTruthy();
    });

    it('says when the runs could not load, and retries', async () => {
      serveRuns(new Error('down'));
      await setup();
      await openRuns();
      expect(screen.getByRole('alert').textContent).toContain("Couldn't load runs.");
      serveRuns(RUNS);
      await userEvent.click(screen.getByRole('button', { name: 'Retry' }));
      await act(async () => {});
      expect(screen.getByText('Succeeded')).toBeTruthy();
    });
  });
});
