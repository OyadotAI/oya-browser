/**
 * Unit tests for useDesktopSignIn: it asks for a pairing link for the chosen
 * profile, a failure is toasted, and a link no app answers is noticed.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, renderHook } from '@testing-library/react';

const toast = vi.fn();
vi.mock('@/components/dashboard/toast', () => ({ useToast: () => toast }));
vi.mock('@/components/dashboard/config', () => ({ desktopSignInUrl: vi.fn() }));

import { desktopSignInUrl } from '@/components/dashboard/config';
import { useDesktopSignIn } from '@/components/dashboard/hooks/use-desktop-sign-in';
import { APP_OPEN_WAIT_MS } from '@/components/dashboard/hooks/constants';

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

  describe('when the link opens nothing', () => {
    beforeEach(() => vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] }));
    afterEach(() => vi.useRealTimers());

    it('says the desktop did not open when the page keeps focus', async () => {
      signIn.mockResolvedValueOnce('#paired');
      const { result } = renderHook(() => useDesktopSignIn('k'));
      await act(() => result.current.open());
      expect(result.current.notOpened).toBe(false);
      act(() => vi.advanceTimersByTime(APP_OPEN_WAIT_MS));
      expect(result.current.notOpened).toBe(true);
    });

    it('stays quiet when the app takes focus', async () => {
      signIn.mockResolvedValueOnce('#paired');
      const { result } = renderHook(() => useDesktopSignIn('k'));
      await act(() => result.current.open());
      act(() => {
        window.dispatchEvent(new Event('blur'));
        vi.advanceTimersByTime(APP_OPEN_WAIT_MS);
      });
      expect(result.current.notOpened).toBe(false);
    });
  });
});
