/**
 * Unit tests for the run dialog's steps: following a run, waiting for a
 * started browser, and answering a paused run.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));

import { api } from '@/lib/api-client';
import { CONNECT_TIMEOUT_MS, RUN_POLL_MS } from '@/components/dashboard/playbooks/constants';
import {
  answerRun,
  followRun,
  keepInFilter,
  launchRun,
  settlePending,
  startPending,
} from '@/components/dashboard/playbooks/run-steps';
import type { BrowserRow } from '@/components/dashboard/types';
import type { RunInfo } from '@/components/dashboard/playbooks/types';

const apiMock = vi.mocked(api);
const run = (status: RunInfo['status']): RunInfo => ({ id: 'r1', status, attention: null });
const browsers = [{ id: 'a' }, { id: 'b' }] as BrowserRow[];

describe('followRun', () => {
  beforeEach(() => {
    apiMock.mockReset();
    vi.useFakeTimers();
  });
  afterEach(() => vi.useRealTimers());

  it('polls the run and reports when it ends', async () => {
    apiMock.mockResolvedValueOnce(run('running')).mockResolvedValueOnce(run('succeeded'));
    const setRun = vi.fn();
    const onFinished = vi.fn();
    const stop = followRun('k', 'r1', setRun, onFinished);
    await vi.advanceTimersByTimeAsync(RUN_POLL_MS);
    expect(onFinished).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(RUN_POLL_MS);
    expect(setRun).toHaveBeenLastCalledWith(run('succeeded'));
    expect(onFinished).toHaveBeenCalledTimes(1);
    expect(apiMock).toHaveBeenCalledWith('/runs/r1', { key: 'k' });
    stop();
  });

  it('keeps following through a failed poll', async () => {
    apiMock.mockRejectedValueOnce(new Error('blip')).mockResolvedValueOnce(run('running'));
    const setRun = vi.fn();
    const stop = followRun('k', 'r1', setRun, vi.fn());
    await vi.advanceTimersByTimeAsync(RUN_POLL_MS * 2);
    expect(setRun).toHaveBeenCalledTimes(1);
    stop();
  });
});

describe('launchRun', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });

  it('posts the run and shows it', async () => {
    apiMock.mockResolvedValueOnce(run('running'));
    const sink = { setRun: vi.fn(), setStarting: vi.fn(), toast: vi.fn() };
    await launchRun('k', 'b1', { playbook: 'p', data: { a: '1' }, autoHeal: true }, sink);
    expect(apiMock).toHaveBeenCalledWith('/browsers/b1/runs', {
      key: 'k',
      method: 'POST',
      body: { playbook: 'p', data: { a: '1' }, autoHeal: true },
    });
    expect(sink.setRun).toHaveBeenCalledWith(run('running'));
    expect(sink.setStarting.mock.calls).toEqual([[true], [false]]);
  });

  it('toasts a refused run', async () => {
    apiMock.mockRejectedValueOnce(new Error('no such playbook'));
    const sink = { setRun: vi.fn(), setStarting: vi.fn(), toast: vi.fn() };
    await launchRun('k', 'b1', { playbook: 'p', data: {}, autoHeal: false }, sink);
    expect(sink.toast).toHaveBeenCalledWith('no such playbook', 'error');
    expect(sink.setStarting).toHaveBeenLastCalledWith(false);
  });
});

describe('waiting for a started browser', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });
  afterEach(() => vi.useRealTimers());

  it('starts a browser on the profile and waits up to the connect timeout', async () => {
    vi.useFakeTimers({ now: 1000 });
    apiMock.mockResolvedValueOnce({ id: 'new' });
    const sink = { setStarting: vi.fn(), setPending: vi.fn(), toast: vi.fn() };
    await startPending('k', 'p1', sink);
    expect(apiMock).toHaveBeenCalledWith('/browsers/start', { key: 'k', method: 'POST', body: { persona: 'p1' } });
    expect(sink.setPending).toHaveBeenCalledWith({ id: 'new', until: 1000 + CONNECT_TIMEOUT_MS });
    expect(sink.setStarting).toHaveBeenCalledTimes(1);
  });

  it('a refused start is toasted and ends the start', async () => {
    apiMock.mockRejectedValueOnce(new Error('cap reached'));
    const sink = { setStarting: vi.fn(), setPending: vi.fn(), toast: vi.fn() };
    await startPending('k', 'p1', sink);
    expect(sink.toast).toHaveBeenCalledWith('cap reached', 'error');
    expect(sink.setStarting).toHaveBeenLastCalledWith(false);
  });

  it('runs on the browser once it shows up in the fleet', () => {
    const out = { clear: vi.fn(), start: vi.fn(), timeout: vi.fn() };
    settlePending({ id: 'b', until: Date.now() + 1 }, browsers, out);
    expect(out.clear).toHaveBeenCalled();
    expect(out.start).toHaveBeenCalledWith('b');
  });

  it('gives up after the deadline, and keeps waiting before it', () => {
    const out = { clear: vi.fn(), start: vi.fn(), timeout: vi.fn() };
    settlePending({ id: 'z', until: Date.now() + CONNECT_TIMEOUT_MS }, browsers, out);
    expect(out.clear).not.toHaveBeenCalled();
    settlePending({ id: 'z', until: Date.now() - 1 }, browsers, out);
    expect(out.clear).toHaveBeenCalled();
    expect(out.timeout).toHaveBeenCalled();
    expect(out.start).not.toHaveBeenCalled();
  });
});

describe('keepInFilter', () => {
  it('keeps a choice inside the filter and falls back to the first browser, or none', () => {
    const set = vi.fn();
    keepInFilter(browsers, 'b', set);
    expect(set).not.toHaveBeenCalled();
    keepInFilter(browsers, 'x', set);
    keepInFilter([], 'x', set);
    expect(set.mock.calls).toEqual([['a'], ['']]);
  });
});

describe('answerRun', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });

  it('sends the trimmed reply, or "done", and resumes the run', async () => {
    apiMock.mockResolvedValue({});
    const sink = { setReply: vi.fn(), setRun: vi.fn(), toast: vi.fn() };
    const paused = { ...run('needs_attention'), attention: { id: 'a', reason: 'mfa' as const, message: 'code?' } };
    await answerRun('k', paused, '  ', sink);
    expect(apiMock).toHaveBeenCalledWith('/runs/r1/respond', { key: 'k', method: 'POST', body: { response: 'done' } });
    expect(sink.setReply).toHaveBeenCalledWith('');
    expect(sink.setRun).toHaveBeenCalledWith({ ...paused, status: 'running', attention: null });
  });

  it('toasts a failed reply and keeps it', async () => {
    apiMock.mockRejectedValueOnce(new Error('expired'));
    const sink = { setReply: vi.fn(), setRun: vi.fn(), toast: vi.fn() };
    await answerRun('k', run('needs_attention'), '123', sink);
    expect(sink.toast).toHaveBeenCalledWith('expired', 'error');
    expect(sink.setReply).not.toHaveBeenCalled();
  });
});
