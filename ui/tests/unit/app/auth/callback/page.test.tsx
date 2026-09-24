/**
 * Unit tests for the Google/GitHub sign-in landing page: it hands the
 * fragment's refresh token to the server and takes it off the URL, and it
 * shows why a sign-in failed.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import OAuthCallbackPage from '@/app/auth/callback/page';
import { refreshToken } from '@/lib/api';

vi.mock('@/lib/api', () => ({ refreshToken: vi.fn() }));

/** Puts the page at `url`, as the provider's redirect would. */
const landAt = (url: string) => window.history.replaceState(null, '', url);

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  localStorage.clear();
});

describe('OAuthCallbackPage', () => {
  it('starts the session from the fragment refresh token and clears it from the URL', async () => {
    vi.mocked(refreshToken).mockResolvedValue({ access_token: 'a', refresh_in_cookie: true });
    landAt('/auth/callback#access_token=a&refresh_token=rt&expires_in=3600');
    render(<OAuthCallbackPage />);
    await waitFor(() => expect(refreshToken).toHaveBeenCalledWith('rt', true));
    expect(window.location.hash).toBe('');
    expect(localStorage.getItem('oya_refresh_token')).toBeNull();
  });

  it('keeps a refresh token the server returned in the body, for a cross-origin console', async () => {
    vi.mocked(refreshToken).mockResolvedValue({ access_token: 'a', refresh_token: 'r2' });
    landAt('/auth/callback#refresh_token=rt');
    render(<OAuthCallbackPage />);
    await waitFor(() => expect(localStorage.getItem('oya_refresh_token')).toBe('r2'));
  });

  it("shows the provider's error", async () => {
    landAt('/auth/callback?error=access_denied&error_description=User+denied+access');
    render(<OAuthCallbackPage />);
    expect((await screen.findByRole('alert')).textContent).toBe('User denied access');
    expect(refreshToken).not.toHaveBeenCalled();
  });

  it('says the sign-in did not complete when no token came back', async () => {
    landAt('/auth/callback');
    render(<OAuthCallbackPage />);
    expect((await screen.findByRole('alert')).textContent).toBe('Sign-in did not complete');
  });

  it('shows why the server refused the token', async () => {
    vi.mocked(refreshToken).mockRejectedValue(new Error('Refresh failed'));
    landAt('/auth/callback#refresh_token=rt');
    render(<OAuthCallbackPage />);
    expect((await screen.findByRole('alert')).textContent).toBe('Refresh failed');
    expect(screen.getByRole('link', { name: 'Back to sign in' }).getAttribute('href')).toBe('/login');
  });
});
