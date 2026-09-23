/**
 * Unit tests for the sign-in page: account and API-key sign-in, their
 * validation, and the errors the server gives.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import LoginPage from '@/app/login/page';
import { LoginView } from '@/app/login/login-view';
import { act } from '@testing-library/react';
import { fakeFetch } from '../../support';

const replace = vi.fn();
const login = vi.fn();
const auth = { user: null as null | object, loading: false, login };
vi.mock('next/navigation', () => ({ useRouter: () => ({ replace }) }));
vi.mock('@/components/auth-provider', () => ({ useAuth: () => auth }));

beforeEach(() => {
  auth.user = null;
  auth.loading = false;
});
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.unstubAllGlobals();
  sessionStorage.clear();
});

/** Clicks the submit button labeled `name`. */
const submit = (name: string) => userEvent.click(screen.getByRole('button', { name }));

describe('LoginPage with an account', () => {
  it('asks for the email, then the password', async () => {
    render(<LoginPage />);
    await submit('Sign in');
    expect(screen.getByRole('alert').textContent).toBe('Email is required');
    await userEvent.type(screen.getByLabelText('Email'), 'a@b');
    await submit('Sign in');
    expect(screen.getByRole('alert').textContent).toBe('Password is required');
    expect(login).not.toHaveBeenCalled();
  });

  it('signs in and goes to the console', async () => {
    login.mockResolvedValue(undefined);
    render(<LoginPage />);
    await userEvent.type(screen.getByLabelText('Email'), 'a@b');
    await userEvent.type(screen.getByLabelText('Password'), 'pw');
    await submit('Sign in');
    expect(login).toHaveBeenCalledWith('a@b', 'pw', undefined);
    expect(replace).toHaveBeenCalledWith('/dashboard');
  });

  it("shows the server's error, and typing clears it", async () => {
    login.mockRejectedValue(new Error('Wrong password'));
    render(<LoginPage />);
    await userEvent.type(screen.getByLabelText('Email'), 'a@b');
    await userEvent.type(screen.getByLabelText('Password'), 'pw');
    await submit('Sign in');
    expect(screen.getByRole('alert').textContent).toBe('Wrong password');
    await userEvent.type(screen.getByLabelText('Password'), 'x');
    expect(screen.queryByRole('alert')).toBeNull();
  });

  it('shows and hides the password', async () => {
    render(<LoginPage />);
    await userEvent.click(screen.getByRole('button', { name: 'Show password' }));
    expect(screen.getByLabelText('Password').getAttribute('type')).toBe('text');
  });
});

describe('LoginPage with an API key', () => {
  it('needs a key', async () => {
    render(<LoginPage />);
    await userEvent.click(screen.getByRole('button', { name: 'Machine' }));
    await submit('Continue');
    expect(screen.getByRole('alert').textContent).toBe('Enter an API key');
  });

  it('verifies the key with the server before keeping it for this tab', async () => {
    fakeFetch({ body: {} });
    render(<LoginPage />);
    await userEvent.click(screen.getByRole('button', { name: 'Machine' }));
    await userEvent.type(screen.getByLabelText('API key'), '  oya_k  ');
    await submit('Continue');
    expect(sessionStorage.getItem('oya_console_key')).toBe('oya_k');
    expect(replace).toHaveBeenCalledWith('/dashboard');
  });

  it('says a rejected key was rejected, and any other failure with its status', async () => {
    fakeFetch({ status: 401, body: {} }, { status: 502, body: {} });
    render(<LoginPage />);
    await userEvent.click(screen.getByRole('button', { name: 'Machine' }));
    await userEvent.type(screen.getByLabelText('API key'), 'bad');
    await submit('Continue');
    expect(screen.getByRole('alert').textContent).toBe('That key was rejected');
    await submit('Continue');
    expect(screen.getByRole('alert').textContent).toBe('Could not verify the key (502)');
    expect(sessionStorage.getItem('oya_console_key')).toBeNull();
  });
});

describe('LoginPage when signed in', () => {
  it('shows only a spinner and moves on to the console', () => {
    auth.user = { id: 'u' };
    render(<LoginPage />);
    expect(screen.queryByRole('heading')).toBeNull();
    expect(replace).toHaveBeenCalledWith('/dashboard');
  });
});

describe('LoginPage with the captcha on', () => {
  /** Stands in for Cloudflare's script; `pass` hands the widget a token. */
  function fakeTurnstile() {
    let callback: (token: string) => void = () => {};
    const api = {
      render: vi.fn(
        (_el: HTMLElement, options: { sitekey: string; callback: (token: string) => void }) => (
          (callback = options.callback),
          'w1'
        ),
      ),
      reset: vi.fn(),
      remove: vi.fn(),
    };
    vi.stubGlobal('turnstile', api);
    return { api, pass: (token: string) => act(() => callback(token)) };
  }

  /** Fills the account form. */
  async function fillAccount() {
    await userEvent.type(screen.getByLabelText('Email'), 'a@b');
    await userEvent.type(screen.getByLabelText('Password'), 'pw');
  }

  it('offers Google and GitHub', () => {
    render(<LoginPage />);
    expect(screen.getByRole('button', { name: /Continue with Google/ })).toBeTruthy();
    expect(screen.getByRole('button', { name: /Continue with GitHub/ })).toBeTruthy();
  });

  it('refuses to sign in until the captcha is solved', async () => {
    fakeTurnstile();
    render(<LoginView siteKey="site-key" />);
    await fillAccount();
    await submit('Sign in');
    expect(screen.getByRole('alert').textContent).toBe('Please complete the captcha check');
    expect(login).not.toHaveBeenCalled();
  });

  it('sends the captcha token with the sign-in', async () => {
    login.mockResolvedValue(undefined);
    const { api, pass } = fakeTurnstile();
    render(<LoginView siteKey="site-key" />);
    expect(api.render.mock.calls[0][1].sitekey).toBe('site-key');
    await pass('tok');
    await fillAccount();
    await submit('Sign in');
    expect(login).toHaveBeenCalledWith('a@b', 'pw', 'tok');
  });

  it('resets the spent captcha after a failed sign-in', async () => {
    login.mockRejectedValue(new Error('Invalid login credentials'));
    const { api, pass } = fakeTurnstile();
    render(<LoginView siteKey="site-key" />);
    await pass('tok');
    await fillAccount();
    await submit('Sign in');
    expect(api.reset).toHaveBeenCalledWith('w1');
    await submit('Sign in');
    expect(screen.getByRole('alert').textContent).toBe('Please complete the captcha check');
  });
});

describe('LoginPage for an agent', () => {
  it('tells the agent to sign itself up from this deployment, with no key and no form', async () => {
    render(<LoginPage />);
    await userEvent.click(screen.getByRole('button', { name: 'Agent' }));
    expect(screen.getByText(new RegExp(`read ${window.location.origin}/llms.txt`))).toBeTruthy();
    expect(screen.queryByText(/API key/)).toBeNull();
    expect(screen.queryByRole('button', { name: 'Sign in' })).toBeNull();
    expect(screen.getByRole('link', { name: 'What agents read' }).getAttribute('href')).toBe('/llms.txt');
  });
});
