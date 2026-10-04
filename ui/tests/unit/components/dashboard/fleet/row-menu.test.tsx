/**
 * Unit tests for the fleet row menu's CDP attach copy: it copies a fresh
 * ticketed URL from the server and never puts the key in a URL.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { rowMenuItems, type RowMenuActions } from '@/components/dashboard/fleet/row-menu';
import { api } from '@/lib/api-client';
import { row } from './fixtures';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));

afterEach(() => vi.mocked(api).mockReset());

/** Menu callbacks with a spy for notify; the key is SECRET so a leak is easy to spot. */
const actions = (): RowMenuActions => ({
  apiKey: 'SECRET',
  onSelect: vi.fn(),
  onConnect: vi.fn(),
  onScreenshot: vi.fn(),
  onStop: vi.fn(),
  notify: vi.fn(),
});

/** Picks the CDP attach item for a CDP browser and returns what was copied and said. */
async function copyAttach() {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
  const a = actions();
  const item = rowMenuItems(row({ clientType: 'cdp' }), a).find((i) => i.label.startsWith('Copy CDP attach URL'))!;
  item.onSelect();
  await vi.waitFor(() => expect(a.notify).toHaveBeenCalled());
  return { writeText, notify: a.notify };
}

describe('row menu CDP attach', () => {
  it('copies the ticketed cdpUrl the server hands out, never the key', async () => {
    vi.mocked(api).mockResolvedValueOnce({ cdpUrl: 'wss://h/connect?ticket=t1&browser=b-1' });
    const { writeText, notify } = await copyAttach();
    expect(api).toHaveBeenCalledWith('/browsers/b-1', { key: 'SECRET' });
    expect(writeText).toHaveBeenCalledWith('wss://h/connect?ticket=t1&browser=b-1');
    expect(notify).toHaveBeenCalledWith('CDP attach URL copied', 'success');
  });

  it('says why when the server offers no CDP URL', async () => {
    vi.mocked(api).mockResolvedValueOnce({});
    const { writeText, notify } = await copyAttach();
    expect(writeText).not.toHaveBeenCalled();
    expect(notify).toHaveBeenCalledWith('Could not copy: this browser offers no CDP URL', 'error');
  });

  it('says why when the ticket cannot be fetched', async () => {
    vi.mocked(api).mockRejectedValueOnce(new Error('denied'));
    const { notify } = await copyAttach();
    expect(notify).toHaveBeenCalledWith('Could not copy: denied', 'error');
  });
});
