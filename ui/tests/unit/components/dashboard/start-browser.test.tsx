/**
 * Unit tests for the start-browser dialog, through what the user sees and
 * clicks: readiness, success, partial failure.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));

import { api } from '@/lib/api-client';
import StartBrowser from '@/components/dashboard/start-browser';
import { ToastProvider } from '@/components/dashboard/toast';

const apiMock = vi.mocked(api);
const ready = [{ id: 'oya-cloud', label: 'Oya Cloud', configured: true }];

/** Renders the dialog open, with spies for its callbacks. */
function setup(providers = ready) {
  const props = { onClose: vi.fn(), onStarted: vi.fn() };
  render(
    <ToastProvider>
      <StartBrowser open apiKey="k" personas={[]} defaultProvider="oya-cloud" providers={providers} {...props} />
    </ToastProvider>,
  );
  return props;
}

describe('StartBrowser', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });
  afterEach(cleanup);

  it('starts a browser, reports it and closes', async () => {
    apiMock.mockResolvedValue({});
    const { onClose, onStarted } = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Start' }));
    expect(onStarted).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
    expect(screen.getByText('Browser starting')).toBeTruthy();
  });

  it('says how many will start on the button', async () => {
    setup();
    fireEvent.change(screen.getByLabelText('How many'), { target: { value: '3' } });
    expect(screen.getByRole('button', { name: 'Start 3' })).toBeTruthy();
  });

  it('keeps the dialog open with the error when a start fails', async () => {
    apiMock.mockRejectedValueOnce(new Error('No capacity'));
    const { onClose } = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Start' }));
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toBe('No capacity');
    expect(onClose).not.toHaveBeenCalled();
  });

  it('disables Start and explains why when the provider is not ready', () => {
    setup([{ id: 'oya-cloud', label: 'Oya Cloud', configured: false }]);
    expect((screen.getByRole('button', { name: 'Start' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByRole('alert').textContent).toMatch(/Cloud browsers are unavailable/);
  });
});
