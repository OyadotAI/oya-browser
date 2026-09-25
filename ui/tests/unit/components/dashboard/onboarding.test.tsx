/**
 * Unit tests for onboarding, through what the user sees and clicks: the
 * desktop state, choosing an AI model for Ask and saving its key, and failure.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/components/dashboard/config', async (actual) => ({
  ...(await actual<object>()),
  saveConfig: vi.fn(),
  desktopSignInUrl: vi.fn(),
}));
vi.mock('@/lib/api', () => ({ apiOrigin: () => 'https://oya.example' }));

import { saveConfig, desktopSignInUrl, type KeyConfig } from '@/components/dashboard/config';
import Onboarding from '@/components/dashboard/onboarding';
import { ToastProvider } from '@/components/dashboard/toast';
import type { BrowserRow, Persona } from '@/components/dashboard/types';
import { LLM_CATALOG } from '../../support/llm-catalog';

const noModel = { llm_provider: '', effective: { hasLlmKey: false }, llm_catalog: LLM_CATALOG } as unknown as KeyConfig;
const persona = {
  id: 'p0',
  name: 'Default',
  isDefault: true,
  login: { cookies: 3, sites: ['a.com'], updatedAt: null },
};

/** Renders onboarding with the given personas and browsers. */
function setup(browsers: Partial<BrowserRow>[] = [], config = noModel) {
  const onDone = vi.fn();
  render(
    <ToastProvider>
      <Onboarding
        apiKey="key-1"
        config={config}
        personas={[persona] as Persona[]}
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

  it('offers to connect until a desktop runs as the default profile', () => {
    setup();
    expect(screen.getByRole('button', { name: 'Connect desktop' })).toBeTruthy();
  });

  it('offers to open the desktop once one is connected', () => {
    setup([{ provider: 'oya-desktop', persona: 'p0' }]);
    expect(screen.getByRole('button', { name: 'Open desktop' })).toBeTruthy();
    expect(screen.getByText('Connected')).toBeTruthy();
  });

  it('pairs the desktop through a pairing link', async () => {
    vi.mocked(desktopSignInUrl).mockResolvedValueOnce('#x');
    setup();
    await userEvent.click(screen.getByRole('button', { name: 'Connect desktop' }));
    expect(desktopSignInUrl).toHaveBeenCalledWith('key-1', undefined);
  });

  it('says Ask needs an AI model', () => {
    setup();
    expect(screen.getByText(/Ask needs one to think/)).toBeTruthy();
  });

  it('shows the desktop clip first, then the model clip once the desktop is connected', () => {
    setup();
    expect(document.querySelector('video')?.getAttribute('src')).toBe('/onboarding-ask-demo.mp4');
    cleanup();
    setup([{ provider: 'oya-desktop', persona: 'p0' }]);
    expect(document.querySelector('video')?.getAttribute('src')).toBe('/onboarding-ask-key.mp4');
  });

  it('saves the typed key on the chosen provider’s defaults, marks the key onboarded, then finishes', async () => {
    vi.mocked(saveConfig).mockResolvedValueOnce({} as KeyConfig);
    const onDone = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Gemini' }));
    await userEvent.type(screen.getByLabelText('API key'), ' AIza1 ');
    await userEvent.click(screen.getByRole('button', { name: 'Save and continue' }));
    expect(saveConfig).toHaveBeenCalledWith('key-1', {
      llm_provider: 'gemini',
      openai_api_key: 'AIza1',
      chat_model: '',
      openai_base_url: '',
      onboarded: 'true',
    });
    expect(onDone).toHaveBeenCalled();
  });

  it('can be skipped without a key, leaving the model alone', async () => {
    vi.mocked(saveConfig).mockResolvedValueOnce({} as KeyConfig);
    setup();
    await userEvent.click(screen.getByRole('button', { name: 'Skip for now' }));
    expect(saveConfig).toHaveBeenCalledWith('key-1', { onboarded: 'true' });
  });

  it('marks the model step done when the project already has one', () => {
    setup([], { ...noModel, llm_provider: 'openai', effective: { hasLlmKey: true } } as unknown as KeyConfig);
    expect(screen.getByRole('button', { name: 'OpenAI' }).getAttribute('aria-pressed')).toBe('true');
    expect(screen.getByText('Ready')).toBeTruthy();
    expect(screen.getByPlaceholderText(/replace the saved one/)).toBeTruthy();
  });

  it('offers OpenRouter beside the common providers, and leaves Gemini Enterprise to Settings', () => {
    setup();
    expect(screen.getByRole('button', { name: 'OpenRouter' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Gemini Enterprise' })).toBeNull();
  });

  it('stays put and toasts when saving fails', async () => {
    vi.mocked(saveConfig).mockRejectedValueOnce(new Error('Bad key'));
    const onDone = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Skip for now' }));
    expect(await screen.findByText('Bad key')).toBeTruthy();
    expect(onDone).not.toHaveBeenCalled();
  });
});
