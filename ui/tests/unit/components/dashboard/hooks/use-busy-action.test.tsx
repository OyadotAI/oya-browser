/**
 * Unit tests for useBusyAction: busy while the work runs, a failure becomes
 * an error toast.
 */
import { describe, it, expect, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

const toast = vi.fn();
vi.mock('@/components/dashboard/toast', () => ({ useToast: () => toast }));

import { useBusyAction } from '@/components/dashboard/hooks/use-busy-action';

/** A promise the test settles by hand. */
function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((r) => (resolve = r));
  return { promise, resolve };
}

describe('useBusyAction', () => {
  it('is busy while the work runs and idle after', async () => {
    const { result } = renderHook(() => useBusyAction());
    const work = deferred();
    let run!: Promise<void>;
    act(() => {
      run = result.current.run(() => work.promise);
    });
    expect(result.current.busy).toBe(true);
    await act(async () => {
      work.resolve();
      await run;
    });
    expect(result.current.busy).toBe(false);
  });

  it('toasts the failure and clears busy when the work throws', async () => {
    toast.mockClear();
    const { result } = renderHook(() => useBusyAction());
    await act(() => result.current.run(() => Promise.reject(new Error('Nope'))));
    expect(toast).toHaveBeenCalledWith('Nope', 'error');
    expect(result.current.busy).toBe(false);
  });
});
