/**
 * Unit tests for the Routines tab, through what the user sees and clicks: the
 * list and its empty and error states, making a routine, Run now and its refusal,
 * turning one off, clearing a history, and deleting.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));

import { api, ApiError } from '@/lib/api-client';
import RoutinesTab from '@/components/dashboard/routines-tab';
import { ToastProvider } from '@/components/dashboard/toast';
import type { Routine } from '@/components/dashboard/routines/types';

const apiMock = vi.mocked(api);
const NOW = Date.parse('2026-01-01T12:00:00Z');
const HOUR = 3_600_000;

/** A cloud routine, hourly, never run, with `over` laid on. */
const routine = (over: Partial<Routine> = {}): Routine => ({
  id: 'r1',
  createdAt: NOW - HOUR / 2,
  name: 'Prices',
  prompt: 'Check prices',
  schedule: { kind: 'every', n: 1, unit: 'hours' },
  enabled: true,
  target: 'cloud',
  lastRunAt: null,
  nextRunAt: NOW + HOUR / 2,
  runs: [],
  ...over,
});
const running = { id: 'run1', startedAt: NOW, status: 'running' };
const done = { id: 'run0', startedAt: NOW - HOUR, finishedAt: NOW - HOUR + 42_000, status: 'done', result: 'All good' };

/** Answers GET /routines with `list`, the playbooks with none, and `other` for anything else. */
function serve(list: Routine[], other: (path: string, opts: { method?: string }) => unknown = () => ({})) {
  apiMock.mockImplementation(async (path, opts) => {
    if (path === '/routines' && !opts?.method) return { routines: list };
    if (path === '/playbooks') return { playbooks: [] };
    return other(path, opts);
  });
}

/** A playbook with a value, a saved default and a secret. */
const prices = { name: 'prices', variables: ['item', 'shop', 'pw'], defaults: { shop: 'acme' }, secrets: ['pw'] };
/** The project's default profile and one named one. */
const personas = [
  { id: 'd', name: 'Default', isDefault: true, createdAt: '', lastUsedAt: null, activeBrowsers: 0 },
  { id: 'p1', name: 'Work', isDefault: false, createdAt: '', lastUsedAt: null, activeBrowsers: 0 },
];

/** Renders the tab and waits for the first load. */
async function setup() {
  render(
    <ToastProvider>
      <RoutinesTab apiKey="k" now={NOW} personas={personas as never} />
    </ToastProvider>,
  );
  await act(async () => {});
}

/** The row of the routine named `name`. */
const rowOf = (name: string) => screen.getByRole('row', { name: new RegExp(name) });

