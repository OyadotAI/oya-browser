/**
 * Unit tests for the "Login as" bar: shown only while impersonating, naming
 * the customer, and Exit ends it and goes back to the admin page.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { ImpersonationBanner } from '@/components/impersonation-banner';

afterEach(() => {
  cleanup();
  sessionStorage.clear();
  vi.unstubAllGlobals();
});

describe('ImpersonationBanner', () => {
  it('shows nothing in the admin’s own session', () => {
    const { container } = render(<ImpersonationBanner />);
    expect(container.innerHTML).toBe('');
  });

  it('names the customer, and Exit ends the Login as and returns to admin', () => {
    const replace = vi.fn();
    vi.stubGlobal('location', { ...window.location, replace });
    sessionStorage.setItem('oya_impersonation', JSON.stringify({ token: 't', email: 'c@example.com' }));
    render(<ImpersonationBanner />);
    expect(screen.getByRole('alert').textContent).toContain('c@example.com');
    fireEvent.click(screen.getByText('Exit'));
    expect(sessionStorage.getItem('oya_impersonation')).toBeNull();
    expect(replace).toHaveBeenCalledWith('/admin');
  });
});
