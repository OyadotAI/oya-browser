/** Buyers choose their Oya account explicitly; durable registration never displays AWS confirmation. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import MarketplacePage from '@/app/marketplace/aws/page';
import { afterSignIn } from '@/components/auth/use-auth-form';
import { fakeFetch, fetchCall } from '../../../support';

const auth = {
  loading: false,
  token: null as string | null,
  user: null as { id: string; email: string } | null,
  logout: vi.fn(),
};
const push = vi.fn();
const CONTEXT = 'opaque-setup-context-fixture';
vi.mock('@/components/auth-provider', () => ({ useAuth: () => auth }));
vi.mock('next/navigation', () => ({ useRouter: () => ({ push, replace: vi.fn() }) }));

/** Resolves the Oya session without altering any real authentication system. */
function signIn(id = 'alice') {
  auth.token = `${id}-session`;
  auth.user = { id, email: `${id}@example.test` };
}

beforeEach(() => {
  window.history.replaceState(null, '', '/marketplace/aws');
  document.cookie = 'oya_aws_pending=1; Path=/';
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.clearAllMocks();
  sessionStorage.clear();
  document.cookie = 'oya_aws_pending=; Max-Age=0; Path=/';
  auth.token = null;
  auth.user = null;
  auth.loading = false;
});

