/**
 * Unit tests for starting browsers: request bodies, stopping at the first
 * failure, the count clamp and the unavailable-provider message.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));

import { api } from '@/lib/api-client';
import { clampCount, startBody, startMany, unavailableReason } from '@/components/dashboard/start/start-run';
import { MAX_BROWSERS_PER_START } from '@/components/dashboard/start/constants';

const apiMock = vi.mocked(api);
const form = { persona: 'p1', provider: '', name: '', count: 1 };

describe('startBody', () => {
  it('sends only the persona when nothing else is chosen', () => {
    expect(startBody(form, 0)).toEqual({ persona: 'p1' });
  });

  it('numbers names from 1 when starting several', () => {
    expect(startBody({ ...form, name: 'w', count: 3 }, 1)).toEqual({ persona: 'p1', name: 'w 2' });
  });

  it('keeps the name as typed for a single browser and passes a provider override', () => {
    expect(startBody({ ...form, name: 'w', provider: 'steel' }, 0)).toEqual({
      persona: 'p1',
      provider: 'steel',
      name: 'w',
    });
  });
});

describe('startMany', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });

  it('starts one request per browser', async () => {
    apiMock.mockResolvedValue({});
    expect(await startMany('k', { ...form, count: 3 })).toEqual({ ok: 3, lastErr: '' });
    expect(apiMock).toHaveBeenCalledTimes(3);
    expect(apiMock).toHaveBeenCalledWith('/browsers/start', { key: 'k', method: 'POST', body: { persona: 'p1' } });
  });

  it('stops at the first failure and reports it', async () => {
    apiMock.mockResolvedValueOnce({}).mockRejectedValueOnce(new Error('Quota reached'));
    expect(await startMany('k', { ...form, count: 5 })).toEqual({ ok: 1, lastErr: 'Quota reached' });
    expect(apiMock).toHaveBeenCalledTimes(2);
  });
});

describe('clampCount', () => {
  it('keeps the count between 1 and the per-click maximum', () => {
    expect(clampCount('0')).toBe(1);
    expect(clampCount('7')).toBe(7);
    expect(clampCount('999')).toBe(MAX_BROWSERS_PER_START);
  });

  it('treats unreadable input as 1', () => {
    expect(clampCount('')).toBe(1);
  });
});

describe('unavailableReason', () => {
  it('explains that cloud setup is unfinished for Oya providers', () => {
    expect(unavailableReason('oya-cloud')).toMatch(/^Cloud browsers are unavailable/);
  });

  it('names a third-party provider that lacks settings', () => {
    expect(unavailableReason('steel')).toMatch(/^Steel is not configured/);
  });
});
