/**
 * Unit tests for the recording dialog, through what the user sees and does:
 * start recording, watch steps arrive, stop, name and save.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));
vi.mock('@/lib/live-stream', () => ({ subscribeFrames: vi.fn(() => () => {}) }));
vi.mock('@/components/dashboard/live-view', () => ({ default: () => <div>live view</div> }));

import { api } from '@/lib/api-client';
import RecordDialog from '@/components/dashboard/playbooks/record-dialog';
import { ToastProvider } from '@/components/dashboard/toast';
import { RECORD_POLL_MS } from '@/components/dashboard/playbooks/constants';
import type { BrowserRow } from '@/components/dashboard/types';

const apiMock = vi.mocked(api);
const browsers = [{ id: 'b1', name: 'one' }] as BrowserRow[];
const steps = [{ action: 'navigate', url: 'https://a.com' }];

/** Answers the record route by mode, the save route with a saved playbook, and the rest with `{}`. */
function serve(recording: { current: boolean }) {
  apiMock.mockImplementation(async (path, opts) => {
    const mode = (opts?.body as { mode?: string } | undefined)?.mode;
    if (path.endsWith('/record') && mode === 'status') return { recording: recording.current, steps: [], secrets: [] };
    if (path.endsWith('/record')) return { recording: mode === 'start', steps, secrets: [] };
    if (path.endsWith('/playbooks')) return { name: 'login', steps: 1 };
    return {};
  });
}

/** Renders the dialog. */
async function setup() {
  const onClose = vi.fn();
  const onSaved = vi.fn();
  const view = render(
    <ToastProvider>
      <RecordDialog apiKey="k" browsers={browsers} onClose={onClose} onSaved={onSaved} />
    </ToastProvider>,
  );
  await act(async () => {});
  return { onClose, onSaved, view };
}

/** The request bodies sent to a path, in order. */
const bodies = (suffix: string) => apiMock.mock.calls.filter(([p]) => p.endsWith(suffix)).map(([, o]) => o?.body);

describe('RecordDialog', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it('checks for a stopped recording on open', async () => {
    serve({ current: false });
    await setup();
    expect(bodies('/record')).toEqual([{ mode: 'status' }]);
    expect(screen.getByRole('button', { name: 'Start recording' })).toBeTruthy();
  });

  it('starts by taking control, then shows the live view and steps', async () => {
    serve({ current: false });
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Start recording' }));
    expect(bodies('/control')).toEqual([{ action: 'acquire', force: false }]);
    expect(screen.getByText('Recording')).toBeTruthy();
    expect(screen.getByText('live view')).toBeTruthy();
    expect(screen.getByText('1. navigate https://a.com')).toBeTruthy();
  });

  it('polls the steps while recording', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const recording = { current: true };
    serve(recording);
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Start recording' }));
    await act(() => vi.advanceTimersByTimeAsync(RECORD_POLL_MS));
    expect(bodies('/record').filter((b) => (b as { mode: string }).mode === 'status').length).toBeGreaterThan(1);
  });

  it('saves only with a valid name, a description and steps', async () => {
    serve({ current: false });
    const { onSaved, onClose } = await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Start recording' }));
    await userEvent.click(screen.getByRole('button', { name: 'Stop' }));
    const saveButton = screen.getByRole('button', { name: 'Save playbook' }) as HTMLButtonElement;
    expect(saveButton.disabled).toBe(true);
    await userEvent.type(screen.getByLabelText('Name'), 'login');
    await userEvent.type(screen.getByLabelText('What does this flow do?'), 'Log in');
    await userEvent.click(saveButton);
    expect(bodies('/playbooks')).toEqual([{ name: 'login', prompt: 'Log in', steps, secrets: [] }]);
    expect(onSaved).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it('hands control back when it goes away while recording', async () => {
    serve({ current: false });
    const { view } = await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Start recording' }));
    view.unmount();
    expect(bodies('/record').at(-1)).toEqual({ mode: 'stop', resume: true });
  });
});
