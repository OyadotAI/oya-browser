/**
 * Unit tests for the two-factor page: entering a code for an app already set
 * up (and going back where the person came from), setting an app up, removing
 * one, a refused code, and a signed-out visitor.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import MfaPage from '@/app/account/mfa/page';
import { mfaEnroll, mfaStatus, mfaUnenroll, mfaVerify } from '@/lib/api';
import { keepStepUp, stepUpPending } from '@/lib/auth/storage';

const auth = { token: 'jwt' as string | null, loading: false };
vi.mock('@/components/auth-provider', () => ({ useAuth: () => auth }));
vi.mock('@/lib/api', () => ({ mfaStatus: vi.fn(), mfaEnroll: vi.fn(), mfaVerify: vi.fn(), mfaUnenroll: vi.fn() }));

/** A working authenticator app. */
const APP = { id: 'f1', name: 'Phone', status: 'verified', created_at: 't' };

/** Captures where the page sends the person, from a page opened with `search`. */
function locationAt(search: string) {
  const replace = vi.fn();
  vi.stubGlobal('location', { ...window.location, search, origin: window.location.origin, replace });
  return replace;
}

/** Types a code and submits it. */
function enterCode(code: string) {
  fireEvent.change(screen.getByLabelText('6-digit code'), { target: { value: code } });
  fireEvent.click(screen.getByRole('button', { name: /Verify/ }));
}

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.unstubAllGlobals();
  sessionStorage.clear();
  Object.assign(auth, { token: 'jwt', loading: false });
});

describe('MfaPage', () => {
  it('asks a session that has not passed its app for a code, then goes back where it came from', async () => {
    const replace = locationAt('?next=%2Fadmin');
    keepStepUp(true);
    vi.mocked(mfaStatus).mockResolvedValue({ factors: [APP], aal: 'aal1' });
    vi.mocked(mfaVerify).mockResolvedValue({});
    render(<MfaPage />);
    await screen.findByText('Enter your code');
    enterCode('123456');
    await vi.waitFor(() => expect(replace).toHaveBeenCalledWith('/admin'));
    expect(mfaVerify).toHaveBeenCalledWith('jwt', 'f1', '123456');
    expect(stepUpPending()).toBe(false);
  });

  it('never goes back to another site, whatever next says', async () => {
    const replace = locationAt('?next=%2F%2Fevil.example');
    vi.mocked(mfaStatus).mockResolvedValue({ factors: [APP], aal: 'aal1' });
    vi.mocked(mfaVerify).mockResolvedValue({});
    render(<MfaPage />);
    await screen.findByText('Enter your code');
    enterCode('123456');
    await vi.waitFor(() => expect(replace).toHaveBeenCalledWith('/account/mfa'));
  });

  it('shows a refused code and stays', async () => {
    const replace = locationAt('');
    vi.mocked(mfaStatus).mockResolvedValue({ factors: [APP], aal: 'aal1' });
    vi.mocked(mfaVerify).mockRejectedValue(new Error('Invalid TOTP code entered'));
    render(<MfaPage />);
    await screen.findByText('Enter your code');
    enterCode('000000');
    expect(await screen.findByRole('alert')).toHaveProperty('textContent', 'Invalid TOTP code entered');
    expect(replace).not.toHaveBeenCalled();
  });

  it('sets up an app: QR code and secret, then the first code verifies it', async () => {
    const replace = locationAt('?next=%2Fadmin');
    vi.mocked(mfaStatus).mockResolvedValue({ factors: [], aal: 'aal1' });
    vi.mocked(mfaEnroll).mockResolvedValue({
      id: 'f-new',
      qr_code: 'data:image/svg+xml;utf-8,<svg/>',
      secret: 'SECRET',
    });
    vi.mocked(mfaVerify).mockResolvedValue({});
    render(<MfaPage />);
    fireEvent.click(await screen.findByText('Set up an authenticator app'));
    expect(await screen.findByAltText('QR code for your authenticator app')).toBeTruthy();
    expect(screen.getByText('SECRET')).toBeTruthy();
    enterCode('123456');
    await vi.waitFor(() => expect(replace).toHaveBeenCalledWith('/admin'));
    expect(mfaVerify).toHaveBeenCalledWith('jwt', 'f-new', '123456');
  });

  it('lists the apps of a two-factor session and removes one', async () => {
    vi.mocked(mfaStatus).mockResolvedValue({ factors: [APP], aal: 'aal2' });
    vi.mocked(mfaUnenroll).mockResolvedValue({});
    render(<MfaPage />);
    expect(await screen.findByText('Two-factor authentication is on.')).toBeTruthy();
    fireEvent.click(screen.getByText('Remove'));
    await vi.waitFor(() => expect(mfaUnenroll).toHaveBeenCalledWith('jwt', 'f1'));
    await vi.waitFor(() => expect(mfaStatus).toHaveBeenCalledTimes(2));
  });

  it('asks a signed-out visitor to sign in', () => {
    Object.assign(auth, { token: null });
    render(<MfaPage />);
    expect(screen.getByText('Sign in')).toBeTruthy();
  });
});
