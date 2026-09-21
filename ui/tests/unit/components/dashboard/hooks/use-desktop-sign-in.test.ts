/**
 * Unit tests for useDesktopSignIn: it asks for a pairing link for the chosen
 * profile, and a failure is toasted.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { act, renderHook } from '@testing-library/react';

const toast = vi.fn();
vi.mock('@/components/dashboard/toast', () => ({ useToast: () => toast }));
vi.mock('@/components/dashboard/config', () => ({ desktopSignInUrl: vi.fn() }));

import { desktopSignInUrl } from '@/components/dashboard/config';
import { useDesktopSignIn } from '@/components/dashboard/hooks/use-desktop-sign-in';

const signIn = vi.mocked(desktopSignInUrl);

describe('useDesktopSignIn', () => {
  beforeEach(() => {
    toast.mockClear();
    signIn.mockReset();
  });

  it('asks for a link for the chosen profile under the key', async () => {
    signIn.mockResolvedValueOnce('#paired');
    const { result } = renderHook(() => useDesktopSignIn('key-1'));
    await act(() => result.current.open('p2'));
    expect(signIn).toHaveBeenCalledWith('key-1', 'p2');
    expect(window.location.hash).toBe('#paired');
  });

  it('toasts why the link could not be made', async () => {
    signIn.mockRejectedValueOnce(new Error('Pairing is off'));
    const { result } = renderHook(() => useDesktopSignIn('k'));
    await act(() => result.current.open());
    expect(toast).toHaveBeenCalledWith('Pairing is off', 'error');
    expect(result.current.busy).toBe(false);
  });
});
