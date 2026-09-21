/**
 * Unit tests for reading a recording's status: the one-off restore check, the
 * poll loop, and sending input.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));

import { api } from '@/lib/api-client';
import { RECORD_POLL_MS } from '@/components/dashboard/playbooks/constants';
import { pollStatus, restoreStopped, sendAction, whenLive } from '@/components/dashboard/playbooks/record-status';

const apiMock = vi.mocked(api);
const target = { apiKey: 'k', browserId: 'b 1' };
const status = (recording: boolean, n = 1) => ({ recording, steps: Array(n).fill({ action: 'click' }), secrets: [] });

describe('restoreStopped', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });

  it('returns a stopped recording that has steps', async () => {
    apiMock.mockResolvedValueOnce(status(false));
    expect(await restoreStopped('k', 'b 1')).toEqual(status(false));
    expect(apiMock).toHaveBeenCalledWith('/control/sessions/b%201/record', {
      key: 'k',
      method: 'POST',
      body: { mode: 'status' },
    });
  });

  it('leaves an active recording to be rejoined through Start', async () => {
    apiMock.mockResolvedValueOnce(status(true));
    expect(await restoreStopped('k', 'b')).toBeNull();
  });

  it('ignores an empty recording and a failed check', async () => {
    apiMock.mockResolvedValueOnce(status(false, 0));
    expect(await restoreStopped('k', 'b')).toBeNull();
    apiMock.mockRejectedValueOnce(new Error('down'));
    expect(await restoreStopped('k', 'b')).toBeNull();
  });
});

describe('whenLive', () => {
  it('drops the value once cancelled', async () => {
    const then = vi.fn();
    const cancel = whenLive(Promise.resolve(1), then);
    cancel();
    await Promise.resolve();
    expect(then).not.toHaveBeenCalled();
  });
});

describe('pollStatus', () => {
  beforeEach(() => {
    apiMock.mockReset();
    vi.useFakeTimers();
  });
  afterEach(() => vi.useRealTimers());

  it('asks once per interval, each after the last answered', async () => {
    apiMock.mockResolvedValue(status(true));
    const sink = { state: vi.fn(), error: vi.fn() };
    const stop = pollStatus(target, sink);
    expect(apiMock).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(RECORD_POLL_MS);
    await vi.advanceTimersByTimeAsync(RECORD_POLL_MS);
    expect(apiMock).toHaveBeenCalledTimes(2);
    expect(sink.state).toHaveBeenCalledTimes(2);
    stop();
  });

  it('reports a failed request and keeps polling', async () => {
    apiMock.mockRejectedValueOnce(new Error('gone')).mockResolvedValueOnce(status(true));
    const sink = { state: vi.fn(), error: vi.fn() };
    const stop = pollStatus(target, sink);
    await vi.advanceTimersByTimeAsync(RECORD_POLL_MS * 2);
    expect(sink.error).toHaveBeenCalledWith('gone');
    expect(sink.state).toHaveBeenCalledTimes(1);
    stop();
  });

  it('drops an answer that lands after stop, and schedules nothing more', async () => {
    let answer: (v: unknown) => void = () => {};
    apiMock.mockReturnValueOnce(new Promise((r) => (answer = r)));
    const sink = { state: vi.fn(), error: vi.fn() };
    const stop = pollStatus(target, sink);
    await vi.advanceTimersByTimeAsync(RECORD_POLL_MS);
    stop();
    answer(status(true));
    await vi.advanceTimersByTimeAsync(RECORD_POLL_MS * 3);
    expect(sink.state).not.toHaveBeenCalled();
    expect(apiMock).toHaveBeenCalledTimes(1);
  });
});

describe('sendAction', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });

  it('posts the action and returns the answer', async () => {
    apiMock.mockResolvedValueOnce({ ok: true });
    const toast = vi.fn();
    expect(await sendAction(target, toast, 'click', { x: 1 })).toEqual({ ok: true });
    expect(apiMock).toHaveBeenCalledWith('/control/sessions/b%201/input', {
      key: 'k',
      method: 'POST',
      body: { action: 'click', params: { x: 1 } },
    });
    expect(toast).not.toHaveBeenCalled();
  });

  it('toasts a refusal with the browser reason, or a default', async () => {
    const toast = vi.fn();
    apiMock.mockResolvedValueOnce({ ok: false, error: 'not held' });
    await sendAction(target, toast, 'click', {});
    apiMock.mockResolvedValueOnce({ ok: false });
    await sendAction(target, toast, 'type', {});
    expect(toast.mock.calls).toEqual([
      ['not held', 'error'],
      ['type failed', 'error'],
    ]);
  });

  it('turns a failed request into ok: false', async () => {
    const toast = vi.fn();
    apiMock.mockRejectedValueOnce(new Error('offline'));
    expect(await sendAction(target, toast, 'click', {})).toEqual({ ok: false, error: 'offline' });
    expect(toast).toHaveBeenCalledWith('offline', 'error');
  });
});
