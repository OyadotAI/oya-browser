/**
 * Unit tests for the self-hosted license section: the bounds on what it sells,
 * the way out to Checkout, the wait for the key after paying, and the key and
 * its billing once bought.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { SelfHostedLicense } from '@/components/dashboard/billing/self-hosted-license';
import { getLicense, licenseCheckout, licensePortal } from '@/lib/api';

vi.mock('@/components/auth-provider', () => ({ useAuth: () => ({ token: 'jwt' }) }));
vi.mock('@/lib/api', () => ({ getLicense: vi.fn(), licenseCheckout: vi.fn(), licensePortal: vi.fn() }));

/** A monthly license for 12 browsers. */
const LICENSE = {
  id: 'L1',
  browsers: 12,
  interval: 'month',
  key: 'KEY-123',
  expiresAt: '2099-01-06T12:00:00Z',
  issuedAt: '2026-11-23T12:00:00Z',
  status: 'active',
  cancelAtPeriodEnd: false,
  renewsAt: '2026-12-23T12:00:00Z',
  previousExpiresAt: null,
};

/** Renders the section with this answer from the server, at this address. */
async function shown(answer: object, search = '') {
  window.history.replaceState(null, '', `/dashboard/billing${search}`);
  vi.mocked(getLicense).mockResolvedValue(answer);
  render(<SelfHostedLicense />);
  await act(async () => {});
}

/** Sets the browser count field. */
const setCount = (n: string) =>
  fireEvent.change(screen.getByLabelText('Concurrent browsers'), { target: { value: n } });

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.useRealTimers();
});

describe('SelfHostedLicense', () => {
  it('shows nothing on a self-hosted server', async () => {
    await shown({ enabled: false });
    expect(screen.queryByText('Self-hosted license')).toBeNull();
  });

  it('offers 6 browsers monthly by default, with the total', async () => {
    await shown({ enabled: true, license: null });
    expect(screen.getByText('$120 a month')).toBeTruthy();
    fireEvent.click(screen.getByLabelText('Yearly, $240 per browser'));
    expect(screen.getByText('$1,440 a year')).toBeTruthy();
  });

  it('refuses fewer than 6 browsers', async () => {
    await shown({ enabled: true, license: null });
    setCount('5');
    expect(screen.getByText('Up to 5 run free. Choose 6 or more.')).toBeTruthy();
    expect((screen.getByText('Buy license') as HTMLButtonElement).disabled).toBe(true);
  });

  it('sends more than 50 browsers to a call with the founders', async () => {
    await shown({ enabled: true, license: null });
    setCount('51');
    expect(screen.getByText('book a time with the founders').closest('a')?.getAttribute('href')).toBe(
      'https://calendly.com/d/dvrm-r65-kkx/oya-founder-call',
    );
    expect((screen.getByText('Buy license') as HTMLButtonElement).disabled).toBe(true);
  });

  it('opens Checkout for the count and period chosen', async () => {
    await shown({ enabled: true, license: null });
    vi.mocked(licenseCheckout).mockResolvedValue({ url: 'https://stripe.test/c' });
    setCount('12');
    fireEvent.click(screen.getByLabelText('Yearly, $240 per browser'));
    await act(async () => fireEvent.click(screen.getByText('Buy license')));
    expect(licenseCheckout).toHaveBeenCalledWith('jwt', 12, 'year');
  });

  it('says when Checkout could not open, keeping the form', async () => {
    await shown({ enabled: true, license: null });
    vi.mocked(licenseCheckout).mockRejectedValue(new Error('Stripe: down'));
    setCount('9');
    await act(async () => fireEvent.click(screen.getByText('Buy license')));
    expect(screen.getByText('Could not open checkout. Try again.')).toBeTruthy();
    expect((screen.getByLabelText('Concurrent browsers') as HTMLInputElement).value).toBe('9');
  });

  it('after paying, waits for the key, then says when it is slow', async () => {
    vi.useFakeTimers();
    await shown({ enabled: true, license: null }, '?license=paid');
    expect(screen.getByText(/Issuing your key/)).toBeTruthy();
    await act(async () => vi.advanceTimersByTime(30_000));
    expect(screen.getByText(/Still issuing/)).toBeTruthy();
    expect(vi.mocked(getLicense).mock.calls.length).toBeGreaterThan(1);
  });

  it('shows the key with how to use it, and copies it', async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.assign(navigator, { clipboard: { writeText } });
    await shown({ enabled: true, license: LICENSE });
    expect((screen.getByLabelText('License key') as HTMLInputElement).value).toBe('KEY-123');
    expect(screen.getByText(/12 browsers, monthly\. Renews/)).toBeTruthy();
    await act(async () => fireEvent.click(screen.getByText('Copy')));
    expect(writeText).toHaveBeenCalledWith('KEY-123');
    expect(screen.getByText('Copied')).toBeTruthy();
  });

  it('after a renewal says to replace the old key before it stops', async () => {
    await shown({ enabled: true, license: { ...LICENSE, previousExpiresAt: '2027-01-06T12:00:00Z' } });
    expect(screen.getByText(/New key issued/)).toBeTruthy();
  });

  it('says a failed payment issues no new key, and offers to update the card', async () => {
    await shown({ enabled: true, license: { ...LICENSE, status: 'past_due' } });
    expect(screen.getByText(/Your last payment failed/)).toBeTruthy();
    vi.mocked(licensePortal).mockResolvedValue({ url: 'https://stripe.test/p' });
    await act(async () => fireEvent.click(screen.getByText('Update card')));
    expect(licensePortal).toHaveBeenCalledWith('jwt');
  });

  it('once cancelled says when the key stops, and offers to buy again', async () => {
    await shown({ enabled: true, license: { ...LICENSE, cancelAtPeriodEnd: true, renewsAt: null } });
    expect(screen.getByText(/Cancelled\. Your key works until/)).toBeTruthy();
    expect(screen.getByText('Buy license')).toBeTruthy();
  });

  it('after buying again over a cancelled license, waits for the new key', async () => {
    await shown({ enabled: true, license: { ...LICENSE, cancelAtPeriodEnd: true, renewsAt: null } }, '?license=paid');
    expect(screen.getByText(/Issuing your key/)).toBeTruthy();
  });

  it('once the key has expired says so, and offers to buy again', async () => {
    const ended = { ...LICENSE, status: 'canceled', renewsAt: null, expiresAt: '2020-01-01T00:00:00Z' };
    await shown({ enabled: true, license: ended });
    expect(screen.getByText(/Your key stopped working/)).toBeTruthy();
    expect(screen.getByText('Buy license')).toBeTruthy();
  });
});
