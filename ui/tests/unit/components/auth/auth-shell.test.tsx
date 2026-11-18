/** Authentication composition puts the form before optional context without changing its controls. */
import { afterEach, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { AuthShell, AuthLoading } from '@/components/auth/auth-shell';
afterEach(cleanup);

it('places the form before product context in reading order and retains the footer', () => {
  render(
    <AuthShell glow="accent" footer={<a href="/signup">Create account</a>} pitch={<p>Product context</p>}>
      <form aria-label="Sign in">
        <input aria-label="Email" />
        <button>Sign in</button>
      </form>
    </AuthShell>,
  );
  const form = screen.getByRole('form', { name: 'Sign in' });
  expect(
    form.compareDocumentPosition(screen.getByText('Product context')) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
  expect(screen.getByRole('link', { name: 'Create account' }).getAttribute('href')).toBe('/signup');
});

it('announces session resolution within the branded shell', () => {
  render(<AuthLoading />);
  expect(screen.getByRole('status').textContent).toContain('Loading your session');
});
