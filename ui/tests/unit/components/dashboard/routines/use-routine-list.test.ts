/**
 * Unit tests for the routine list hook (routines/use-routine-list.ts): a save's
 * answer shows at once, and a poll that started before it cannot put the old
 * routine back.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act, renderHook } from '@testing-library/react';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));

import { api } from '@/lib/api-client';
import { useRoutineList } from '@/components/dashboard/routines/use-routine-list';
import type { Routine } from '@/components/dashboard/routines/types';

const apiMock = vi.mocked(api);

/** A routine, on or off. */
const routine = (enabled: boolean): Routine => ({
  id: 'r1',
  createdAt: 0,
  name: 'Prices',
  prompt: 'Check',
  schedule: { kind: 'every', n: 1, unit: 'hours' },
  enabled,
  target: 'cloud',
  lastRunAt: null,
  nextRunAt: enabled ? 1 : null,
  runs: [],
});

describe('useRoutineList', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });

  it('drops a poll’s answer that started before a save landed', async () => {
    let answerSlowPoll: (value: unknown) => void = () => {};
    apiMock.mockResolvedValueOnce({ routines: [routine(true)] });
    apiMock.mockImplementationOnce(() => new Promise((resolve) => (answerSlowPoll = resolve)));
    const { result } = renderHook(() => useRoutineList('k'));
    await act(async () => {});
    let slow: Promise<void> = Promise.resolve();
    act(() => {
      slow = result.current.refresh() as Promise<void>;
    });
    act(() => result.current.replace(routine(false)));
    await act(async () => (answerSlowPoll({ routines: [routine(true)] }), await slow));
    expect(result.current.routines?.[0].enabled).toBe(false);
  });

  it('shows a poll that started after the save', async () => {
    apiMock.mockResolvedValueOnce({ routines: [routine(true)] });
    const { result } = renderHook(() => useRoutineList('k'));
    await act(async () => {});
    act(() => result.current.replace(routine(false)));
    apiMock.mockResolvedValueOnce({ routines: [routine(true)] });
    await act(() => result.current.refresh() as Promise<void>);
    expect(result.current.routines?.[0].enabled).toBe(true);
  });
});
