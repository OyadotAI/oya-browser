/**
 * Unit tests for the panel's polled detail and optimistic activity lines.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import { freshLines, pushLine, useBrowserDetail } from '@/components/dashboard/browser/use-browser-detail';
import { OPTIMISTIC_MAX, OPTIMISTIC_TTL_MS, POLL_MS } from '@/components/dashboard/browser/constants';
import { api } from '@/lib/api-client';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.mocked(api).mockReset();
});

/** Answers the detail and session requests. */
const answer = (mode = 'human') =>
  vi
    .mocked(api)
    .mockImplementation(async (path: string) =>
      path.startsWith('/browsers/') ? { id: 'b', name: 'alpha' } : { control: { mode } },
    );

describe('useBrowserDetail', () => {
  it('keeps only the newest optimistic lines', () => {
    let lines: ReturnType<typeof pushLine> = [];
    for (let i = 0; i < OPTIMISTIC_MAX + 2; i++) lines = pushLine(lines, `l${i}`);
    expect(lines.map((l) => l.line)).toEqual(['l6', 'l5', 'l4', 'l3', 'l2']);
  });

  it('drops optimistic lines once the server has had time to log them', () => {
    const now = Date.now();
    const lines = [
      { ts: new Date(now - OPTIMISTIC_TTL_MS).toISOString(), line: 'old' },
      { ts: new Date(now).toISOString(), line: 'new' },
    ];
    expect(freshLines(lines, now).map((l) => l.line)).toEqual(['new']);
  });

  it('loads the detail and control mode, then polls again', async () => {
    answer();
    const onClose = vi.fn();
    const { result } = renderHook(() => useBrowserDetail('b', 'k', onClose));
    await act(async () => {});
    expect(result.current.detail?.name).toBe('alpha');
    expect(result.current.controlMode).toBe('human');
    await act(async () => vi.advanceTimersByTime(POLL_MS));
    expect(api).toHaveBeenCalledTimes(4);
  });

  it('keeps the agent mode when the session cannot be read', async () => {
    vi.mocked(api).mockImplementation(async (path: string) => {
      if (path.startsWith('/browsers/')) return { id: 'b' };
      throw new Error('no session');
    });
    const onClose = vi.fn();
    const { result } = renderHook(() => useBrowserDetail('b', 'k', onClose));
    await act(async () => {});
    expect(result.current.controlMode).toBe('agent');
  });

  it('closes the panel when the browser is gone', async () => {
    vi.mocked(api).mockRejectedValueOnce(Object.assign(new Error('gone'), { status: 404 }));
    const onClose = vi.fn();
    renderHook(() => useBrowserDetail('b', 'k', onClose));
    await act(async () => {});
    expect(onClose).toHaveBeenCalled();
  });

  it('stays open on other errors', async () => {
    vi.mocked(api).mockRejectedValueOnce(Object.assign(new Error('flaky'), { status: 500 }));
    const onClose = vi.fn();
    renderHook(() => useBrowserDetail('b', 'k', onClose));
    await act(async () => {});
    expect(onClose).not.toHaveBeenCalled();
  });
});
