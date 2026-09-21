/**
 * Unit tests for the account dialog: only the display name is editable, it
 * saves when changed, and a failure is shown.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

const auth = { user: {}, token: 't', logout: vi.fn(), applyProfile: vi.fn() };
vi.mock('@/components/auth-provider', () => ({ useAuth: () => auth }));
vi.mock('@/lib/api', () => ({ updateProfile: vi.fn() }));

import { updateProfile } from '@/lib/api';
import ProfileDialog from '@/components/dashboard/profile-dialog';

const mockUpdate = vi.mocked(updateProfile);

/** Renders the open dialog. */
function setup() {
  const onClose = vi.fn();
  render(<ProfileDialog open onClose={onClose} />);
  return { onClose, input: screen.getByPlaceholderText('Your name') as HTMLInputElement };
}

describe('ProfileDialog', () => {
  beforeEach(() => {
    auth.user = { email: 'a@b.c', role: 'admin', display_name: 'Ann', created_at: '2026-01-15T12:00:00Z' };
    auth.logout = vi.fn();
    auth.applyProfile = vi.fn();
  });
  afterEach(cleanup);

  it('shows the stored name and the read-only details', () => {
    const { input } = setup();
    expect(input.value).toBe('Ann');
    expect(screen.getByText('a@b.c')).toBeTruthy();
    expect(screen.getByText('admin')).toBeTruthy();
    expect(screen.getByText('Member since')).toBeTruthy();
  });

  it('keeps Save off until the name changes', async () => {
    const { input } = setup();
    const save = screen.getByRole('button', { name: 'Save' }) as HTMLButtonElement;
    expect(save.disabled).toBe(true);
    await userEvent.type(input, 'a');
    expect(save.disabled).toBe(false);
  });

  it('saves the trimmed name on Enter and applies the answer', async () => {
    mockUpdate.mockResolvedValueOnce({ display_name: 'Anna' });
    const { input } = setup();
    await userEvent.clear(input);
    await userEvent.type(input, ' Anna {Enter}');
    expect(await screen.findByText('Saved.')).toBeTruthy();
    expect(mockUpdate).toHaveBeenCalledWith('t', 'Anna');
    expect(auth.applyProfile).toHaveBeenCalledWith({ display_name: 'Anna' });
  });

  it('shows why saving failed', async () => {
    mockUpdate.mockRejectedValueOnce(new Error('Too long'));
    const { input } = setup();
    await userEvent.type(input, 'x');
    await userEvent.click(screen.getByRole('button', { name: 'Save' }));
    expect(await screen.findByText('Too long')).toBeTruthy();
    expect(screen.queryByText('Saved.')).toBeNull();
  });

  it('logs out and closes', async () => {
    const { onClose } = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Log out' }));
    expect(auth.logout).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });
});
