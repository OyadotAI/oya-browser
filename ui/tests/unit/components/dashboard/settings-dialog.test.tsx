/**
 * Unit tests for the settings dialog: it loads the key's settings, saves only
 * what changed, guards a provider switch, and reports failures in place.
 */
import { describe, it, expect, vi, afterEach, beforeEach } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (importActual) => ({
  ...(await importActual<typeof import('@/lib/api-client')>()),
  api: vi.fn(),
}));

import { api } from '@/lib/api-client';
import SettingsDialog from '@/components/dashboard/settings-dialog';
import { ToastProvider } from '@/components/dashboard/toast';
import { LLM_CATALOG } from '../../support/llm-catalog';

const apiMock = vi.mocked(api);

/** Settings as GET /config returns them. */
const CONFIG = {
  llm_provider: 'openai',
  openai_api_key: 'sk-…abcd',
  openai_base_url: '',
  chat_model: 'gpt-4o-mini',
  browser_provider: 'steel',
  captcha_solver: '',
  captcha_api_key: '',
  inherited: false,
  effective: { baseUrl: 'https://api.openai.com/v1', model: 'gpt-4o-mini', hasLlmKey: true },
  providers: [{ id: 'steel', label: 'Steel', needs: ['steel_api_key'], configured: true }],
  llm_catalog: LLM_CATALOG,
};

/** Opens the dialog on `section`; returns the close spy. */
function setup(initialSection?: 'model' | 'browsers' | 'verification') {
  const onClose = vi.fn();
  render(
    <ToastProvider>
      <SettingsDialog open onClose={onClose} apiKey="k" initialSection={initialSection} />
    </ToastProvider>,
  );
  return onClose;
}

/** The footer's Save button. */
const saveButton = () => screen.getByRole('button', { name: 'Save' }) as HTMLButtonElement;

describe('SettingsDialog', () => {
  beforeEach(() => {
    apiMock.mockResolvedValue(CONFIG);
  });
  afterEach(cleanup);

  it('renders nothing while closed', () => {
    render(<SettingsDialog open={false} onClose={() => {}} apiKey="k" />);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('opens on the requested section with the saved values', async () => {
    setup('browsers');
    expect(await screen.findByText('A home for your browsers.')).toBeTruthy();
    expect((screen.getByLabelText('Default provider') as HTMLSelectElement).value).toBe('steel');
    expect(screen.getByLabelText('Steel API key')).toBeTruthy();
  });

  it('shows a load failure with Retry, and retries', async () => {
    apiMock.mockRejectedValueOnce(new Error('Server down'));
    setup();
    expect((await screen.findByRole('alert')).textContent).toContain('Server down');
    await userEvent.click(screen.getByRole('button', { name: 'Retry' }));
    expect(await screen.findByText('Choose how Oya thinks.')).toBeTruthy();
  });

  it('saves only what changed, then confirms and closes', async () => {
    const onClose = setup('verification');
    await userEvent.selectOptions(await screen.findByLabelText('CAPTCHA solver'), 'capsolver');
    await userEvent.click(saveButton());
    expect(apiMock).toHaveBeenLastCalledWith('/config', {
      key: 'k',
      method: 'POST',
      body: { captcha_solver: 'capsolver' },
    });
    await waitFor(() => expect(onClose).toHaveBeenCalled());
  });

  it('is clean again once a field is set back to its saved value', async () => {
    setup('verification');
    const solver = await screen.findByLabelText('CAPTCHA solver');
    await userEvent.selectOptions(solver, 'capsolver');
    expect(screen.getByText('Unsaved changes')).toBeTruthy();
    await userEvent.selectOptions(solver, '');
    expect(screen.getByText('Changes apply to this API key')).toBeTruthy();
    expect(saveButton().disabled).toBe(true);
  });

  it('asks for a key before a provider switch can be saved', async () => {
    setup();
    await userEvent.click(await screen.findByRole('button', { name: 'Claude' }));
    expect(screen.getByText('Enter a key for Claude to switch providers.')).toBeTruthy();
    expect(saveButton().disabled).toBe(true);
    await userEvent.type(screen.getByLabelText('API key'), 'sk-ant');
    await userEvent.click(saveButton());
    const body = {
      llm_provider: 'anthropic',
      chat_model: 'claude-opus-5',
      openai_base_url: '',
      openai_api_key: 'sk-ant',
    };
    expect(apiMock).toHaveBeenLastCalledWith('/config', { key: 'k', method: 'POST', body });
  });

  it('keeps the dialog open with the reason when saving fails', async () => {
    const onClose = setup('verification');
    await userEvent.selectOptions(await screen.findByLabelText('CAPTCHA solver'), 'capsolver');
    apiMock.mockRejectedValueOnce(new Error('Invalid solver'));
    await userEvent.click(saveButton());
    expect((await screen.findByRole('alert')).textContent).toBe('Invalid solver');
    expect(onClose).not.toHaveBeenCalled();
  });

  it('moves between sections with the arrow keys', async () => {
    setup();
    await screen.findByText('Choose how Oya thinks.');
    screen.getByRole('tab', { name: 'AI model' }).focus();
    await userEvent.keyboard('{ArrowDown}');
    expect(screen.getByRole('tab', { name: 'Browsers' }).getAttribute('aria-selected')).toBe('true');
    expect(document.activeElement).toBe(screen.getByRole('tab', { name: 'Browsers' }));
  });
});
