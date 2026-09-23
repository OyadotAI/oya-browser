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
vi.mock('next/navigation', () => ({ useRouter: () => ({ replace }) }));
vi.mock('@/components/auth-provider', () => ({ useAuth: () => ({ user: null, loading: false, signup }) }));

afterEach(() => {
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
