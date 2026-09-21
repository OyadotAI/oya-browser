/**
 * Unit tests for the key settings helpers: the requests they make and the
 * desktop sign-in link they build.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@/lib/api-client', () => ({ api: vi.fn() }));
vi.mock('@/lib/api', () => ({ apiOrigin: () => 'https://oya.example' }));

import { api } from '@/lib/api-client';
import { desktopSignInUrl, isOyaProvider, loadConfig, saveConfig } from '@/components/dashboard/config';

const apiMock = vi.mocked(api);

describe('config', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });

  it('loads settings for the key', async () => {
    apiMock.mockResolvedValueOnce({ chat_model: 'm' });
    expect(await loadConfig('k')).toEqual({ chat_model: 'm' });
    expect(apiMock).toHaveBeenCalledWith('/config', { key: 'k' });
  });

  it('saves settings with a POST', async () => {
    apiMock.mockResolvedValueOnce({});
    await saveConfig('k', { chat_model: 'm' });
    expect(apiMock).toHaveBeenCalledWith('/config', { key: 'k', method: 'POST', body: { chat_model: 'm' } });
  });

  it('builds an oya:// link with the pairing code and the websocket server, never the key', async () => {
    apiMock.mockResolvedValueOnce({ code: 'a b' });
    const url = await desktopSignInUrl('secret-key', 'p1');
    expect(apiMock).toHaveBeenCalledWith('/pairing', { key: 'secret-key', method: 'POST', body: { profile: 'p1' } });
    expect(url).toBe('oya://connect?code=a%20b&server=wss%3A%2F%2Foya.example%2Fws');
    expect(url).not.toContain('secret-key');
  });

  it('pairs the default profile when none is named', async () => {
    apiMock.mockResolvedValueOnce({ code: 'c' });
    await desktopSignInUrl('k');
    expect(apiMock.mock.calls[0][1]).toMatchObject({ body: { profile: 'default' } });
  });

  it('counts only Oya Cloud and self-hosted as Oya providers', () => {
    expect([isOyaProvider('oya-cloud'), isOyaProvider('oya-selfhosted'), isOyaProvider('oya-desktop')]).toEqual([
      true,
      true,
      false,
    ]);
  });
});
