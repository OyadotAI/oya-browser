/** Customer selection is not overwritten by older search or adjustment refreshes. */
import { afterEach, expect, it, vi } from 'vitest';
import { act, cleanup, renderHook } from '@testing-library/react';
import { useLookup } from '@/components/admin/use-lookup';
import type { Found } from '@/components/admin/types';

afterEach(cleanup);

it('ignores a completed adjustment refresh after selecting another customer', async () => {
  const lookup = vi.fn(async (email: string) => ({ profile: { id: email, email } }) as Found);
  const { result } = renderHook(() => useLookup(lookup));
  await act(() => result.current.search('first@example.com'));
  const oldRefresh = result.current.refresh;
  await act(() => result.current.search('second@example.com'));
  await act(() => oldRefresh());
  expect(result.current.found?.profile.email).toBe('second@example.com');
  expect(lookup).toHaveBeenCalledTimes(2);
});

it('drops a slow search once a newer search has finished', async () => {
  let finish!: (found: Found) => void;
  const lookup = vi
    .fn()
    .mockImplementationOnce(
      () =>
        new Promise<Found>((resolve) => {
          finish = resolve;
        }),
    )
    .mockResolvedValueOnce({ profile: { id: 'second', email: 'second@example.com' } });
  const { result } = renderHook(() => useLookup(lookup));
  let first!: Promise<void>;
  act(() => {
    first = result.current.search('first@example.com');
  });
  await act(() => result.current.search('second@example.com'));
  await act(async () => {
    finish({ profile: { id: 'first', email: 'first@example.com' } } as Found);
    await first;
  });
  expect(result.current.found?.profile.id).toBe('second');
});
