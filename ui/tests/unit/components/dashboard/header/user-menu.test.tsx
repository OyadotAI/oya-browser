/**
 * Unit tests for the account menu: Oya staff get a link to the admin page,
 * everyone else does not.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import UserMenu from '@/components/dashboard/header/user-menu';

const auth = { user: { email: 'mk@getoya.ai' } as { email: string }, logout: vi.fn() };
vi.mock('@/components/auth-provider', () => ({ useAuth: () => auth }));

/** Opens the menu. */
function open() {
  render(<UserMenu onOpenProfile={() => {}} />);
  fireEvent.click(screen.getByRole('button'));
}

afterEach(() => {
  cleanup();
  auth.user = { email: 'mk@getoya.ai' };
});

describe('UserMenu', () => {
  it('links Oya staff to the admin page, whatever the address’s case', () => {
    auth.user = { email: 'MK@GetOya.ai' };
    open();
    expect(screen.getByText('Admin').closest('a')?.getAttribute('href')).toBe('/admin');
  });

  it('shows no admin link to anyone else', () => {
    auth.user = { email: 'ana@example.com' };
    open();
    expect(screen.queryByText('Admin')).toBeNull();
    expect(screen.getByText('Profile settings')).toBeTruthy();
  });

  it('links a signed-in person to their plan and billing', () => {
    auth.user = { email: 'ana@example.com' };
    open();
    expect(screen.getByText('Plan & billing').closest('a')?.getAttribute('href')).toBe('/dashboard/billing');
  });
});