describe('RoutinesTab', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });
  afterEach(() => cleanup());

  it('shows what a routine is for when there are none', async () => {
    serve([]);
    await setup();
    expect(screen.getByText('No routines yet')).toBeTruthy();
  });

  it('keeps the last list and says why a refresh failed', async () => {
    apiMock.mockRejectedValueOnce(new Error('forbidden'));
    await setup();
    expect(screen.getByRole('alert').textContent).toBe('Could not reach this project’s routines: forbidden');
    expect(screen.queryByText('No routines yet')).toBeNull();
  });

  it('lists a routine with its schedule, where it runs, and when it runs next', async () => {
    serve([routine()]);
    await setup();
    const row = rowOf('Prices');
    expect(within(row).getByText('Every hour')).toBeTruthy();
    expect(within(row).getByText('Cloud')).toBeTruthy();
    expect(within(row).getByText('Never run')).toBeTruthy();
    expect(screen.getByRole('heading', { name: /Routines/ }).textContent).toBe('Routines 1');
  });

  it('makes a routine every 2 hours in the cloud, sending the person’s time zone', async () => {
    const made = routine({ id: 'r2', name: 'Inbox', schedule: { kind: 'every', n: 2, unit: 'hours' } });
    serve([], (path, opts) => (path === '/routines' && opts.method === 'POST' ? made : {}));
    await setup();
    await userEvent.click(screen.getByRole('button', { name: /New routine/ }));
    await userEvent.type(screen.getByLabelText('Name'), 'Inbox');
    await userEvent.type(screen.getByLabelText('Prompt'), 'Summarize');
    await userEvent.clear(screen.getByLabelText('How often'));
    await userEvent.type(screen.getByLabelText('How often'), '2');
    await userEvent.click(screen.getByLabelText('Run in the cloud'));
    await userEvent.click(screen.getByRole('button', { name: 'Save routine' }));
    const body = apiMock.mock.calls.find(([p, o]) => p === '/routines' && o?.method === 'POST')?.[1]?.body as Record<
      string,
      unknown
    >;
    expect(body).toMatchObject({
      name: 'Inbox',
      prompt: 'Summarize',
      target: 'cloud',
      schedule: { kind: 'every', n: 2, unit: 'hours' },
    });
    expect(body.tz).toBe(Intl.DateTimeFormat().resolvedOptions().timeZone);
    expect(within(rowOf('Inbox')).getByText('Every 2 hours')).toBeTruthy();
  });

  it('saves a playbook routine’s typed values (never a secret or a blank) and the profile picked', async () => {
    serve([], (path, opts) => (path === '/routines' && opts.method === 'POST' ? routine({ id: 'r2' }) : {}));
    apiMock.mockImplementation(async (path, opts) => {
      if (path === '/routines' && !opts?.method) return { routines: [] };
      if (path === '/playbooks') return { playbooks: [prices] };
      return path === '/routines' ? routine({ id: 'r2' }) : {};
    });
    await setup();
    await userEvent.click(screen.getByRole('button', { name: /New routine/ }));
    await userEvent.type(screen.getByLabelText('Name'), 'Tea');
    await userEvent.selectOptions(screen.getByLabelText('Does'), 'prices');
    await userEvent.type(screen.getByLabelText('item'), 'tea');
    expect(screen.getByLabelText('shop').getAttribute('placeholder')).toBe('acme');
    expect(screen.getByLabelText('pw').getAttribute('placeholder')).toBe('Uses the playbook’s saved value');
    expect(screen.queryByRole('option', { name: 'Default', hidden: false })).toBeTruthy();
    await userEvent.selectOptions(screen.getByLabelText('Profile'), 'p1');
    expect(screen.getByText('A desktop runs it only while it is on this profile.')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Save routine' }));
    const body = apiMock.mock.calls.find(([p, o]) => p === '/routines' && o?.method === 'POST')?.[1]?.body;
    expect(body).toMatchObject({ playbook: 'prices', prompt: '', values: { item: 'tea' }, persona: 'p1' });
  });

  it('sends no profile for the default, and no values with a prompt', async () => {
    serve([], (path, opts) => (path === '/routines' && opts.method === 'POST' ? routine({ id: 'r2' }) : {}));
    await setup();
    await userEvent.click(screen.getByRole('button', { name: /New routine/ }));
    await userEvent.type(screen.getByLabelText('Name'), 'Inbox');
    await userEvent.type(screen.getByLabelText('Prompt'), 'Summarize');
    await userEvent.click(screen.getByRole('button', { name: 'Save routine' }));
    const body = apiMock.mock.calls.find(([p, o]) => p === '/routines' && o?.method === 'POST')?.[1]?.body as object;
    expect(body).toMatchObject({ persona: null });
    expect('values' in body).toBe(false);
  });

  it('keeps a routine’s profile that is no longer listed, by its id', async () => {
    serve([routine({ persona: 'gone' })]);
    await setup();
    await userEvent.click(within(rowOf('Prices')).getByRole('button', { name: /Edit/ }));
    expect((screen.getByLabelText('Profile') as HTMLSelectElement).value).toBe('gone');
  });

  it('says what is missing instead of saving an empty routine', async () => {
    serve([]);
    await setup();
    await userEvent.click(screen.getByRole('button', { name: /New routine/ }));
    await userEvent.click(screen.getByRole('button', { name: 'Save routine' }));
    expect(screen.getByText('Give the routine a name.')).toBeTruthy();
    expect(screen.getByText('Write the prompt, or pick a playbook.')).toBeTruthy();
    expect(apiMock.mock.calls.some(([, o]) => o?.method === 'POST')).toBe(false);
  });

  it('runs a cloud routine now, then holds Run now while it runs', async () => {
    serve([routine()], (path) => (path === '/routines/r1/run' ? routine({ runs: [running] }) : {}));
    await setup();
    await userEvent.click(within(rowOf('Prices')).getByRole('button', { name: 'Run now' }));
    const row = rowOf('Prices');
    expect(within(row).getByText('Running in the cloud')).toBeTruthy();
    expect((within(row).getByRole('button', { name: 'Run now' }) as HTMLButtonElement).disabled).toBe(true);
    expect(within(row).getByRole('status').textContent).toBe('Started.');
  });

  it('puts the server’s refusal of a Run now in the row', async () => {
    serve([routine()], () => {
      throw new ApiError('This routine is already running.', 409, null);
    });
    await setup();
    await userEvent.click(within(rowOf('Prices')).getByRole('button', { name: 'Run now' }));
    expect(within(rowOf('Prices')).getByRole('status').textContent).toBe('This routine is already running.');
  });

  it('gives a desktop routine no Run now', async () => {
    serve([routine({ target: 'desktop' })]);
    await setup();
    expect(within(rowOf('Prices')).queryByRole('button', { name: 'Run now' })).toBeNull();
  });

  it('turns a routine off, and flips back when the save fails', async () => {
    serve([routine()], () => {
      throw new ApiError('Server down', 500, null);
    });
    await setup();
    const toggle = within(rowOf('Prices')).getByRole('switch', { name: 'Run “Prices” on its schedule' });
    await userEvent.click(toggle);
    expect(apiMock).toHaveBeenCalledWith('/routines/r1', { key: 'k', method: 'PATCH', body: { enabled: false } });
    expect(toggle.getAttribute('aria-checked')).toBe('true');
    expect(await screen.findByText('Server down')).toBeTruthy();
  });

  it('says a routine was deleted elsewhere and refreshes', async () => {
    serve([routine()], () => {
      throw new ApiError('No such routine', 404, null);
    });
    await setup();
    await userEvent.click(within(rowOf('Prices')).getByRole('switch'));
    expect(await screen.findByText('This routine was deleted elsewhere.')).toBeTruthy();
    expect(apiMock.mock.calls.filter(([p, o]) => p === '/routines' && !o?.method).length).toBe(2);
  });

  it('clears a history down to the run still going, and hides Clear history then', async () => {
    serve([routine({ runs: [running, done] })], (path) =>
      path === '/routines/r1/runs' ? routine({ runs: [running] }) : {},
    );
    await setup();
    await userEvent.click(within(rowOf('Prices')).getByRole('button', { name: /History · 2/ }));
    expect(screen.getByText('All good')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Clear history' }));
    expect(screen.queryByText('All good')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Clear history' })).toBeNull();
  });

  it('asks before deleting, then the row is gone', async () => {
    serve([routine()]);
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Delete Prices' }));
    expect(screen.getByText('Delete “Prices” and its history?')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Delete' }));
    expect(apiMock).toHaveBeenCalledWith('/routines/r1', { key: 'k', method: 'DELETE' });
    expect(screen.queryByRole('row', { name: /Prices/ })).toBeNull();
  });
});
