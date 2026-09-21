/**
 * Unit tests for useControl: it polls the key's control data, never overlaps
 * loads, and runs actions that reload or report their failure.
 */
import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import { act, cleanup, renderHook } from '@testing-library/react';

vi.mock('@/lib/api-client', async (importActual) => ({
  ...(await importActual<typeof import('@/lib/api-client')>()),
  api: vi.fn(),
}));

import { api } from '@/lib/api-client';
import { useControl } from '@/components/dashboard/control/use-control';
import { CONTROL_POLL_MS } from '@/components/dashboard/control/constants';

/** What each control path answers with. */
const ANSWERS: Record<string, unknown> = {
  '/fleet': { routing: { strategy: 'priority', providers: [] } },
  '/gateway/sessions': { sessions: [{ id: 's1' }] },
  '/audit?limit=200': { events: [] },
  '/gateway/recordings': {},
  '/providers': { providers: [{ name: 'steel', configured: true }] },
};

/** Answers GETs from ANSWERS and anything else with {}. */
const answer = async (path: string) => ANSWERS[path] ?? {};

/** Lets pending promises settle. */
const settle = () => act(async () => {});

describe('useControl', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.mocked(api)
      .mockReset()
      .mockImplementation(answer as typeof api);
  });
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it('loads every view’s data with the key and derives routing from the fleet', async () => {
    const { result } = renderHook(() => useControl('k'));
    await settle();
    expect(api).toHaveBeenCalledWith('/fleet', { key: 'k' });
    expect(result.current.sessions).toEqual([{ id: 's1' }]);
    expect(result.current.recordings).toEqual([]);
    expect(result.current.choices).toEqual([{ name: 'steel', configured: true }]);
    expect(result.current.routing?.strategy).toBe('priority');
  });

  it('polls again every interval', async () => {
    renderHook(() => useControl('k'));
    await settle();
    const calls = vi.mocked(api).mock.calls.length;
    await act(async () => vi.advanceTimersByTime(CONTROL_POLL_MS));
    expect(vi.mocked(api).mock.calls.length).toBe(calls * 2);
  });

  it('does not load without a key', async () => {
    renderHook(() => useControl(''));
    await settle();
    expect(api).not.toHaveBeenCalled();
  });

  it('skips a poll while the previous load is still running', async () => {
    vi.mocked(api).mockImplementation(() => new Promise(() => {}));
    renderHook(() => useControl('k'));
    await act(async () => vi.advanceTimersByTime(CONTROL_POLL_MS * 2));
    expect(vi.mocked(api).mock.calls.length).toBe(Object.keys(ANSWERS).length);
  });

  it('reports a failed load and keeps the last data', async () => {
    const { result } = renderHook(() => useControl('k'));
    await settle();
    vi.mocked(api).mockRejectedValueOnce(new Error('down'));
    await act(() => result.current.refresh());
    expect(result.current.error).toBe('down');
    expect(result.current.sessions).toEqual([{ id: 's1' }]);
  });

  it('runs an action, reloads, and clears busy', async () => {
    const { result } = renderHook(() => useControl('k'));
    await settle();
    const fn = vi.fn(async () => {});
    await act(() => result.current.act('x', fn));
    expect(fn).toHaveBeenCalled();
    expect(result.current.busy).toBe('');
    expect(result.current.actionError).toBe('');
  });

  it('reports a failed action by its message', async () => {
    const { result } = renderHook(() => useControl('k'));
    await settle();
    await act(() => result.current.act('x', () => Promise.reject(new Error('nope'))));
    expect(result.current.actionError).toBe('nope');
    expect(result.current.busy).toBe('');
  });

  it('saves a provider from the form, then closes it with a notice', async () => {
    const { result } = renderHook(() => useControl('k'));
    await settle();
    act(() => result.current.toggleAdd());
    act(() => result.current.edit({ name: 'mine', wsUrl: 'ws://h' }));
    await act(async () => result.current.submitAdd({ preventDefault() {} } as never));
    await settle();
    const [path, init] = vi.mocked(api).mock.calls.find(([p]) => p === '/gateway/providers')!;
    expect(path).toBe('/gateway/providers');
    expect(init).toMatchObject({ method: 'POST', body: { name: 'mine', type: 'cdp', wsUrl: 'ws://h' } });
    expect(result.current.showAdd).toBe(false);
    expect(result.current.draft.name).toBe('');
    expect(result.current.notice).toMatch(/^Provider saved/);
  });

  it('drops a typed vendor key when the form is cancelled', async () => {
    const { result } = renderHook(() => useControl('k'));
    await settle();
    act(() => result.current.toggleAdd());
    act(() => result.current.edit({ apiKey: 'secret', name: 'n' }));
    act(() => result.current.cancelAdd());
    expect(result.current.showAdd).toBe(false);
    expect(result.current.draft).toMatchObject({ apiKey: '', name: 'n' });
  });
});
