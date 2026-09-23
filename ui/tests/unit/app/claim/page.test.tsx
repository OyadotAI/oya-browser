/**
 * Unit tests for claiming an agent's key: the key leaves the URL at once,
 * waits through a sign-in, and is imported for whoever is signed in; a
 * refused or missing key says so.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import ClaimPage from '@/app/claim/page';
import { importApiKey } from '@/lib/api';
import { afterSignIn } from '@/components/auth/use-auth-form';

const auth = { token: null as string | null, loading: false };
vi.mock('@/components/auth-provider', () => ({ useAuth: () => auth }));
vi.mock('next/navigation', () => ({ useRouter: () => ({ replace: vi.fn() }) }));
vi.mock('@/lib/api', () => ({ importApiKey: vi.fn() }));

/** The agent key the link carries. */
const KEY = 'k'.repeat(32);

/** Opens the claim link. */
const openLink = (hash = KEY) => window.history.replaceState(null, '', `/claim${hash ? `#${hash}` : ''}`);

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  sessionStorage.clear();
  auth.token = null;
});

describe('ClaimPage', () => {
  it('asks a signed-out person to sign in, keeping the key off the URL until they do', async () => {
    openLink();
    render(<ClaimPage />);
    expect(await screen.findByText('Claim your agent’s key')).toBeTruthy();
    expect(window.location.hash).toBe('');
    expect(afterSignIn()).toBe('/claim');
    expect(importApiKey).not.toHaveBeenCalled();
  });

  it('claims the key for the signed-in person, then sign-ins go home again', async () => {
    vi.mocked(importApiKey).mockResolvedValue({ ok: true });
    auth.token = 'jwt';
    openLink();
    render(<ClaimPage />);
    expect(await screen.findByText('Key claimed')).toBeTruthy();
    expect(importApiKey).toHaveBeenCalledWith('jwt', KEY, 'Agent');
    expect(screen.getByRole('link', { name: 'a star on GitHub' }).getAttribute('href')).toBe(
      'https://github.com/OyadotAI/oya-browser',
    );
    expect(afterSignIn()).toBe('/dashboard');
  });

  it('claims a key remembered from before the sign-in', async () => {
    vi.mocked(importApiKey).mockResolvedValue({ ok: true });
    sessionStorage.setItem('oya_pending_claim', KEY);
    auth.token = 'jwt';
    openLink('');
    render(<ClaimPage />);
    expect(await screen.findByText('Key claimed')).toBeTruthy();
    expect(importApiKey).toHaveBeenCalledWith('jwt', KEY, 'Agent');
  });

  it("shows the server's reason when the key cannot be claimed", async () => {
    vi.mocked(importApiKey).mockRejectedValue(new Error('Key cannot be imported'));
    auth.token = 'jwt';
    openLink();
    render(<ClaimPage />);
    expect((await screen.findByRole('alert')).textContent).toBe('Key cannot be imported');
  });

  it('says so when the link carries no key', async () => {
    openLink('');
    render(<ClaimPage />);
    expect(await screen.findByText('No key in this link')).toBeTruthy();
  });
});
