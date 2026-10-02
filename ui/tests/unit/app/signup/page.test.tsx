/**
 * Unit tests for the sign-up page: validation, the length hint, and account
 * creation.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import SignupPage from '@/app/signup/page';
import { SignupView } from '@/app/signup/signup-view';
import { act } from '@testing-library/react';

const replace = vi.fn();
const signup = vi.fn();
let search = new URLSearchParams();
vi.mock('next/navigation', () => ({ useRouter: () => ({ replace }), useSearchParams: () => search }));
vi.mock('@/components/auth-provider', () => ({ useAuth: () => ({ user: null, loading: false, signup }) }));

afterEach(() => {
  search = new URLSearchParams();
  cleanup();
  vi.clearAllMocks();
  vi.unstubAllGlobals();
});

describe('SignupPage', () => {
  it('counts down the characters a short password still needs', async () => {
    render(<SignupPage />);
    await userEvent.type(screen.getByLabelText('Password'), 'abcdef');
    expect(screen.getByText('2 more characters needed')).toBeTruthy();
    await userEvent.type(screen.getByLabelText('Password'), 'g');
    expect(screen.getByText('1 more character needed')).toBeTruthy();
    await userEvent.type(screen.getByLabelText('Password'), 'h');
    expect(screen.queryByText(/more character/)).toBeNull();
  });

  it('refuses a short password', async () => {
    render(<SignupPage />);
    await userEvent.type(screen.getByLabelText('Email'), 'a@b');
    await userEvent.type(screen.getByLabelText('Password'), 'short');
    await userEvent.click(screen.getByRole('button', { name: 'Create account' }));
    expect(screen.getByRole('alert').textContent).toBe('Password must be at least 8 characters');
    expect(signup).not.toHaveBeenCalled();
  });

  it('creates the account with a trimmed name, or none when blank', async () => {
    signup.mockResolvedValue(undefined);
    render(<SignupPage />);
    await userEvent.type(screen.getByLabelText(/Display name/), '  Ann ');
    await userEvent.type(screen.getByLabelText('Email'), 'a@b');
    await userEvent.type(screen.getByLabelText('Password'), 'longenough');
    await userEvent.click(screen.getByRole('button', { name: 'Create account' }));
    expect(signup).toHaveBeenCalledWith('a@b', 'longenough', 'Ann', undefined);
    expect(replace).toHaveBeenCalledWith('/dashboard');
  });

  it('falls back to a generic message for a non-Error failure', async () => {
    signup.mockRejectedValue('nope');
    render(<SignupPage />);
    await userEvent.type(screen.getByLabelText('Email'), 'a@b');
    await userEvent.type(screen.getByLabelText('Password'), 'longenough');
    await userEvent.click(screen.getByRole('button', { name: 'Create account' }));
    expect(screen.getByRole('alert').textContent).toBe('Something went wrong. Please try again.');
    expect(signup).toHaveBeenCalledWith('a@b', 'longenough', undefined, undefined);
  });
});

describe('SignupPage with the captcha on', () => {
  it('sends the solved captcha with the new account, and removes the widget when it goes', async () => {
    signup.mockResolvedValue(undefined);
    let solve: (token: string) => void = () => {};
    const api = {
      render: vi.fn((_el: HTMLElement, o: { callback: (token: string) => void }) => ((solve = o.callback), 'w1')),
      reset: vi.fn(),
      remove: vi.fn(),
    };
    vi.stubGlobal('turnstile', api);
    const { unmount } = render(<SignupView siteKey="site-key" />);
    await userEvent.type(screen.getByLabelText('Email'), 'a@b');
    await userEvent.type(screen.getByLabelText('Password'), 'longenough');
    await userEvent.click(screen.getByRole('button', { name: 'Create account' }));
    expect(screen.getByRole('alert').textContent).toBe('Please complete the captcha check');
    act(() => solve('tok'));
    await userEvent.click(screen.getByRole('button', { name: 'Create account' }));
    expect(signup).toHaveBeenCalledWith('a@b', 'longenough', undefined, 'tok');
    unmount();
    expect(api.remove).toHaveBeenCalledWith('w1');
  });

  it('resets the spent captcha after a failed signup', async () => {
    signup.mockRejectedValue(new Error('User already registered'));
    let solve: (token: string) => void = () => {};
    const api = {
      render: vi.fn((_el: HTMLElement, o: { callback: (token: string) => void }) => ((solve = o.callback), 'w1')),
      reset: vi.fn(),
      remove: vi.fn(),
    };
    vi.stubGlobal('turnstile', api);
    render(<SignupView siteKey="site-key" />);
    act(() => solve('tok'));
    await userEvent.type(screen.getByLabelText('Email'), 'a@b');
    await userEvent.type(screen.getByLabelText('Password'), 'longenough');
    await userEvent.click(screen.getByRole('button', { name: 'Create account' }));
    expect(screen.getByRole('alert').textContent).toBe('User already registered');
    expect(api.reset).toHaveBeenCalledWith('w1');
  });
});

describe('SignupPage enterprise path', () => {
  it('opens on the sales questions from ?plan=enterprise, with links back to the account form and sign-in', () => {
    search = new URLSearchParams('plan=enterprise');
    render(<SignupPage />);
    expect(screen.getByRole('heading', { name: 'Talk to sales' })).toBeTruthy();
    expect(screen.getByRole('link', { name: 'Enterprise' }).getAttribute('aria-current')).toBe('page');
    expect(screen.getByRole('link', { name: 'Developer' }).getAttribute('href')).toBe('/signup');
    expect(screen.getByRole('link', { name: 'Sign in' }).getAttribute('href')).toBe('/login');
  });

  it('points the account form at the sales questions, from the switch and the nudge', () => {
    render(<SignupPage />);
    expect(screen.getByRole('link', { name: 'Enterprise' }).getAttribute('href')).toBe('/signup?plan=enterprise');
    expect(screen.getByRole('link', { name: /Talk to sales/ }).getAttribute('href')).toBe('/signup?plan=enterprise');
  });

  it('holds the calendar back until the required answers are in', async () => {
    search = new URLSearchParams('plan=enterprise');
    render(<SignupPage />);
    await userEvent.click(screen.getByRole('button', { name: 'Continue to pick a time' }));
    expect(screen.getByRole('alert').textContent).toBe('First name is required');
    expect(screen.queryByTitle(/Pick a time/)).toBeNull();
  });

  it('shows the prefilled calendar, then confirms once Calendly reports the booking', async () => {
    search = new URLSearchParams('plan=enterprise');
    render(<SignupPage />);
    await userEvent.type(screen.getByLabelText('First name'), 'Ann');
    await userEvent.type(screen.getByLabelText('Work email'), 'ann@acme.test');
    await userEvent.type(screen.getByLabelText('Company'), 'Acme');
    await userEvent.click(screen.getByRole('button', { name: 'EHRs' }));
    await userEvent.click(screen.getByRole('button', { name: 'Continue to pick a time' }));
    const src = new URL(screen.getByTitle(/Pick a time/).getAttribute('src') ?? '');
    expect(src.searchParams.get('a1')).toContain('Needs to reach: EHRs');
    const data = { event: 'calendly.event_scheduled' };
    act(() => void window.dispatchEvent(new MessageEvent('message', { origin: 'https://calendly.com', data })));
    expect(screen.getByRole('heading', { name: "You're booked." })).toBeTruthy();
    expect(screen.queryByRole('button', { name: /Edit your details/ })).toBeNull();
  });
});
