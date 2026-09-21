/**
 * A recording in progress or stopped: its status, the live view to act in, the
 * steps so far, and once stopped the fields to save it under.
 */
'use client';

import LiveView from '../live-view';
import { RECORD_WARN_MINUTES } from './constants';
import { describeStep, frameAge, withScheme } from './format';
import type { RecordForm } from './use-record-form';
import type { useRecorder } from './use-recorder';

/** The session and its form. */
interface Props {
  /** The recording session. */
  session: ReturnType<typeof useRecorder>;
  /** The save form. */
  form: RecordForm;
}

/** Whether so few minutes are left that the person should be told. */
const endingSoon = (minutes?: number) => minutes !== undefined && minutes <= RECORD_WARN_MINUTES;

/** Props for the status line. */
interface StatusProps {
  /** Capture is running. */
  recording: boolean;
  /** Steps so far. */
  count: number;
}

/** Recording or stopped, and how many steps. */
function StatusLine({ recording, count }: StatusProps) {
  return (
    <div className="flex items-center gap-2 text-[12px]">
      <span className={recording ? 'inline-flex items-center gap-1.5 text-red' : 'text-text-muted'}>
        {recording && <span className="h-2 w-2 animate-pulse rounded-full bg-red" />}
        {recording ? 'Recording' : 'Stopped'}
      </span>
      <span className="text-text-dim">·</span>
      <span className="text-text-secondary">
        {count} step{count === 1 ? '' : 's'}
      </span>
    </div>
  );
}

/** Said in the last minutes of a recording, which the server stops on its own: it used to stop without a word. */
function EndingSoon({ minutes }: { /** Whole minutes left. */ minutes: number }) {
  return (
    <p className="text-[12px] text-yellow" role="status">
      This recording stops on its own in {minutes} minute{minutes === 1 ? '' : 's'}. Stop and save it, then record the
      rest.
    </p>
  );
}

/** Where the flow starts: typed here rather than clicked, and recorded as the first step. */
function AddressBar({ session, form }: Props) {
  const busy = !!session.r.busy;
  const go = async () => {
    const target = form.url.trim();
    if (!target || busy) return;
    await session.send('navigate', { url: withScheme(target) });
  };
  return (
    <form
      className="flex gap-2"
      onSubmit={(e) => {
        e.preventDefault();
        go();
      }}
    >
      <input
        className="field flex-1 font-mono text-[12px]"
        value={form.url}
        onChange={(e) => form.setUrl(e.target.value)}
        placeholder="Go to a page, example.com/login"
        aria-label="Address"
        spellCheck={false}
        autoComplete="off"
        disabled={busy}
      />
      <button className="btn-ghost" type="submit" disabled={busy || !form.url.trim()}>
        Go
      </button>
    </form>
  );
}

/** The steps so far, numbered. */
function StepList({ steps }: Pick<Props['session'], 'steps'>) {
  return (
    <div className="max-h-40 overflow-auto rounded-lg border border-border bg-bg-card px-3 py-2 font-mono text-[11px] text-text-secondary">
      {steps.length ? (
        steps.map((s, i) => (
          <div key={i} className="truncate">
            {i + 1}. {describeStep(s)}
          </div>
        ))
      ) : (
        <span className="text-text-dim">Nothing yet, click and type in the view above.</span>
      )}
    </div>
  );
}

/** Name and description for the playbook. */
function SaveFields({ form }: Pick<Props, 'form'>) {
  return (
    <>
      <div>
        <label className="label" htmlFor="rec-name">
          Name
        </label>
        <input
          id="rec-name"
          className="field font-mono"
          value={form.name}
          autoComplete="off"
          placeholder="portal-login"
          onChange={(e) => form.setName(e.target.value)}
        />
      </div>
      <div>
        <label className="label" htmlFor="rec-desc">
          What does this flow do?
        </label>
        <input
          id="rec-desc"
          className="field"
          value={form.description}
          autoComplete="off"
          placeholder="Log into the portal and open the eligibility screen"
          onChange={(e) => form.setDescription(e.target.value)}
        />
        <p className="mt-1 text-[12px] text-text-muted">
          Used to finish the job if a replay breaks. Every value you typed, picked or clicked is already a variable.
        </p>
      </div>
    </>
  );
}

/** The session body. */
export default function RecordSession({ session, form }: Props) {
  const { recording, steps, frames, send, r } = session;
  return (
    <>
      <StatusLine recording={recording} count={steps.length} />
      {recording && endingSoon(session.r.state?.minutesLeft) && (
        <EndingSoon minutes={session.r.state?.minutesLeft ?? 0} />
      )}
      {recording && (
        <>
          <AddressBar session={session} form={form} />
          <LiveView
            frameSrc={frames.frame}
            fps={frames.fps}
            frameAgeMs={frameAge(frames.frameAt)}
            send={send}
            interactive={!r.busy}
          />
        </>
      )}
      <StepList steps={steps} />
      {!recording && <SaveFields form={form} />}
    </>
  );
}
