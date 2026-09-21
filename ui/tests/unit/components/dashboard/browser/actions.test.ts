/**
 * Unit tests for the panel's input, reload and share actions.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { sendInput } from '@/components/dashboard/browser/use-send';
import { reloadCommand } from '@/components/dashboard/browser/use-navigation';
import { openStream, share } from '@/components/dashboard/browser/share';
import { withBusy, type PanelContext } from '@/components/dashboard/browser/context';
import type { BrowserDetail } from '@/components/dashboard/types';
import { api } from '@/lib/api-client';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));

afterEach(() => vi.mocked(api).mockReset());

/** A context whose toast and busy setter are spies. */
const ctx = () => ({ browserId: 'b 1', apiKey: 'k', toast: vi.fn(), setBusy: vi.fn() }) as unknown as PanelContext;

describe('sendInput', () => {
  it("toasts the server's refusal", async () => {
    const toast = vi.fn();
    vi.mocked(api).mockResolvedValueOnce({ ok: false });
    expect(await sendInput({ browserId: 'b', apiKey: 'k', toast }, 'click')).toEqual({ ok: false });
    expect(toast).toHaveBeenCalledWith('click failed', 'error');
  });

  it('toasts a network error and answers not-ok instead of throwing', async () => {
    const toast = vi.fn();
    vi.mocked(api).mockRejectedValueOnce(new Error('offline'));
    expect(await sendInput({ browserId: 'b', apiKey: 'k', toast }, 'click')).toEqual({ ok: false });
    expect(toast).toHaveBeenCalledWith('offline', 'error');
  });
});

describe('reloadCommand', () => {
  it('reloads CDP browsers, and re-navigates Oya browsers to their current page', () => {
    expect(reloadCommand({ clientType: 'cdp' } as BrowserDetail)).toEqual(['reload']);
    expect(reloadCommand({ clientType: 'oya', currentUrl: 'https://x' } as BrowserDetail)).toEqual([
      'navigate',
      { url: 'https://x' },
    ]);
    expect(reloadCommand(null)).toBeNull();
  });
});

describe('withBusy', () => {
  it('marks the action busy while it runs', async () => {
    const setBusy = vi.fn();
    expect(await withBusy(setBusy, 'go', async () => 7)).toBe(7);
    expect(setBusy.mock.calls).toEqual([['go'], [null]]);
  });
});

describe('share', () => {
  it('copies a live link carrying the token in the fragment', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });
    vi.mocked(api).mockResolvedValueOnce({ token: 't/1' });
    const c = ctx();
    await share(c, false);
    expect(writeText).toHaveBeenCalledWith(`${window.location.origin}/live/b%201#t=t%2F1`);
    expect(c.toast).toHaveBeenCalledWith('View link copied, expires in 1h', 'success');
  });

  it('shows the link in a toast when the clipboard refuses', async () => {
    Object.assign(navigator, { clipboard: { writeText: vi.fn().mockRejectedValue(new Error('denied')) } });
    vi.mocked(api).mockResolvedValueOnce({ token: 't' });
    const c = ctx();
    await share(c, true);
    expect(c.toast).toHaveBeenCalledWith(`${window.location.origin}/live/b%201#t=t`, 'info');
  });

  it('toasts why a link could not be minted', async () => {
    vi.mocked(api).mockRejectedValueOnce(new Error('forbidden'));
    const c = ctx();
    await share(c, true);
    expect(c.toast).toHaveBeenCalledWith('forbidden', 'error');
  });
});

describe('openStream', () => {
  it('points the tab opened in the click at the minted link', async () => {
    const tab = { close: vi.fn(), location: { href: '' } };
    vi.spyOn(window, 'open').mockReturnValue(tab as unknown as Window);
    vi.mocked(api).mockResolvedValueOnce({ token: 't' });
    await openStream(ctx());
    expect(tab.location.href).toBe(`${window.location.origin}/live/b%201#t=t`);
  });

  it('closes the tab and toasts when the link cannot be minted', async () => {
    const tab = { close: vi.fn(), location: { href: '' } };
    vi.spyOn(window, 'open').mockReturnValue(tab as unknown as Window);
    vi.mocked(api).mockRejectedValueOnce(new Error('forbidden'));
    const c = ctx();
    await openStream(c);
    expect(tab.close).toHaveBeenCalled();
    expect(c.toast).toHaveBeenCalledWith('forbidden', 'error');
  });
});
