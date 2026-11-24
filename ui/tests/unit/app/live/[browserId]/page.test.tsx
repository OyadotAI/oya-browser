/**
 * Unit tests for the live page: the shared-link token wins over the tab's
 * credential and leaves the URL, frames stream with an fps meter, control
 * cycles agent → human → paused → agent, and failed input shows.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import LiveBrowserPage from '@/app/live/[browserId]/page';
import { controlFor } from '@/app/live/[browserId]/live-session';
import { subscribeFrames } from '@/lib/live-stream';
import { api } from '@/lib/api-client';

vi.mock('next/navigation', () => ({ useParams: () => ({ browserId: 'b1' }) }));
vi.mock('@/lib/live-stream', () => ({ subscribeFrames: vi.fn(() => () => {}) }));
vi.mock('@/lib/api-client', async (real) => ({ ...(await real<object>()), api: vi.fn() }));
/** What the page last handed the live view. */
let viewProps: Record<string, unknown> = {};
vi.mock('@/components/dashboard/live-view', () => ({
  default: (p: Record<string, unknown>) => ((viewProps = p), (<i data-testid="view" />)),
}));

/** The frame and loss callbacks the page gave the stream. */
const streamCallbacks = () => vi.mocked(subscribeFrames).mock.calls[0].slice(2) as [(f: string) => void, () => void];

beforeEach(() => {
  vi.mocked(api).mockImplementation(async (path: string) =>
    path.endsWith('/control') ? { mode: 'human' } : { name: 'QA', canControl: true },
  );
});
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.useRealTimers();
  sessionStorage.clear();
  history.replaceState(null, '', '/');
});

describe('LiveBrowserPage', () => {
  it('asks for a key when there is neither a share token nor a console credential and omits dead controls', async () => {
    render(<LiveBrowserPage />);
    expect((await screen.findByText(/Connect your Oya key/)).textContent).toContain('to view this browser.');
    expect(screen.queryByRole('button', { name: 'Take control' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Reconnect' })).toBeNull();
    expect(api).not.toHaveBeenCalled();
  });

  it('uses the share token from the fragment, strips it, and keeps it across a same-tab reload', async () => {
    history.replaceState(null, '', '/live/b1?x=1#t=shared');
    sessionStorage.setItem('oya_console_key', 'own');
    const first = render(<LiveBrowserPage />);
    expect(await screen.findByRole('heading', { name: 'QA' })).toBeTruthy();
    expect(vi.mocked(api).mock.calls[0]).toEqual(['/browsers/b1', { key: 'shared' }]);
    expect(window.location.hash).toBe('');
    expect(window.location.search).toBe('?x=1');
    first.unmount();
    vi.mocked(api).mockClear();
    render(<LiveBrowserPage />);
    expect(await screen.findByRole('heading', { name: 'QA' })).toBeTruthy();
    expect(vi.mocked(api).mock.calls[0]).toEqual(['/browsers/b1', { key: 'shared' }]);
  });

  it('syncs the initial control mode from the server instead of hardcoding agent', async () => {
    sessionStorage.setItem('oya_console_key', 'k');
    vi.mocked(api).mockImplementation(async (path: string) => {
      if (path === '/control/sessions/b1') return { control: { mode: 'paused' } };
      return { name: 'QA', canControl: true };
    });
    render(<LiveBrowserPage />);
    expect(await screen.findByRole('button', { name: 'Resume agent' })).toBeTruthy();
  });

  it('streams frames, reports fps each second, and shows a lost stream', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    sessionStorage.setItem('oya_console_key', 'k');
    render(<LiveBrowserPage />);
    await screen.findByRole('heading', { name: 'QA' });
    const [onFrame, onLost] = streamCallbacks();
    act(() => (onFrame('f1'), onFrame('f2')));
    await act(() => vi.advanceTimersByTimeAsync(1000));
    expect(viewProps).toMatchObject({ frameSrc: 'f2', fps: 2 });
    act(() => onLost());
    expect(screen.getByRole('alert').textContent).toBe('Disconnected. Reconnecting…');
    expect(viewProps.frameSrc).toBe('f2');
  });

  it('keeps a viewer watch-only even when a person holds control', async () => {
    history.replaceState(null, '', '/live/b1#t=viewer');
    vi.mocked(api).mockResolvedValue({ name: 'Watched tab', controlMode: 'human', canControl: false });
    render(<LiveBrowserPage />);
    await screen.findByRole('heading', { name: 'Watched tab' });
    expect(viewProps.interactive).toBe(false);
    expect(screen.queryByRole('button', { name: 'Release control' })).toBeNull();
  });

  it('takes control and makes the view interactive', async () => {
    sessionStorage.setItem('oya_console_key', 'k');
    render(<LiveBrowserPage />);
    await userEvent.click(await screen.findByRole('button', { name: 'Take control' }));
    expect(vi.mocked(api).mock.calls.at(-1)).toEqual([
      '/control/sessions/b1/control',
      { key: 'k', method: 'POST', body: { action: 'acquire' } },
    ]);
    expect(await screen.findByRole('button', { name: 'Release control' })).toBeTruthy();
    expect(viewProps.interactive).toBe(true);
  });

  it('shows a refused input and still rejects it for the live view', async () => {
    sessionStorage.setItem('oya_console_key', 'k');
    render(<LiveBrowserPage />);
    await screen.findByRole('heading', { name: 'QA' });
    vi.mocked(api).mockResolvedValue({ ok: false, error: 'Not in control' });
    const send = viewProps.send as (a: string) => Promise<unknown>;
    await act(() => expect(send('click')).rejects.toThrow('Not in control'));
    expect(screen.getByRole('alert').textContent).toBe('Not in control');
  });

  it('reconnects on request', async () => {
    sessionStorage.setItem('oya_console_key', 'k');
    render(<LiveBrowserPage />);
    await screen.findByRole('heading', { name: 'QA' });
    await userEvent.click(screen.getByRole('button', { name: 'Reconnect' }));
    await screen.findByRole('heading', { name: 'QA' });
    expect(vi.mocked(api).mock.calls.filter(([p]) => p === '/browsers/b1')).toHaveLength(2);
  });
});

describe('controlFor', () => {
  it('cycles agent → acquire, human → release, anything else → resume', () => {
    expect(controlFor('agent')).toEqual({ action: 'acquire', label: 'Take control' });
    expect(controlFor('human')).toEqual({ action: 'release', label: 'Release control' });
    expect(controlFor('paused')).toEqual({ action: 'resume', label: 'Resume agent' });
    expect(controlFor('toString').action).toBe('resume');
  });
});
