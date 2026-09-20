/**
 * Unit tests for the desktop banner: it opens the desktop through a pairing
 * link and can be dismissed.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/components/dashboard/config', () => ({ desktopSignInUrl: vi.fn(async () => '#desktop') }));

import { desktopSignInUrl } from '@/components/dashboard/config';
import DesktopBanner from '@/components/dashboard/desktop-banner';
import { ToastProvider } from '@/components/dashboard/toast';

/** Renders the banner with a dismiss spy. */
function setup() {
  const onDismiss = vi.fn();
  render(
    <ToastProvider>
      <DesktopBanner apiKey="k" onDismiss={onDismiss} />
    </ToastProvider>,
  );
  return onDismiss;
}

describe('DesktopBanner', () => {
  afterEach(cleanup);

  it('opens the desktop browser with a pairing link for the key', async () => {
    setup();
    await userEvent.click(screen.getByRole('button', { name: /Open desktop browser/ }));
    expect(desktopSignInUrl).toHaveBeenCalledWith('k', undefined);
  });

  it('hides when dismissed', async () => {
    const onDismiss = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Dismiss for now' }));
    expect(onDismiss).toHaveBeenCalled();
  });
});
