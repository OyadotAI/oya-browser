/**
 * Unit tests for the desktop's sign-in request: `?connect=desktop` is
 * remembered, and answered with one pairing link once a key is known.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook } from '@testing-library/react';

const open = vi.fn();
vi.mock('@/components/dashboard/hooks/use-desktop-sign-in', () => ({ useDesktopSignIn: () => ({ open }) }));

import { rememberDesktopConnect, useConnectDesktop } from '@/components/dashboard/hooks/use-connect-desktop';

describe('useConnectDesktop', () => {
  beforeEach(() => {
    open.mockClear();
    window.localStorage.clear();
    window.history.replaceState(null, '', '/dashboard');
  });

  it('pairs once a key is known after a ?connect=desktop visit, and only once', () => {
    window.history.replaceState(null, '', '/dashboard?connect=desktop');
    rememberDesktopConnect();
    const { rerender } = renderHook(({ key }) => useConnectDesktop(key), { initialProps: { key: '' } });
    expect(open).not.toHaveBeenCalled();
    rerender({ key: 'k1' });
    rerender({ key: 'k2' });
    expect(open).toHaveBeenCalledTimes(1);
  });

  it('does nothing on a plain dashboard visit', () => {
    rememberDesktopConnect();
    renderHook(() => useConnectDesktop('k1'));
    expect(open).not.toHaveBeenCalled();
  });
});

it('preserves a validated callback across authentication and clears it from the visible URL', () => {
  open.mockClear();
  window.localStorage.clear();
  const callback = 'http://127.0.0.1:41777/oya-desktop/callback',
    state = 'b'.repeat(64);
  window.history.replaceState(null, '', '/dashboard?' + new URLSearchParams({ connect: 'desktop', callback, state }));
  rememberDesktopConnect();
  expect(window.location.search).toBe('');
  renderHook(() => useConnectDesktop('signed-in-key'));
  expect(open).toHaveBeenCalledWith(undefined, expect.objectContaining({ callback, state }));
  expect(window.localStorage.getItem('oya.connectDesktop')).toBeNull();
});
it('does not downgrade an invalid native return into an unsolicited protocol link', () => {
  open.mockClear();
  window.localStorage.clear();
  window.history.replaceState(null, '', '/dashboard?connect=desktop&callback=https://evil.test&state=guess');
  rememberDesktopConnect();
  renderHook(() => useConnectDesktop('signed-in-key'));
  expect(open).not.toHaveBeenCalled();
});
