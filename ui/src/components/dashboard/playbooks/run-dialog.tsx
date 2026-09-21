/**
 * Run a playbook on a live browser and follow it, answering it if it stops for
 * a person.
 */
'use client';

import Dialog from '@/components/ui/dialog';
import type { BrowserRow, Persona } from '../types';
import RunForm from './run-form';
import RunStatusView from './run-status';
import { useRunDialog, type RunDialogState } from './use-run';
import type { PlaybookInfo } from './types';

/** Props for the run dialog. */
interface Props {
  /** The caller's API key. */
  apiKey: string;
  /** The playbook to run. */
  playbook: PlaybookInfo;
  /** Running browsers. */
  browsers: BrowserRow[];
  /** Profiles to filter them by. */
  personas: Persona[];
  /** Closes the dialog. */
  onClose: () => void;
  /** Reloads the list once the run ends (it may have left a draft). */
  onFinished: () => void;
}

/** Props for the footer. */
interface FooterProps {
  /** The dialog's state. */
  run: RunDialogState;
  /** Closes the dialog. */
  onClose: () => void;
}

/** Cancel and Run (or start a browser and run); only Close once a run started. */
function RunFooter({ run, onClose }: FooterProps) {
  const { runner, target } = run;
  if (runner.run)
    return (
      <button className="btn-ghost" onClick={onClose}>
        Close
      </button>
    );
  return (
    <>
      <button className="btn-ghost" onClick={onClose}>
        Cancel
      </button>
      {target.matching.length ? (
        <button
          className="btn-primary"
          onClick={() => runner.start(target.browserId)}
          disabled={!target.browserId || runner.starting}
        >
          {runner.starting ? 'Starting…' : 'Run'}
        </button>
      ) : (
        <button className="btn-primary" onClick={run.startAndRun} disabled={!target.persona || runner.starting}>
          {runner.starting ? 'Starting a browser…' : 'Start one and run'}
        </button>
      )}
    </>
  );
}

/** The form until a run starts, then its status. */
export default function RunDialog({ apiKey, playbook, browsers, personas, onClose, onFinished }: Props) {
  const run = useRunDialog(apiKey, playbook, browsers, onFinished);
  const started = run.runner.run;
  return (
    <Dialog
      open
      onClose={onClose}
      title={`Run ${playbook.name}`}
      description="Replays on a running browser without the LLM."
      footer={<RunFooter run={run} onClose={onClose} />}
    >
      {!started ? (
        <RunForm run={run} playbook={playbook} personas={personas} />
      ) : (
        <RunStatusView run={started} reply={run.reply} />
      )}
    </Dialog>
  );
}
