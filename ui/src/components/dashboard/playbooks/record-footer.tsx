/**
 * The recording dialog's buttons: close, start over, and the one next step —
 * start, stop or save.
 */
'use client';

import { record, save } from './record-actions';
import { isPlaybookName } from './format';
import type { RecordForm } from './use-record-form';
import type { useRecorder } from './use-recorder';

/** Props for the footer. */
interface Props {
  /** The recording session. */
  session: ReturnType<typeof useRecorder>;
  /** The save form. */
  form: RecordForm;
  /** Closes, stopping first. */
  onClose: () => void;
  /** Runs after a save. */
  onDone: () => void;
}

/** The primary button for where the recording is. */
function NextStep({ session, form, onDone }: Omit<Props, 'onClose'>) {
  const { r, recording, steps } = session;
  if (!r.state)
    return (
      <button
        className="btn-primary"
        onClick={() => record(r, 'start', r.held)}
        disabled={!r.browserId || r.busy === 'start'}
      >
        {r.busy === 'start' ? 'Starting…' : r.held ? 'Take over and record' : 'Start recording'}
      </button>
    );
  if (recording)
    return (
      <button className="btn-primary" onClick={() => record(r, 'stop')} disabled={r.busy === 'stop'}>
        {r.busy === 'stop' ? 'Stopping…' : 'Stop'}
      </button>
    );
  const ready = isPlaybookName(form.name.trim()) && form.description.trim() && steps.length;
  return (
    <button
      className="btn-primary"
      onClick={() => save(r, { ...form, steps }, onDone)}
      disabled={!ready || r.busy === 'save'}
    >
      {r.busy === 'save' ? 'Saving…' : 'Save playbook'}
    </button>
  );
}

/** Close, "Start new recording" once stopped, and the next step. */
export default function RecordFooter({ session, form, onClose, onDone }: Props) {
  const { r, recording } = session;
  return (
    <>
      <button className="btn-ghost" onClick={onClose} disabled={!!r.busy}>
        Close
      </button>
      {r.state && !recording && (
        <button className="btn-ghost" onClick={() => record(r, 'start')} disabled={!!r.busy}>
          Start new recording
        </button>
      )}
      <NextStep session={session} form={form} onDone={onDone} />
    </>
  );
}
