/**
 * What the recording dialog's buttons do: start and stop capture, save the
 * steps as a playbook, and close (handing control back first).
 */
import { errorMessage } from '@/lib/api-client';
import { acquireControl, recordCall, savePlaybook, STOP_AND_RESUME } from './api';
import { CONTROL_BUSY, RECORD_ERRORS } from './constants';
import type { Recorder } from './recorder';
import type { CodedError, RecordBusy, RecordedStep, RecordState } from './types';

/** Handles a failed request. */
type OnError = (err: unknown) => void | Promise<void>;

/** Runs `work` with `label` marked busy; a failure goes to `onError`. Resolves to whether it succeeded. */
async function busyWhile(r: Recorder, label: RecordBusy, work: () => Promise<void>, onError?: OnError) {
  r.setBusy(label);
  const report = onError ?? ((err: unknown) => r.toast(errorMessage(err), 'error'));
  const ok = await work().then(
    () => true,
    async (err) => (await report(err), false),
  );
  r.setBusy(null);
  return ok;
}

/** Takes control of the browser; input is refused unless a person holds it. */
async function takeHold(r: Recorder, force: boolean) {
  await acquireControl(r.apiKey, r.browserId, force);
  r.acquired.current = true;
}

/** The dialog closed mid-request: stop capture and hand control back. */
async function releaseUnmounted(r: Recorder) {
  await recordCall(r.apiKey, r.browserId, STOP_AND_RESUME);
  r.acquired.current = false;
}

/** Starts or stops capture, handing control back on stop. */
async function recordStep(r: Recorder, mode: 'start' | 'stop', force: boolean) {
  if (mode === 'start') await takeHold(r, force);
  if (!r.mounted.current) return releaseUnmounted(r);
  const next = await recordCall<RecordState>(r.apiKey, r.browserId, { mode, resume: mode === 'stop' });
  if (mode === 'stop') r.acquired.current = false;
  if (r.mounted.current) r.setState(next);
}

/** A start or stop failed: release a hold just taken, and say why in plain words. */
async function recordFailed(r: Recorder, mode: 'start' | 'stop', err: unknown) {
  if (mode === 'start' && r.acquired.current) {
    await recordCall(r.apiKey, r.browserId, STOP_AND_RESUME)
      .then(() => void (r.acquired.current = false))
      .catch(() => undefined);
  }
  const code = (err as CodedError).body?.code;
  r.setHeld(code === CONTROL_BUSY);
  r.toast(code && Object.hasOwn(RECORD_ERRORS, code) ? RECORD_ERRORS[code] : errorMessage(err), 'error');
}

/** Starts or stops recording; `force` takes the browser from whoever holds it. */
export async function record(r: Recorder, mode: 'start' | 'stop', force = false) {
  r.revision.current++;
  await busyWhile(
    r,
    mode,
    () => recordStep(r, mode, force),
    (err) => recordFailed(r, mode, err),
  );
}

/** What the save form holds. */
export interface SaveForm {
  /** The playbook's name. */
  name: string;
  /** What the flow does. */
  description: string;
  /** The recorded steps. */
  steps: RecordedStep[];
}

/** The save request: the form's trimmed text, the steps, and the secrets the recording marked. */
function saveBody(r: Recorder, form: SaveForm) {
  return {
    name: form.name.trim(),
    prompt: form.description.trim(),
    steps: form.steps,
    secrets: r.state?.secrets || [],
  };
}

/** Saves the steps as a playbook, then discards the server's copy of the recording. */
async function persist(r: Recorder, form: SaveForm, done: () => void) {
  const saved = await savePlaybook(r.apiKey, r.browserId, saveBody(r, form));
  await recordCall(r.apiKey, r.browserId, { mode: 'discard' });
  r.toast(`${saved.name} saved, ${saved.steps} steps`, 'success');
  done();
}

/** Saves the recording as a playbook; `done` runs on success. */
export async function save(r: Recorder, form: SaveForm, done: () => void) {
  await busyWhile(r, 'save', () => persist(r, form, done));
}

/** Stops capture and hands control back, keeping the final steps. */
async function stopFinal(r: Recorder) {
  r.revision.current++;
  const final = await recordCall<RecordState>(r.apiKey, r.browserId, STOP_AND_RESUME);
  r.acquired.current = false;
  r.setState(final);
}

/** Closes the dialog, stopping and releasing first; a failed stop keeps it open. */
export async function closeRecorder(r: Recorder, recording: boolean, onClose: () => void) {
  if (r.busy) return;
  if ((recording || r.acquired.current) && !(await busyWhile(r, 'stop', () => stopFinal(r)))) return;
  onClose();
}
