/**
 * A recording session on one browser: its state, the effects that keep it in
 * step with the server, and the input channel the live view drives.
 */
import { useCallback, useEffect, useRef } from 'react';
import { acquireControl, renewControl } from './api';
import { CONTROL_RENEW_MS, HOLD_LOST } from './constants';
import { useFrameStream } from './use-frame-stream';
import { useRecorderState, type Recorder } from './recorder';
import { pollStatus, restoreStopped, sendAction, whenLive } from './record-status';

/** Picks up a stopped recording left on this browser, unless a start or stop overtook the check. */
function useRestoreStopped({ browserId, apiKey, revision, setState }: Recorder) {
  useEffect(() => {
    const version = revision.current;
    return whenLive(restoreStopped(apiKey, browserId), (saved) => {
      if (saved && version === revision.current) setState(saved);
    });
  }, [browserId, apiKey, revision, setState]);
}

/** Never overlap status requests or let a response resurrect a stopped flow. */
function useStatusPoll({ apiKey, browserId, busy, setState, toast }: Recorder, recording: boolean) {
  useEffect(() => {
    if (!recording || busy) return;
    return pollStatus({ apiKey, browserId }, { state: setState, error: (m) => toast(m, 'error') });
  }, [recording, busy, browserId, apiKey, toast, setState]);
}

/** Extends the hold; one that lapsed meanwhile is taken again, and a browser someone else now has is said out loud. */
async function keepHold({ apiKey, browserId, toast }: Recorder) {
  try {
    await renewControl(apiKey, browserId).catch(() => acquireControl(apiKey, browserId));
  } catch {
    toast(HOLD_LOST, 'error');
  }
}

/** Recording renews the person's hold, which would otherwise expire mid-flow. */
function useControlRenewal(r: Recorder, recording: boolean) {
  const session = useRef(r);
  useEffect(() => void (session.current = r));
  useEffect(() => {
    if (!recording) return;
    const timer = setInterval(() => void keepHold(session.current), CONTROL_RENEW_MS);
    return () => clearInterval(timer);
  }, [recording, r.browserId]);
}

/** Sends one action to the browser; a refusal is toasted and returned. */
function useSend({ apiKey, browserId, toast }: Recorder) {
  return useCallback(
    (action: string, params: Record<string, unknown> = {}) => sendAction({ apiKey, browserId }, toast, action, params),
    [browserId, apiKey, toast],
  );
}

/** The session, its live frames and its input channel. */
export function useRecorder(apiKey: string, firstBrowser: string) {
  const r = useRecorderState(apiKey, firstBrowser);
  const recording = !!r.state?.recording;
  useRestoreStopped(r);
  const frames = useFrameStream(!!r.state, r.browserId, apiKey);
  useStatusPoll(r, recording);
  useControlRenewal(r, recording);
  return { r, recording, steps: r.state?.steps || [], frames, send: useSend(r) };
}
