/**
 * Unit tests for the recording dialog's actions: start, stop, save and close,
 * including the control hold they take and hand back.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));

import { api, ApiError } from '@/lib/api-client';
import { closeRecorder, record, save } from '@/components/dashboard/playbooks/record-actions';
import type { Recorder } from '@/components/dashboard/playbooks/recorder';
import type { RecordState } from '@/components/dashboard/playbooks/types';

const apiMock = vi.mocked(api);
const stopped: RecordState = { recording: false, steps: [{ action: 'click' }], secrets: ['pw'] };

/** A recorder with spies for every setter. */
function recorder(over: Partial<Recorder> = {}): Recorder {
  return {
    apiKey: 'k',
    browserId: 'b1',
    toast: vi.fn(),
    setBrowserId: vi.fn(),
    state: null,
    setState: vi.fn(),
    busy: null,
    setBusy: vi.fn(),
    held: false,
    setHeld: vi.fn(),
    acquired: { current: false },
    mounted: { current: true },
    revision: { current: 0 },
    ...over,
  };
}

/** The paths requested, in order. */
const paths = () => apiMock.mock.calls.map(([path, opts]) => `${path} ${JSON.stringify(opts?.body ?? null)}`);

describe('record', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });

  it('start takes control first, then starts capture and shows it', async () => {
    const r = recorder();
    apiMock.mockResolvedValueOnce({}).mockResolvedValueOnce({ ...stopped, recording: true });
    await record(r, 'start');
    expect(paths()).toEqual([
      '/control/sessions/b1/control {"action":"acquire","force":false}',
      '/control/sessions/b1/record {"mode":"start","resume":false}',
    ]);
    expect(r.acquired.current).toBe(true);
    expect(r.revision.current).toBe(1);
    expect(r.setState).toHaveBeenCalledWith({ ...stopped, recording: true });
    expect(vi.mocked(r.setBusy).mock.calls).toEqual([['start'], [null]]);
  });

  it('stop hands control back', async () => {
    const r = recorder({ acquired: { current: true } });
    apiMock.mockResolvedValueOnce(stopped);
    await record(r, 'stop');
    expect(paths()).toEqual(['/control/sessions/b1/record {"mode":"stop","resume":true}']);
    expect(r.acquired.current).toBe(false);
  });

  it('a dialog closed mid-start stops and releases instead of recording', async () => {
    const r = recorder({ mounted: { current: false } });
    apiMock.mockResolvedValue({});
    await record(r, 'start', true);
    expect(paths()).toEqual([
      '/control/sessions/b1/control {"action":"acquire","force":true}',
      '/control/sessions/b1/record {"mode":"stop","resume":true}',
    ]);
    expect(r.acquired.current).toBe(false);
    expect(r.setState).not.toHaveBeenCalled();
  });

  it('a browser held elsewhere offers take-over and says why', async () => {
    const r = recorder();
    apiMock.mockRejectedValueOnce(new ApiError('busy', 409, { code: 'control_busy' }));
    await record(r, 'start');
    expect(r.setHeld).toHaveBeenCalledWith(true);
    expect(r.toast).toHaveBeenCalledWith('Another tab or operator is holding this browser.', 'error');
  });

  it('a failed start after taking control releases it again', async () => {
    const r = recorder();
    apiMock
      .mockResolvedValueOnce({})
      .mockRejectedValueOnce(new ApiError('pending', 409, { code: 'commands_pending' }))
      .mockResolvedValueOnce({});
    await record(r, 'start');
    expect(paths()[2]).toBe('/control/sessions/b1/record {"mode":"stop","resume":true}');
    expect(r.acquired.current).toBe(false);
    expect(r.setHeld).toHaveBeenCalledWith(false);
    expect(r.toast).toHaveBeenCalledWith('The browser is still finishing a command. Try again in a moment.', 'error');
  });

  it('an unnamed failure shows the server reason', async () => {
    const r = recorder();
    apiMock.mockRejectedValueOnce(new Error('nope'));
    await record(r, 'stop');
    expect(r.toast).toHaveBeenCalledWith('nope', 'error');
  });
});

describe('save', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });

  it('saves trimmed fields with the secrets, discards the recording, then finishes', async () => {
    const r = recorder({ state: stopped });
    const done = vi.fn();
    apiMock.mockResolvedValueOnce({ name: 'login', steps: 1 }).mockResolvedValueOnce({});
    await save(r, { name: ' login ', description: ' Log in ', steps: stopped.steps }, done);
    expect(paths()).toEqual([
      '/browsers/b1/playbooks {"name":"login","prompt":"Log in","steps":[{"action":"click"}],"secrets":["pw"]}',
      '/control/sessions/b1/record {"mode":"discard"}',
    ]);
    expect(r.toast).toHaveBeenCalledWith('login saved — 1 steps', 'success');
    expect(done).toHaveBeenCalled();
  });

  it('a failed save keeps the dialog open and says why', async () => {
    const r = recorder({ state: stopped });
    const done = vi.fn();
    apiMock.mockRejectedValueOnce(new Error('taken'));
    await save(r, { name: 'x', description: 'y', steps: [] }, done);
    expect(done).not.toHaveBeenCalled();
    expect(r.toast).toHaveBeenCalledWith('taken', 'error');
    expect(vi.mocked(r.setBusy).mock.calls).toEqual([['save'], [null]]);
  });
});

describe('closeRecorder', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });

  it('does nothing while a request is in flight', async () => {
    const onClose = vi.fn();
    await closeRecorder(recorder({ busy: 'save' }), false, onClose);
    expect(onClose).not.toHaveBeenCalled();
  });

  it('closes straight away when nothing is held', async () => {
    const onClose = vi.fn();
    await closeRecorder(recorder(), false, onClose);
    expect(apiMock).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it('stops and releases a running recording before closing', async () => {
    const r = recorder({ acquired: { current: true } });
    const onClose = vi.fn();
    apiMock.mockResolvedValueOnce(stopped);
    await closeRecorder(r, true, onClose);
    expect(r.setState).toHaveBeenCalledWith(stopped);
    expect(r.acquired.current).toBe(false);
    expect(onClose).toHaveBeenCalled();
  });

  it('stays open when the stop fails', async () => {
    const r = recorder();
    const onClose = vi.fn();
    apiMock.mockRejectedValueOnce(new Error('offline'));
    await closeRecorder(r, true, onClose);
    expect(onClose).not.toHaveBeenCalled();
    expect(r.toast).toHaveBeenCalledWith('offline', 'error');
  });
});
