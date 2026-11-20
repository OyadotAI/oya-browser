/**
 * Unit tests for claiming an agent's key: the key leaves the URL at once,
 * waits through a sign-in, and nothing is sent until the signed-in person
 * confirms; they can discard it instead, and a refused or missing key says so.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import ClaimPage from '@/app/claim/page';
import { AccountError, claimAgentKey, importApiKey } from '@/lib/api';
import { afterSignIn } from '@/components/auth/use-auth-form';

const auth = { token: null as string | null, loading: false };
vi.mock('@/components/auth-provider', () => ({ useAuth: () => auth }));
vi.mock('next/navigation', () => ({ useRouter: () => ({ replace: vi.fn() }) }));
vi.mock('@/lib/api', async (importOriginal) => ({
  ...(await importOriginal<typeof import('@/lib/api')>()),
  claimAgentKey: vi.fn(),
  importApiKey: vi.fn(),
}));

/** The agent key the link carries. */
const KEY = 'k'.repeat(32);
/** A refusal's status. */
const FORBIDDEN = 403;
/** A server failure's status. */
const INTERNAL = 500;
/** A lapsed session's status. */
const UNAUTHORIZED = 401;
/** A rate limit's status. */
const TOO_MANY = 429;

/** Opens the claim link. */
const openLink = (hash = KEY) => window.history.replaceState(null, '', `/claim${hash ? `#${hash}` : ''}`);

/** Opens the link signed in and waits for the question. */
async function openSignedIn() {
  auth.token = 'jwt';
  openLink();
  const view = render(<ClaimPage />);
  await screen.findByText('Claim this agent key?');
  return view;
}

/** Answers the question with a claim. */
const confirm = () => fireEvent.click(screen.getByRole('button', { name: 'Claim this key' }));

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
    expect(screen.getByText(/You will be asked to confirm/)).toBeTruthy();
    expect(window.location.hash).toBe('');
    expect(afterSignIn()).toBe('/claim');
    expect(claimAgentKey).not.toHaveBeenCalled();
  });

  it('sends nothing for a signed-in person until they confirm, and shows only the start of the key', async () => {
    await openSignedIn();
    await new Promise((r) => setTimeout(r, 0));
    expect(claimAgentKey).not.toHaveBeenCalled();
    expect(window.location.hash).toBe('');
    expect(screen.getByText(`${KEY.slice(0, 8)}…`)).toBeTruthy();
    expect(document.body.textContent).not.toContain(KEY);
  });

  it('claims the key once the person confirms, then sign-ins go home again', async () => {
    vi.mocked(claimAgentKey).mockResolvedValue({ ok: true });
    await openSignedIn();
    confirm();
    expect(await screen.findByText('Key claimed')).toBeTruthy();
    expect(claimAgentKey).toHaveBeenCalledWith('jwt', KEY);
    expect(screen.getByRole('link', { name: 'a star on GitHub' }).getAttribute('href')).toBe(
      'https://github.com/OyadotAI/oya-browser',
    );
    expect(afterSignIn()).toBe('/dashboard');
  });

  it('never uses the open import, which takes any key', async () => {
    vi.mocked(claimAgentKey).mockResolvedValue({ ok: true });
    await openSignedIn();
    confirm();
    await screen.findByText('Key claimed');
    expect(importApiKey).not.toHaveBeenCalled();
  });

  it('asks before claiming a key remembered from before the sign-in', async () => {
    sessionStorage.setItem('oya_pending_claim', KEY);
    auth.token = 'jwt';
    openLink('');
    render(<ClaimPage />);
    expect(await screen.findByText('Claim this agent key?')).toBeTruthy();
    expect(claimAgentKey).not.toHaveBeenCalled();
  });

  it('forgets the key and claims nothing when the person discards the link', async () => {
    await openSignedIn();
    fireEvent.click(screen.getByRole('button', { name: 'Discard this link' }));
    expect(await screen.findByText('Link discarded')).toBeTruthy();
    expect(sessionStorage.getItem('oya_pending_claim')).toBeNull();
    expect(claimAgentKey).not.toHaveBeenCalled();
  });

  it("shows the server's reason and forgets the key when the claim is refused", async () => {
    vi.mocked(claimAgentKey).mockRejectedValue(new AccountError('This key cannot be claimed.', undefined, FORBIDDEN));
    await openSignedIn();
    confirm();
    expect((await screen.findByRole('alert')).textContent).toBe('This key cannot be claimed.');
    expect(screen.getByText('Could not claim this key')).toBeTruthy();
    expect(sessionStorage.getItem('oya_pending_claim')).toBeNull();
  });

  for (const [what, failure] of [
    ['the server cannot be reached', new TypeError('Failed to fetch')],
    ['the server fails', new AccountError('boom', undefined, INTERNAL)],
    ['the session has lapsed', new AccountError('Invalid token', undefined, UNAUTHORIZED)],
    ['the request is rate limited', new AccountError('Slow down', undefined, TOO_MANY)],
  ] as const) {
    it(`keeps the key and offers another try when ${what}`, async () => {
      vi.mocked(claimAgentKey).mockRejectedValueOnce(failure).mockResolvedValue({ ok: true });
      await openSignedIn();
      confirm();
      expect((await screen.findByRole('alert')).textContent).toMatch(/^Could not reach Oya\./);
      expect(sessionStorage.getItem('oya_pending_claim')).toBe(KEY);
      confirm();
      expect(await screen.findByText('Key claimed')).toBeTruthy();
    });
  }

  it('preserves the pending claim across a second-factor step-up', async () => {
    sessionStorage.setItem('oya_mfa_pending', '1');
    auth.token = 'jwt';
    openLink();
    render(<ClaimPage />);
    await new Promise((r) => setTimeout(r, 0));
    expect(window.location.hash).toBe('');
    expect(sessionStorage.getItem('oya_pending_claim')).toBe(KEY);
    expect(claimAgentKey).not.toHaveBeenCalled();
  });

  it('preserves the pending claim when the claim is refused with mfa_required', async () => {
    vi.mocked(claimAgentKey).mockRejectedValue(new AccountError('Enter the code', 'mfa_required', 401));
    await openSignedIn();
    confirm();
    await vi.waitFor(() => expect(sessionStorage.getItem('oya_mfa_pending')).toBe('1'));
    expect(sessionStorage.getItem('oya_pending_claim')).toBe(KEY);
  });

  it('keeps the settled claim screen when the session renews its access token', async () => {
    vi.mocked(claimAgentKey).mockResolvedValue({ ok: true });
    const view = await openSignedIn();
    confirm();
    expect(await screen.findByText('Key claimed')).toBeTruthy();
    auth.token = 'jwt-renewed';
    view.rerender(<ClaimPage />);
    await new Promise((r) => setTimeout(r, 0));
    expect(screen.getByText('Key claimed')).toBeTruthy();
    expect(claimAgentKey).toHaveBeenCalledTimes(1);
  });

  it('says so when the link carries no key', async () => {
    openLink('');
    render(<ClaimPage />);
    expect(await screen.findByText('No key in this link')).toBeTruthy();
  });
});
