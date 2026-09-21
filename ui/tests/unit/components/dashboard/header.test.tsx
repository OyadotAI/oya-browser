/**
 * Unit tests for the dashboard header: the health badge and its polling, the
 * account menu, and the settings button.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { act, cleanup, render, renderHook, screen, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

/** The logout spy handed out by useAuth. */
const logout = vi.fn();

vi.mock('@/lib/api', () => ({ apiUrl: (p: string) => p }));
vi.mock('@/components/auth-provider', () => ({
  useAuth: () => ({ user: { email: 'ada@example.com', display_name: 'Ada' }, token: null, logout }),
}));
vi.mock('@/components/dashboard/profile-dialog', () => ({
  default: ({ open }: { /** Whether it shows. */ open: boolean }) => (open ? <div role="dialog">Profile</div> : null),
}));
vi.mock('@/components/dashboard/toast', () => ({ useToast: () => vi.fn() }));
vi.mock('@/components/theme-toggle', () => ({ default: () => null }));

import Header from '@/components/dashboard/header';
import { HEALTH_POLL_MS } from '@/components/dashboard/header/constants';
import { useHealth } from '@/components/dashboard/header/use-health';

/** Makes /health answer with `status` (or fail with `httpStatus`). */
function stubHealth(status: string, httpStatus = 200) {
  const spy = vi.fn(async () => ({ ok: httpStatus < 400, json: async () => ({ status }) }) as Response);
  vi.stubGlobal('fetch', spy);
  return spy;
}

describe('useHealth', () => {
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it('reads "healthy" when the server says ok', async () => {
    stubHealth('ok');
    const { result } = renderHook(() => useHealth());
    await vi.waitFor(() => expect(result.current).toEqual({ status: 'healthy', ok: true }));
  });

  it('shows the server status when it is not ok', async () => {
    stubHealth('degraded');
    const { result } = renderHook(() => useHealth());
    await vi.waitFor(() => expect(result.current).toEqual({ status: 'degraded', ok: false }));
  });

  it('reads "offline" on an error answer', async () => {
    stubHealth('ok', 503);
    const { result } = renderHook(() => useHealth());
    await vi.waitFor(() => expect(result.current).toEqual({ status: 'offline', ok: false }));
  });

  it('polls on an interval and stops when unmounted', async () => {
    vi.useFakeTimers();
    const spy = stubHealth('ok');
    const { unmount } = renderHook(() => useHealth());
    await act(() => vi.advanceTimersByTimeAsync(HEALTH_POLL_MS));
    expect(spy).toHaveBeenCalledTimes(2);
    unmount();
    await vi.advanceTimersByTimeAsync(HEALTH_POLL_MS);
    expect(spy).toHaveBeenCalledTimes(2);
  });
});

describe('Header', () => {
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });

  /** Renders the header with a settings spy. */
  function setup() {
    stubHealth('ok');
    const onOpenSettings = vi.fn();
    render(<Header apiKey="k" setApiKey={vi.fn()} onOpenSettings={onOpenSettings} />);
    return { onOpenSettings, avatar: screen.getAllByRole('button').at(-1)! };
  }

  it('the settings button opens settings', async () => {
    const { onOpenSettings } = setup();
    await userEvent.click(screen.getByTitle('Settings'));
    expect(onOpenSettings).toHaveBeenCalled();
  });

  it('the account menu names the user and logs out', async () => {
    const { avatar } = setup();
    await userEvent.click(avatar);
    expect(screen.getByText('Ada')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: /Log out/ }));
    expect(logout).toHaveBeenCalled();
    expect(screen.queryByText('Ada')).toBeNull();
  });

  it('profile settings opens the profile dialog and closes the menu', async () => {
    const { avatar } = setup();
    await userEvent.click(avatar);
    await userEvent.click(screen.getByRole('button', { name: /Profile settings/ }));
    expect(screen.getByRole('dialog').textContent).toBe('Profile');
    expect(screen.queryByText('Ada')).toBeNull();
  });

  it('a click outside closes the account menu', async () => {
    const { avatar } = setup();
    await userEvent.click(avatar);
    fireEvent.mouseDown(document.body);
    expect(screen.queryByText('Ada')).toBeNull();
  });

  it('Escape closes the download menu', () => {
    setup();
    const details = screen.getByLabelText('Download Oya Browser').closest('details')!;
    details.open = true;
    fireEvent.keyDown(details, { key: 'Escape' });
    expect(details.open).toBe(false);
  });
});
