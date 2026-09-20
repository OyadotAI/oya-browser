/**
 * Unit tests for the control-mode tables and the control toggle.
 */
import { describe, it, expect, vi } from 'vitest';
import { buttonLabel, controlAction, modeText, toggleControl } from '@/components/dashboard/browser/control';
import type { PanelContext } from '@/components/dashboard/browser/context';
import { api } from '@/lib/api-client';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));

/** A context whose toast is a spy. */
const ctx = () => ({ browserId: 'b', apiKey: 'k', toast: vi.fn() }) as unknown as PanelContext;

describe('control', () => {
  it('takes control from the agent, releases it from a person, and otherwise resumes the agent', () => {
    expect([controlAction('agent'), controlAction('human'), controlAction('paused')]).toEqual([
      'acquire',
      'release',
      'resume',
    ]);
    expect([buttonLabel('agent'), buttonLabel('human'), buttonLabel('paused')]).toEqual([
      'Take control',
      'Release control',
      'Resume agent',
    ]);
  });

  it('reads an unknown mode as agent control in the status line', () => {
    expect(modeText('human')).toBe('Human control · agent paused');
    expect(modeText('paused')).toBe('Paused · awaiting agent resume');
    expect(modeText('toString')).toBe('Agent control · take control to drive');
  });

  it('stores the mode the server answers with', async () => {
    vi.mocked(api).mockResolvedValueOnce({ mode: 'human' });
    const setMode = vi.fn();
    await toggleControl(ctx(), 'agent', setMode);
    expect(api).toHaveBeenCalledWith('/control/sessions/b/control', {
      key: 'k',
      method: 'POST',
      body: { action: 'acquire' },
    });
    expect(setMode).toHaveBeenCalledWith('human');
  });

  it('toasts a refused control change and keeps the mode', async () => {
    vi.mocked(api).mockRejectedValueOnce(new Error('held by someone else'));
    const c = ctx();
    const setMode = vi.fn();
    await toggleControl(c, 'agent', setMode);
    expect(c.toast).toHaveBeenCalledWith('held by someone else', 'error');
    expect(setMode).not.toHaveBeenCalled();
  });
});
