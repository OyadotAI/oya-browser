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
import { CONTROL_RENEW_MS, RECORD_POLL_MS } from '@/components/dashboard/playbooks/constants';
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

/** Renders the dialog. `renewCredential` re-renders it under a new console credential. */
async function setup() {
  const onClose = vi.fn();
  const onSaved = vi.fn();
  const dialog = (apiKey: string) => (
    <ToastProvider>
      <RecordDialog apiKey={apiKey} browsers={browsers} onClose={onClose} onSaved={onSaved} />
    </ToastProvider>
  );
  const view = render(dialog('k'));
  await act(async () => {});
  const renewCredential = () => act(async () => view.rerender(dialog('k2')));
  return { onClose, onSaved, view, renewCredential };
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

  it('keeps recording when the console credential is renewed', async () => {
    serve({ current: true });
    const { renewCredential, view } = await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Start recording' }));
    await renewCredential();
    expect(bodies('/record').some((b) => (b as { mode: string }).mode === 'stop')).toBe(false);
    expect(screen.getByText('Recording')).toBeTruthy();
    view.unmount();
    expect(apiMock.mock.calls.at(-1)?.[1]?.key).toBe('k2');
  });

  it('renews the hold while recording, without asking to take it again', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    serve({ current: true });
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Start recording' }));
    await act(() => vi.advanceTimersByTimeAsync(CONTROL_RENEW_MS));
    expect(bodies('/control')).toEqual([{ action: 'acquire', force: false }, { action: 'renew' }]);
  });

  it('takes the hold again when it lapsed, and says so when it cannot', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    serve({ current: true });
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Start recording' }));
    const served = apiMock.getMockImplementation()!;
    apiMock.mockImplementation(async (path, opts) => {
      if (path.endsWith('/control')) throw new Error('control_busy');
      return served(path, opts);
    });
    await act(() => vi.advanceTimersByTimeAsync(CONTROL_RENEW_MS));
    expect(bodies('/control').slice(1)).toEqual([{ action: 'renew' }, { action: 'acquire', force: false }]);
    expect(screen.getByText(/could not keep control/i)).toBeTruthy();
  });

  it('warns in the last minutes before the recording stops on its own', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    apiMock.mockImplementation(async (path, opts) => {
      const mode = (opts?.body as { mode?: string } | undefined)?.mode;
      if (path.endsWith('/record'))
        return { recording: true, steps, secrets: [], minutesLeft: mode === 'status' ? 4 : 30 };
      return {};
    });
    await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Start recording' }));
    expect(screen.queryByText(/stops on its own/)).toBeNull();
    await act(() => vi.advanceTimersByTimeAsync(RECORD_POLL_MS));
    expect(
      screen.getByText('This recording stops on its own in 4 minutes. Stop and save it, then record the rest.'),
    ).toBeTruthy();
  });

  it('hands control back when it goes away while recording', async () => {
    serve({ current: false });
    const { view } = await setup();
    await userEvent.click(screen.getByRole('button', { name: 'Start recording' }));
    view.unmount();
    expect(bodies('/record').at(-1)).toEqual({ mode: 'stop', resume: true });
  });
});
