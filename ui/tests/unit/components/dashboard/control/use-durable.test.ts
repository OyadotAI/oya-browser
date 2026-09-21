/**
 * Unit tests for useDurable: polling the project overview and its members,
 * seeding the settings drafts without overwriting edits, and running actions
 * that surface once-shown secrets or failures.
 */
import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import { act, cleanup, renderHook } from '@testing-library/react';

vi.mock('@/lib/api-client', async (importActual) => ({
  ...(await importActual<typeof import('@/lib/api-client')>()),
  api: vi.fn(),
}));

import { api } from '@/lib/api-client';
import { seedDrafts, settingsBody, shownOnce, useDurable } from '@/components/dashboard/control/use-durable';
import { DURABLE_POLL_MS } from '@/components/dashboard/control/constants';
import type { ProjectSettings } from '@/components/dashboard/control/types';

/** Server settings. */
const SETTINGS: ProjectSettings = {
  maxConcurrent: 2,
  budgetUsd: null,
  recordingDays: 7,
  auditDays: 30,
  rates: { a: 1 },
  policy: { region: 'eu' },
};
/** An administrator's overview: credentials present, so members load too. */
const OVERVIEW = {
  project: { id: 'p', name: 'P', settings: SETTINGS },
  sessions: [],
  draining: false,
  events: [],
  credentials: [],
};

/** Lets pending promises settle. */
const settle = () => act(async () => {});

describe('useDurable rules', () => {
  it('seeds untouched drafts from the server and keeps edited ones', () => {
    const seeded = seedDrafts({ settings: null, rates: '{}', policy: '{}' }, SETTINGS);
    expect(seeded.settings).toBe(SETTINGS);
    expect(JSON.parse(seeded.rates)).toEqual({ a: 1 });
    const edited = seedDrafts({ settings: SETTINGS, rates: '{"b":2}', policy: '{}' }, { ...SETTINGS, rates: {} });
    expect(edited.rates).toBe('{"b":2}');
  });

  it('refuses to build a settings body from invalid JSON', () => {
    expect(() => settingsBody({ settings: SETTINGS, rates: 'nope', policy: '{}' })).toThrow();
    expect(settingsBody({ settings: SETTINGS, rates: '{"x":3}', policy: '{}' })).toMatchObject({
      rates: { x: 3 },
      auditDays: 30,
    });
  });

  it('finds a once-shown secret as token, secret or code', () => {
    expect(shownOnce({ token: 't' })).toBe('t');
    expect(shownOnce({ secret: 's' })).toBe('s');
    expect(shownOnce({ code: 'c' })).toBe('c');
    expect(shownOnce({})).toBeUndefined();
  });
});

describe('useDurable', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.mocked(api)
      .mockReset()
      .mockImplementation((async (path: string) =>
        path === '/control/members' ? { members: [{ userId: 'u1', role: 'operator' }] } : OVERVIEW) as typeof api);
  });
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it('loads the overview, its members and seeds the drafts', async () => {
    const { result } = renderHook(() => useDurable('k'));
    await settle();
    expect(api).toHaveBeenCalledWith('/control', { key: 'k', method: 'GET', body: undefined });
    expect(result.current.data?.project.name).toBe('P');
    expect(result.current.members).toEqual([{ userId: 'u1', role: 'operator' }]);
    expect(result.current.drafts.settings).toBe(SETTINGS);
  });

  it('polls on an interval', async () => {
    renderHook(() => useDurable('k'));
    await settle();
    const calls = vi.mocked(api).mock.calls.length;
    await act(async () => vi.advanceTimersByTime(DURABLE_POLL_MS));
    expect(vi.mocked(api).mock.calls.length).toBe(calls * 2);
  });

  it('reports a failed first load', async () => {
    vi.mocked(api).mockRejectedValueOnce(new Error('offline'));
    const { result } = renderHook(() => useDurable('k'));
    await settle();
    expect(result.current.error).toBe('offline');
    expect(result.current.data).toBeNull();
  });

  it('keeps a secret an action returns and reloads', async () => {
    const { result } = renderHook(() => useDurable('k'));
    await settle();
    vi.mocked(api).mockResolvedValueOnce({ code: 'invite-1' });
    await act(() => result.current.act('/members/invite', 'POST', { role: 'viewer' }));
    expect(api).toHaveBeenCalledWith('/control/members/invite', { key: 'k', method: 'POST', body: { role: 'viewer' } });
    expect(result.current.secret).toBe('invite-1');
    expect(result.current.busy).toBe(false);
  });

  it('reports a failed action and clears busy', async () => {
    const { result } = renderHook(() => useDurable('k'));
    await settle();
    vi.mocked(api).mockRejectedValueOnce(new Error('forbidden'));
    await act(() => result.current.act('/credentials/c1', 'DELETE'));
    expect(result.current.error).toBe('forbidden');
    expect(result.current.busy).toBe(false);
  });
});
