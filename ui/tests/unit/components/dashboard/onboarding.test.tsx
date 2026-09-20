/**
 * Unit tests for onboarding, through what the user sees and clicks: the
 * desktop state, saved sites, the provider's credentials, saving, and copying
 * the SDK example.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/components/dashboard/config', () => ({ saveConfig: vi.fn(), desktopSignInUrl: vi.fn() }));
vi.mock('@/lib/api', () => ({ apiOrigin: () => 'https://oya.example' }));
vi.mock('@/components/ui/syntax-code', () => ({ default: ({ code }: { code: string }) => <code>{code}</code> }));

import { saveConfig, desktopSignInUrl, type KeyConfig } from '@/components/dashboard/config';
import Onboarding from '@/components/dashboard/onboarding';
import { ToastProvider } from '@/components/dashboard/toast';
import type { BrowserRow, Persona } from '@/components/dashboard/types';

const config = {
  browser_provider: 'steel',
  providers: [
    { id: 'oya-cloud', label: 'Oya Cloud', needs: [], configured: true },
    { id: 'steel', label: 'Steel', needs: ['steel_api_key'], configured: false },
  ],
} as unknown as KeyConfig;
const persona = {
  id: 'p0',
  name: 'Default',
  isDefault: true,
  login: { cookies: 3, sites: ['a.com'], updatedAt: null },
};

/** Renders onboarding with the given personas and browsers. */
function setup(personas = [persona], browsers: Partial<BrowserRow>[] = []) {
  const onDone = vi.fn();
  render(
    <ToastProvider>
      <Onboarding
        apiKey="key-1"
        config={config}
        personas={personas as Persona[]}
        browsers={browsers as BrowserRow[]}
        onDone={onDone}
      />
    </ToastProvider>,
  );
  return onDone;
}

describe('Onboarding', () => {
  beforeEach(() => {
    vi.mocked(saveConfig).mockReset();
  });
  afterEach(cleanup);

  it('offers to connect until a desktop runs as the profile', () => {
    setup();
    expect(screen.getByRole('button', { name: 'Connect desktop' })).toBeTruthy();
  });

  it('offers to open the desktop once one runs as the profile', () => {
    setup([persona], [{ provider: 'oya-desktop', persona: 'p0' }]);
    expect(screen.getByRole('button', { name: 'Open desktop' })).toBeTruthy();
  });

  it('pairs the chosen profile', async () => {
    vi.mocked(desktopSignInUrl).mockResolvedValueOnce('#x');
    setup();
    await userEvent.click(screen.getByRole('button', { name: 'Connect desktop' }));
    expect(desktopSignInUrl).toHaveBeenCalledWith('key-1', 'default');
  });

  it('counts the sites the profile has saved', () => {
    setup();
    expect(screen.getByText('Saved state for 1 site')).toBeTruthy();
  });

  it('asks for what the chosen provider needs', () => {
    setup();
    expect(screen.getByLabelText('steel api key')).toBeTruthy();
  });

  it('saves the provider and its credentials, marks the key onboarded, then finishes', async () => {
    vi.mocked(saveConfig).mockResolvedValueOnce({} as KeyConfig);
    const onDone = setup();
    await userEvent.type(screen.getByLabelText('steel api key'), 's3');
    await userEvent.click(screen.getByRole('button', { name: /Save and open console/ }));
    expect(saveConfig).toHaveBeenCalledWith('key-1', {
      steel_api_key: 's3',
      browser_provider: 'steel',
      onboarded: 'true',
    });
    expect(onDone).toHaveBeenCalled();
  });

  it('stays put and toasts when saving fails', async () => {
    vi.mocked(saveConfig).mockRejectedValueOnce(new Error('Bad key'));
    const onDone = setup();
    await userEvent.click(screen.getByRole('button', { name: /Go to console/ }));
    expect(await screen.findByText('Bad key')).toBeTruthy();
    expect(onDone).not.toHaveBeenCalled();
  });

  it('shows a placeholder key but copies the real one', async () => {
    const user = userEvent.setup();
    const write = vi.spyOn(navigator.clipboard, 'writeText').mockResolvedValue();
    setup();
    expect(screen.getByText(/<your-api-key>/)).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Copy with your key' }));
    expect(write.mock.calls[0][0]).toContain('"key-1"');
    expect(screen.getByRole('button', { name: 'Copied with your key' })).toBeTruthy();
  });
});
