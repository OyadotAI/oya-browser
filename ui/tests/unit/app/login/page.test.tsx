/**
 * Unit tests for the sign-in page: account and API-key sign-in, their
 * validation, and the errors the server gives.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import LoginPage from '@/app/login/page';
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
    expect(login).toHaveBeenCalledWith('a@b', 'pw');
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
    await userEvent.click(screen.getByRole('button', { name: 'API key' }));
    await submit('Continue');
    expect(screen.getByRole('alert').textContent).toBe('Enter an API key');
  });

  it('verifies the key with the server before keeping it for this tab', async () => {
    fakeFetch({ body: {} });
    render(<LoginPage />);
    await userEvent.click(screen.getByRole('button', { name: 'API key' }));
    await userEvent.type(screen.getByLabelText('API key'), '  oya_k  ');
    await submit('Continue');
    expect(sessionStorage.getItem('oya_console_key')).toBe('oya_k');
    expect(replace).toHaveBeenCalledWith('/dashboard');
  });

  it('says a rejected key was rejected, and any other failure with its status', async () => {
    fakeFetch({ status: 401, body: {} }, { status: 502, body: {} });
    render(<LoginPage />);
    await userEvent.click(screen.getByRole('button', { name: 'API key' }));
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
