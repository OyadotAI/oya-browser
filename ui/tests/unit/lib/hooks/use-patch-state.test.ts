/**
 * Unit tests for usePatchState: updates merge, and patch stays the same function.
 */
import { describe, it, expect, afterEach } from 'vitest';
import { act, cleanup, renderHook } from '@testing-library/react';
import { usePatchState } from '@/lib/hooks/use-patch-state';

afterEach(cleanup);

describe('usePatchState', () => {
  it('merges partial updates and updates computed from the current state', () => {
    const { result } = renderHook(() => usePatchState({ a: 1, b: 'x' }));
    act(() => result.current[1]({ b: 'y' }));
    act(() => result.current[1]((s) => ({ a: s.a + 1 })));
    expect(result.current[0]).toEqual({ a: 2, b: 'y' });
  });

  it('keeps patch stable across renders', () => {
    const { result } = renderHook(() => usePatchState({ a: 1 }));
    const first = result.current[1];
    act(() => first({ a: 2 }));
    expect(result.current[1]).toBe(first);
  });
});
