/**
 * Record a flow by doing it. The page watches what the person does in the live
 * view and reports it as steps, so coordinate clicks come back as elements — the
 * same shape an ask() run leaves behind, saved as a playbook by the same route.
 */
'use client';

import Dialog from '@/components/ui/dialog';
import type { BrowserRow } from '../types';
import { closeRecorder } from './record-actions';
import RecordFooter from './record-footer';
import RecordSession from './record-session';
import RecordSetup from './record-setup';
import { useRecordForm } from './use-record-form';
import { useRecorder } from './use-recorder';

/** Props for the recording dialog. */
interface Props {
  /** The caller's API key. */
  apiKey: string;
  /** Running browsers; the first is picked. */
  browsers: BrowserRow[];
  /** Closes the dialog. */
  onClose: () => void;
  /** Reloads the playbook list after a save. */
  onSaved: () => void;
}

/** Pick a browser, record, stop, name it, save. */
export default function RecordDialog({ apiKey, browsers, onClose, onSaved }: Props) {
  const session = useRecorder(apiKey, browsers[0]?.id || '');
  const form = useRecordForm();
  const close = () => closeRecorder(session.r, session.recording, onClose);
  const done = () => {
    onSaved();
    onClose();
  };
  return (
    <Dialog
      open
      onClose={() => {
        void close();
      }}
      size="lg"
      title="Record a flow"
      description="Do the task yourself in the live view. What you click and type becomes a playbook of named fields, and a Playwright module."
      footer={<RecordFooter session={session} form={form} onClose={close} onDone={done} />}
    >
      <div className="flex flex-col gap-4">
        {!session.r.state ? (
          <RecordSetup r={session.r} browsers={browsers} />
        ) : (
          <RecordSession session={session} form={form} />
        )}
      </div>
    </Dialog>
  );
}
