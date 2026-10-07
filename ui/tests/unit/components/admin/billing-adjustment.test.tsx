/** Support controls submit explicit actions, retain retry ids, and surface failures. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { BillingAdjustment } from '@/components/admin/billing-adjustment';
import type { Found } from '@/components/admin/types';
import type { AdminState } from '@/components/admin/use-admin';

afterEach(cleanup);

/** One selected profile and mock admin actions. */
function fixture() {
  const p: Found = {
    profile: { id: 'u', email: 'u@example.com' },
    standing: { plan: 'free', status: null, since: '2026-10-01' },
    used: {},
    subscription: null,
    keys: [],
  };
  const setPlan = vi.fn().mockResolvedValue({});
  const grant = vi.fn().mockResolvedValue({});
  const changed = vi.fn();
  render(<BillingAdjustment p={p} s={{ setPlan, grant } as unknown as AdminState} changed={changed} />);
  return { setPlan, grant, changed };
}

/** Types into an accessible labeled control. */
const fill = (label: string, value: string) => fireEvent.change(screen.getByLabelText(label), { target: { value } });

describe('admin billing adjustment', () => {
  it('requires a reason and sends the selected plan explicitly', async () => {
    const { setPlan, grant, changed } = fixture();
    expect((screen.getByText('Save plan access') as HTMLButtonElement).disabled).toBe(true);
    fill('Reason for adjustment', 'Trial');
    fill('Access plan', 'developer');
    fireEvent.click(screen.getByText('Save plan access'));
    await waitFor(() => expect(changed).toHaveBeenCalledOnce());
    expect(setPlan).toHaveBeenCalledWith('u', { plan: 'developer', reason: 'Trial' });
    expect(grant).not.toHaveBeenCalled();
  });

  it('restores the subscription with an explicit null override', async () => {
    const { setPlan } = fixture();
    fill('Reason for adjustment', 'Restore');
    fireEvent.click(screen.getByText('Save plan access'));
    await waitFor(() => expect(setPlan).toHaveBeenCalledWith('u', { plan: null, reason: 'Restore' }));
  });

  it('retries a failed grant with the same id and clears inputs after success', async () => {
    const { grant, changed } = fixture();
    grant.mockRejectedValueOnce(new Error('Connection lost'));
    fill('Reason for adjustment', 'Support');
    fill('Extra cloud hours', '2');
    fill('Hosted AI credits (USD)', '1.25');
    fireEvent.click(screen.getByText('Grant allowance'));
    await screen.findByRole('alert');
    expect(changed).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText('Grant allowance'));
    await waitFor(() => expect(changed).toHaveBeenCalledOnce());
    expect(grant.mock.calls[0]).toEqual(grant.mock.calls[1]);
    expect(grant.mock.calls[0][1]).toMatchObject({ hours: 2, credits: 1.25, reason: 'Support' });
    expect((screen.getByLabelText('Extra cloud hours') as HTMLInputElement).value).toBe('');
  });

  it('prevents duplicate clicks while a grant is pending', async () => {
    const { grant } = fixture();
    grant.mockReturnValue(new Promise(() => {}));
    fill('Reason for adjustment', 'Support');
    fill('Extra cloud hours', '1');
    fireEvent.click(screen.getByText('Grant allowance'));
    fireEvent.click(screen.getByText('Grant allowance'));
    await waitFor(() => expect(grant).toHaveBeenCalledOnce());
  });
});
