/**
 * Unit tests for BrowserPanel: one browser, close up.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { createRef } from 'react';
import BrowserPanel from '@/components/dashboard/browser-panel';
import { api } from '@/lib/api-client';

const toast = vi.fn();
vi.mock('@/components/dashboard/toast', () => ({ useToast: () => toast }));
vi.mock('@/components/dashboard/live-view', () => ({ default: () => <div>live view</div> }));
vi.mock('@/lib/live-stream', () => ({ subscribeFrames: () => () => {} }));
vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));

afterEach(() => {
  cleanup();
  vi.mocked(api).mockReset();
  toast.mockReset();
});

const detail = {
  id: 'b-1',
  name: 'alpha',
  clientType: 'cdp',
  provider: 'oya-cloud',
  persona: 'p-1',
  personaName: 'Sales',
  health: 'errors',
  connectedAt: new Date().toISOString(),
  lastSeen: new Date().toISOString(),
  currentUrl: 'https://example.com/',
  commands: 4,
  errors: 1,
  pending: 0,
  lastError: 'boom',
  activity: [],
};

/** Answers every panel request; `mode` is the control mode. */
function answer(mode: string, extra: Record<string, unknown> = {}) {
  vi.mocked(api).mockImplementation(async (path: string) => {
    if (path in extra) return extra[path];
    return path.startsWith('/browsers/') ? detail : { control: { mode } };
  });
}

const props = {
  apiKey: 'k',
  browserId: 'b-1',
  onClose: vi.fn(),
  onStop: vi.fn(),
  onOpenPersona: vi.fn(),
  onConnect: vi.fn(),
  urlRef: createRef<HTMLInputElement>(),
  now: Date.now(),
};

/** Renders the panel and lets the first poll land. */
async function setup(mode = 'agent', extra = {}) {
  answer(mode, extra);
  render(<BrowserPanel {...props} />);
  await act(async () => {});
}

describe('BrowserPanel', () => {
  it('shows the browser, its stats and its last error', async () => {
    await setup();
    expect(screen.getByRole('heading', { name: 'alpha' })).toBeTruthy();
    expect(screen.getByText('Last error: boom')).toBeTruthy();
    expect(screen.getByTitle('Destroys the sandbox')).toBeTruthy();
  });

  it('keeps the URL bar read-only while the agent drives', async () => {
    await setup('agent');
    expect((screen.getByRole('textbox', { name: 'Navigate to URL' }) as HTMLInputElement).readOnly).toBe(true);
    expect((screen.getByRole('button', { name: 'Go' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('takes control from the agent', async () => {
    await setup('agent', { '/control/sessions/b-1/control': { mode: 'human' } });
    await userEvent.click(screen.getByRole('button', { name: 'Take control' }));
    expect(screen.getByRole('button', { name: 'Release control' })).toBeTruthy();
  });

  it('navigates to what was typed once a person holds control', async () => {
    await setup('human', { '/control/sessions/b-1/input': { ok: true } });
    const input = screen.getByRole('textbox', { name: 'Navigate to URL' });
    await userEvent.clear(input);
    await userEvent.type(input, 'https://next.test{Enter}');
    expect(api).toHaveBeenCalledWith('/control/sessions/b-1/input', {
      key: 'k',
      method: 'POST',
      body: { action: 'navigate', params: { url: 'https://next.test' } },
    });
    expect(screen.getByText('navigate https://next.test')).toBeTruthy();
  });

  it('lists visible elements and clicks one', async () => {
    const elements = [
      { id: 3, type: 'button', text: 'Buy', visible: true },
      { id: 4, type: 'a', text: 'Hidden', visible: false },
    ];
    await setup('human', { '/control/sessions/b-1/input': { ok: true, data: { elements } } });
    await userEvent.click(screen.getByRole('button', { name: /Elements/ }));
    expect(screen.getByText('Elements (1 visible)')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: /#3/ }));
    expect(api).toHaveBeenLastCalledWith(
      '/control/sessions/b-1/input',
      expect.objectContaining({ body: { action: 'click', params: { element_id: 3, selector: '[data-ac-id="3"]' } } }),
    );
  });

  it('screenshots through the agent path while the agent drives', async () => {
    await setup('agent', { '/browsers/b-1/command': { ok: true, data: { screenshot: 'data:image/png;base64,x' } } });
    await userEvent.click(screen.getByRole('button', { name: /Screenshot/ }));
    expect(screen.getByRole('img', { name: 'Screenshot' })).toBeTruthy();
  });

  it('opens the persona and stops the browser', async () => {
    await setup();
    await userEvent.click(screen.getByTitle('Open persona'));
    expect(props.onOpenPersona).toHaveBeenCalledWith('p-1');
    await userEvent.click(screen.getByRole('button', { name: /Stop/ }));
    expect(props.onStop).toHaveBeenCalledWith(['b-1']);
  });
});