describe('AWS fulfillment page', () => {
  it('returns new and existing buyers through the normal auth flow without linking', async () => {
    const fetch = fakeFetch({ body: { status: 'ready', contextId: CONTEXT, awsAccount: '9012' } });
    render(<MarketplacePage />);
    expect(await screen.findByRole('heading', { name: 'Finish setting up Oya' })).toBeTruthy();
    expect(screen.getByRole('link', { name: 'Create account' }).getAttribute('href')).toBe('/signup');
    expect(screen.getByRole('link', { name: 'Sign in' }).getAttribute('href')).toBe('/login');
    expect(afterSignIn()).toBe('/marketplace/aws');
    expect(fetch.mock.calls).toHaveLength(1);
    expect(fetchCall(fetch)[1].method).toBeUndefined();
  });

  it('shows the selected account and requires a click before persisting a pending connection', async () => {
    signIn();
    const fetch = fakeFetch(
      { body: { status: 'ready', contextId: CONTEXT, awsAccount: '9012' } },
      { body: { status: 'pending', contextId: CONTEXT, awsAccount: '9012' } },
    );
    render(<MarketplacePage />);
    const button = await screen.findByRole('button', { name: 'Connect to this account' });
    expect(screen.getByText('alice@example.test')).toBeTruthy();
    expect(fetch.mock.calls).toHaveLength(1);
    await userEvent.click(button);
    const heading = await screen.findByRole('heading', { name: 'Waiting for AWS confirmation' });
    expect(fetchCall(fetch, 1)[0]).toBe('/api/marketplace/aws/connect');
    expect(fetchCall(fetch, 1)[1].method).toBe('POST');
    expect(fetchCall(fetch, 1)[1].body).toBe('{}');
    expect(fetchCall(fetch, 1)[1].headers).toMatchObject({
      Authorization: 'Bearer alice-session',
      'Idempotency-Key': expect.any(String),
      'X-Oya-Marketplace-Context': CONTEXT,
    });
    expect(document.activeElement).toBe(heading);
    expect(screen.queryByText('Subscription confirmed')).toBeNull();
    expect(screen.queryByText('Open Oya')).toBeNull();
    expect(afterSignIn()).toBe('/dashboard');
    await userEvent.click(screen.getByRole('button', { name: 'Check again' }));
    await waitFor(() => expect(fetch.mock.calls).toHaveLength(3));
    expect(fetchCall(fetch, 2)[1].method).toBeUndefined();
  });

  it('switches accounts through existing logout and client navigation, preserving setup', async () => {
    signIn();
    fakeFetch({ body: { status: 'ready', contextId: CONTEXT, awsAccount: '9012' } });
    render(<MarketplacePage />);
    await userEvent.click(await screen.findByRole('button', { name: 'Use another account' }));
    expect(auth.logout).toHaveBeenCalledOnce();
    expect(push).toHaveBeenCalledWith('/login');
    expect(afterSignIn()).toBe('/marketplace/aws');
  });

  it('shows a safe conflict without disclosing another owner', async () => {
    signIn();
    fakeFetch({ status: 409, body: { code: 'purchase_conflict', error: 'private-other-owner@example.test' } });
    render(<MarketplacePage />);
    expect(await screen.findByRole('heading', { name: 'This purchase is already connected' })).toBeTruthy();
    expect(screen.queryByText('private-other-owner@example.test')).toBeNull();
    expect(screen.queryByRole('button', { name: 'Connect to this account' })).toBeNull();
  });

  it('clears expired continuation navigation and never asks the buyer to buy again', async () => {
    fakeFetch({ status: 410, body: { code: 'setup_expired' } });
    render(<MarketplacePage />);
    expect(await screen.findByRole('heading', { name: 'Continue from AWS Marketplace' })).toBeTruthy();
    expect(afterSignIn()).toBe('/dashboard');
    expect(screen.getByRole('link', { name: 'Open AWS Marketplace' }).getAttribute('href')).toBe(
      'https://aws.amazon.com/marketplace',
    );
  });

  it('discards a previous account response after an account change', async () => {
    signIn();
    let old: (response: Response) => void = () => {};
    const fetch = vi
      .fn()
      .mockImplementationOnce(
        () =>
          new Promise<Response>((done) => {
            old = done;
          }),
      )
      .mockResolvedValue(new Response(JSON.stringify({ status: 'ready', contextId: CONTEXT, awsAccount: '3456' })));
    vi.stubGlobal('fetch', fetch);
    const view = render(<MarketplacePage />);
    await waitFor(() => expect(fetch).toHaveBeenCalledOnce());
    signIn('bob');
    view.rerender(<MarketplacePage />);
    expect(await screen.findByText('bob@example.test')).toBeTruthy();
    await act(async () =>
      old(new Response(JSON.stringify({ status: 'pending', contextId: CONTEXT, awsAccount: '9012' }))),
    );
    expect(screen.queryByText('alice@example.test')).toBeNull();
    expect(screen.queryByRole('heading', { name: 'Waiting for AWS confirmation' })).toBeNull();
    expect(screen.getByText('•••• 3456')).toBeTruthy();
  });

  it('recovers an uncertain network result by reading status before any new mutation', async () => {
    signIn();
    const fetch = fakeFetch({ body: { status: 'pending', contextId: CONTEXT, awsAccount: '9012' } });
    fetch.mockRejectedValueOnce(new TypeError('offline'));
    render(<MarketplacePage />);
    await userEvent.click(await screen.findByRole('button', { name: 'Try again' }));
    expect(await screen.findByRole('heading', { name: 'Waiting for AWS confirmation' })).toBeTruthy();
    expect(fetch.mock.calls.every((_, index) => !fetchCall(fetch, index)[1].method)).toBe(true);
  });

  it('refuses an unexpected confirmed response until real activation is implemented', async () => {
    signIn();
    fakeFetch({ body: { status: 'confirmed', contextId: CONTEXT, awsAccount: '9012' } });
    render(<MarketplacePage />);
    expect(await screen.findByRole('heading', { name: 'Setup is temporarily unavailable' })).toBeTruthy();
    expect(screen.queryByText('Subscription confirmed')).toBeNull();
  });

  it('requires a fresh purchase review and click when another tab replaces the setup cookie', async () => {
    signIn();
    const nextContext = 'replacement-context-fixture';
    const fetch = fakeFetch(
      { body: { status: 'ready', contextId: CONTEXT, awsAccount: '9012' } },
      { status: 409, body: { code: 'setup_changed' } },
      { body: { status: 'ready', contextId: nextContext, awsAccount: '3456' } },
      { body: { status: 'pending', contextId: nextContext, awsAccount: '3456' } },
    );
    render(<MarketplacePage />);
    await userEvent.click(await screen.findByRole('button', { name: 'Connect to this account' }));
    expect(await screen.findByRole('heading', { name: 'Review your AWS purchase' })).toBeTruthy();
    expect(fetch.mock.calls).toHaveLength(2);
    await userEvent.click(screen.getByRole('button', { name: 'Review purchase' }));
    expect(await screen.findByText('•••• 3456')).toBeTruthy();
    expect(fetchCall(fetch, 2)[1].method).toBeUndefined();
    await userEvent.click(screen.getByRole('button', { name: 'Connect to this account' }));
    expect(await screen.findByRole('heading', { name: 'Waiting for AWS confirmation' })).toBeTruthy();
    expect(fetchCall(fetch, 3)[1].headers).toMatchObject({ 'X-Oya-Marketplace-Context': nextContext });
  });
});
