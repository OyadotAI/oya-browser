/** Independent feed requests retain stale snapshots safely and reject obsolete credential responses. */
import { afterEach, expect, it, vi } from 'vitest';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { useAdminFeed } from '@/components/admin/use-admin-feed';

afterEach(cleanup);

it('retains a successful snapshot through ordinary refresh failure, then clears it on access denial', async () => {
  const source = vi
    .fn()
    .mockResolvedValueOnce({ count: 7 })
    .mockRejectedValueOnce(new Error('offline'))
    .mockRejectedValueOnce(Object.assign(new Error('Staff permission removed'), { status: 403 }));
  const { result } = renderHook(() => useAdminFeed<{ count: number }>('token', source));
  await waitFor(() => expect(result.current.value?.count).toBe(7));
  act(() => result.current.refresh());
  await waitFor(() => expect(result.current.error).toBe('offline'));
  expect(result.current.value?.count).toBe(7);
  act(() => result.current.refresh());
  await waitFor(() => expect(result.current.denied).toBe(true));
  expect(result.current.value).toBeNull();
});

it('never exposes its credential or applies a delayed response from the previous token', async () => {
  let finish!: (value: { count: number }) => void;
  const source = vi
    .fn()
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    )
    .mockResolvedValueOnce({ count: 2 });
  const { result, rerender } = renderHook(({ token }) => useAdminFeed<{ count: number }>(token, source), {
    initialProps: { token: 'first-secret' },
  });
  rerender({ token: 'second-secret' });
  await waitFor(() => expect(result.current.value?.count).toBe(2));
  await act(async () => {
    finish({ count: 100 });
  });
  expect(result.current.value?.count).toBe(2);
  expect(JSON.stringify(result.current)).not.toContain('secret');
  expect(Object.keys(result.current)).not.toContain('token');
});
