/**
 * Unit tests for the auth session: it is restored from the refresh cookie on
 * load, renews itself before expiry, and sign-in, sign-up and sign-out
 * update it.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import type { PropsWithChildren } from 'react';
import { AuthProvider, useAuth } from '@/components/auth-provider';
import * as api from '@/lib/api';

vi.mock('@/lib/api', () => ({
  login: vi.fn(),
  signup: vi.fn(),
  getProfile: vi.fn(),
  refreshToken: vi.fn(),
  logout: vi.fn(async () => {}),
}));

const USER = { id: 'u1', email: 'a@b' };
/** An unsigned JWT expiring at `exp` (seconds). */
const jwt = (exp: number) => `e30.${btoa(JSON.stringify({ exp }))}.sig`;
/** Renders useAuth inside the provider. */
const setup = () =>
  renderHook(() => useAuth(), {
    wrapper: ({ children }: PropsWithChildren) => <AuthProvider>{children}</AuthProvider>,
  });

beforeEach(() => vi.mocked(api.getProfile).mockResolvedValue(USER));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.useRealTimers();
  localStorage.clear();
  sessionStorage.clear();
  document.cookie = 'oya_session=; expires=Thu, 01 Jan 1970 00:00:00 GMT; path=/';
});

describe('AuthProvider', () => {
  it('asks nothing of the server for an anonymous visitor', async () => {
    const { result } = setup();
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(api.refreshToken).not.toHaveBeenCalled();
    expect(result.current.user).toBeNull();
  });

  it('restores the session from the refresh cookie on load', async () => {
    document.cookie = 'oya_session=1; path=/';
    vi.mocked(api.refreshToken).mockResolvedValue({ access_token: 'tok' });
    const { result } = setup();
    await waitFor(() => expect(result.current.user).toEqual(USER));
    expect(result.current.token).toBe('tok');
    expect(result.current.loading).toBe(false);
  });

  it('clears everything stored when the refresh is refused', async () => {
    localStorage.setItem('oya_refresh_token', 'old');
    sessionStorage.setItem('oya_console_key', 'k');
    vi.mocked(api.refreshToken).mockRejectedValue(new Error('expired'));
    const { result } = setup();
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(api.refreshToken).toHaveBeenCalledWith('old');
    expect(localStorage.getItem('oya_refresh_token')).toBeNull();
    expect(sessionStorage.getItem('oya_console_key')).toBeNull();
  });

  it('renews the token a minute before it expires', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const exp = Math.floor(Date.now() / 1000) + 3600;
    vi.mocked(api.login).mockResolvedValue({ access_token: jwt(exp), user: USER });
    vi.mocked(api.refreshToken).mockResolvedValue({ access_token: 'renewed' });
    document.cookie = 'oya_session=1; path=/';
    const { result } = setup();
    await waitFor(() => expect(result.current.loading).toBe(false));
    vi.mocked(api.refreshToken).mockClear();
    await act(() => result.current.login('a@b', 'pw'));
    await act(() => vi.advanceTimersByTimeAsync((3600 - 60) * 1000));
    expect(api.refreshToken).toHaveBeenCalledOnce();
    expect(result.current.token).toBe('renewed');
  });

  it('signs in, keeping a returned refresh token only for the cross-origin fallback', async () => {
    vi.mocked(api.login).mockResolvedValue({ access_token: 'a', refresh_token: 'r', user: USER });
    const { result } = setup();
    await act(() => result.current.login('a@b', 'pw'));
    expect(result.current.user).toEqual(USER);
    expect(localStorage.getItem('oya_refresh_token')).toBe('r');
  });

  it('signs up and adopts the session the server started, without a second sign-in', async () => {
    vi.mocked(api.signup).mockResolvedValue({ access_token: 'a', user: USER });
    const { result } = setup();
    await act(() => result.current.signup('a@b', 'pw', 'Ann', 'captcha'));
    expect(api.signup).toHaveBeenCalledWith('a@b', 'pw', 'Ann', 'captcha');
    expect(api.login).not.toHaveBeenCalled();
    expect(result.current.user).toEqual(USER);
  });

  it('signs out here and on the server', async () => {
    vi.mocked(api.login).mockResolvedValue({ access_token: 'a', user: USER });
    sessionStorage.setItem('oya_project_credential', 'c');
    const { result } = setup();
    await act(() => result.current.login('a@b', 'pw'));
    act(() => result.current.logout());
    expect(result.current.user).toBeNull();
    expect(sessionStorage.getItem('oya_project_credential')).toBeNull();
    expect(api.logout).toHaveBeenCalledOnce();
  });

  it('adopts a profile the server returned', async () => {
    const { result } = setup();
    act(() => result.current.applyProfile({ ...USER, display_name: 'New' }));
    expect(result.current.user?.display_name).toBe('New');
  });

  it('throws when used outside the provider', () => {
    expect(() => renderHook(() => useAuth())).toThrow('useAuth must be used within AuthProvider');
  });
});
