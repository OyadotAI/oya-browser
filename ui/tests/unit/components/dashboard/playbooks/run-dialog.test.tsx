/**
 * Unit tests for the run dialog, through what the user sees and does: pick
 * where to run, set values, run, and answer a run that stops for a person.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));

import { api } from '@/lib/api-client';
import RunDialog from '@/components/dashboard/playbooks/run-dialog';
import { ToastProvider } from '@/components/dashboard/toast';
import { RUN_POLL_MS } from '@/components/dashboard/playbooks/constants';
import type { BrowserRow, Persona } from '@/components/dashboard/types';
import type { PlaybookInfo } from '@/components/dashboard/playbooks/types';

const apiMock = vi.mocked(api);
const playbook = { name: 'order', variables: ['name'], defaults: { name: 'Ada' } } as unknown as PlaybookInfo;
const browsers = [{ id: 'b1', name: 'one', persona: 'p1' }] as BrowserRow[];
const personas = [
  { id: 'p1', name: 'Work', isDefault: true },
  { id: 'p2', name: 'Home', isDefault: false },
] as Persona[];

/** Renders the dialog. */
function setup(onFinished = vi.fn()) {
  render(
    <ToastProvider>
      <RunDialog
        apiKey="k"
        playbook={playbook}
        browsers={browsers}
        personas={personas}
        onClose={vi.fn()}
        onFinished={onFinished}
      />
    </ToastProvider>,
  );
  return onFinished;
}

describe('RunDialog', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });
  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it('prefills the variables with what the recording used and runs with them', async () => {
    apiMock.mockResolvedValueOnce({ id: 'r1', status: 'running', attention: null });
    setup();
    const field = screen.getByLabelText('name') as HTMLInputElement;
    expect(field.value).toBe('Ada');
    await userEvent.clear(field);
    await userEvent.type(field, 'Alan');
    await userEvent.click(screen.getByRole('button', { name: 'Run' }));
    expect(apiMock).toHaveBeenCalledWith('/browsers/b1/runs', {
      key: 'k',
      method: 'POST',
      body: { playbook: 'order', data: { name: 'Alan' }, autoHeal: true },
    });
    expect(screen.getByText('running')).toBeTruthy();
  });

  it('offers to start a browser when none runs the chosen profile', async () => {
    setup();
    await userEvent.selectOptions(screen.getByLabelText('Profile'), 'p2');
    expect(screen.getByText(/No browser is running this profile/)).toBeTruthy();
    apiMock.mockResolvedValueOnce({ id: 'new' });
    await userEvent.click(screen.getByRole('button', { name: 'Start one and run' }));
    expect(apiMock).toHaveBeenCalledWith('/browsers/start', { key: 'k', method: 'POST', body: { persona: 'p2' } });
    expect(screen.getByRole('button', { name: 'Starting a browser…' })).toBeTruthy();
  });

  it('asks a person for help when the run pauses, and resumes on reply', async () => {
    apiMock.mockResolvedValueOnce({
      id: 'r1',
      status: 'needs_attention',
      attention: { id: 'a', reason: 'agent', message: 'Which size?' },
    });
    setup();
    await userEvent.click(screen.getByRole('button', { name: 'Run' }));
    expect(screen.getByText('needs a person')).toBeTruthy();
    expect(screen.getByText('Agent question')).toBeTruthy();
    apiMock.mockResolvedValueOnce({});
    await userEvent.type(screen.getByRole('textbox', { name: 'Reply' }), 'Large');
    await userEvent.click(screen.getByRole('button', { name: 'Reply' }));
    expect(apiMock).toHaveBeenLastCalledWith('/runs/r1/respond', {
      key: 'k',
      method: 'POST',
      body: { response: 'Large' },
    });
    expect(screen.getByText('running')).toBeTruthy();
  });

  it('follows the run to its end and reports how it went', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    apiMock.mockResolvedValueOnce({ id: 'r1', status: 'running', attention: null });
    const onFinished = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Run' }));
    apiMock.mockResolvedValueOnce({
      id: 'r1',
      status: 'succeeded',
      attention: null,
      result: { steps: 3, total: 4, healed: true },
    });
    await act(() => vi.advanceTimersByTimeAsync(RUN_POLL_MS));
    expect(screen.getByText('Replayed 3 of 4 steps without the LLM.')).toBeTruthy();
    expect(screen.getByText(/saved its fix as a draft for review/)).toBeTruthy();
    expect(onFinished).toHaveBeenCalled();
  });

  it('shows why a run failed', async () => {
    apiMock.mockResolvedValueOnce({ id: 'r1', status: 'failed', attention: null, error: 'Selector gone' });
    setup();
    await userEvent.click(screen.getByRole('button', { name: 'Run' }));
    expect(screen.getByText('Selector gone')).toBeTruthy();
  });
});
