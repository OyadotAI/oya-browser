/**
 * The hero's product shot: a saved playbook replaying on a portal. The form fills on
 * the left as each line of the generated Playwright code runs on the right, and the
 * audit strip underneath writes one row per line. It plays only while on screen, can be
 * paused (a looping animation needs a way to stop it), and shows the finished run to
 * anyone who asked for reduced motion. The server renders the finished run too, so
 * nothing moves when the page hydrates.
 */
'use client';

import { useInView, useReducedMotion } from 'framer-motion';
import { Check, ChevronDown, Lock, Pause, Play } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { ReplayField, ReplayTask } from './content';
import { REVEAL_MARGIN, SHOWCASE_HOLD, SHOWCASE_TICK_MS } from './constants';
import { Ledger } from './ledger';
import styles from './replay-showcase.module.css';

/** A string literal inside a line of code, kept by the split so it can be coloured. */
const STRING = /("[^"]*")/;
/** The step before the first line runs: shown as the finished run, so the server and the first paint agree. */
const NOT_STARTED = -1;

/** The comment a playbook ends on once it has run, from its own line count. */
export const doneLine = (task: ReplayTask) =>
  `// ${task.code.length} steps replayed. No model, no tokens, nothing to review.`;

/** Which line is running: it steps while the shot is in view and not paused, and holds on the finished run. */
export function useReplay(task: ReplayTask, executionBlocked = false) {
  const ref = useRef<HTMLElement>(null);
  const seen = useInView(ref, { margin: REVEAL_MARGIN });
  const still = !!useReducedMotion();
  const [paused, setPaused] = useState(false);
  const [tick, setTick] = useState(NOT_STARTED);
  const done = task.code.length;
  useEffect(() => {
    if (!seen || still || paused || executionBlocked) return;
    const id = setInterval(() => setTick((t) => (t + 1) % (done + SHOWCASE_HOLD + 1)), SHOWCASE_TICK_MS);
    return () => clearInterval(id);
  }, [seen, still, paused, executionBlocked, done]);
  const step = still || tick === NOT_STARTED ? done : Math.min(tick, done);
  return {
    ref,
    step,
    paused,
    still,
    toggle: () => setPaused((p) => !p),
    resume: () => setPaused(false),
    restart: () => {
      setTick(NOT_STARTED);
      setPaused(false);
    },
  };
}

/** Props for each pane: the task, and the line running now. */
interface PaneProps {
  /** The task being replayed. */
  task: ReplayTask;
  /** The index of the running line, or the line count once done. */
  step: number;
}

/** Props for one field: the field, its place, and the line running now. */
interface FieldProps {
  /** The field. */
  field: ReplayField;
  /** Its index on the form. */
  i: number;
  /** The index of the running line. */
  step: number;
}

/** One field of the form, typed in once its line has run. */
function Field({ field, i, step }: FieldProps) {
  return (
    <div className={styles.field} data-active={step === i + 1}>
      <span>{field.label}</span>
      <span className={styles.fieldBox}>
        {step > i && (
          <span key={step === i + 1 ? 'typing' : 'typed'} className={styles.typed}>
            {field.value}
          </span>
        )}
        {field.select && <ChevronDown size={14} aria-hidden="true" />}
      </span>
    </div>
  );
}

/** Props for the portal pane: the form, and the line running now; the workflow tabs pass a form alone. */
interface PortalProps {
  /** The form: its title, fields, button and answer. */
  task: Pick<ReplayTask, 'title' | 'fields' | 'button' | 'result'>;
  /** The index of the running line, or past the last once done. */
  step: number;
  /** A scene can present the result beside the form rather than repeating it inside. */
  showResult?: boolean;
}

/** The portal's form, filled up to the running line. */
export function PortalPane({ task, step, showResult = true }: PortalProps) {
  const press = task.fields.length + 1;
  return (
    <div className={styles.portalPane}>
      <p className={styles.portalTitle}>{task.title}</p>
      {task.fields.map((field, i) => (
        <Field key={field.label} field={field} i={i} step={step} />
      ))}
      <span className={styles.portalButton} data-active={step === press}>
        {task.button}
      </span>
      <p className={styles.portalResult} data-shown={step > press} hidden={!showResult}>
        <Check size={15} aria-hidden="true" /> {task.result}
      </p>
    </div>
  );
}

/** Props for one line of code. */
interface LineProps {
  /** The line, as generated. */
  line: string;
}

/** One line of code with its strings picked out. */
export function CodeLine({ line }: LineProps) {
  return line.split(STRING).map((part, i) => (STRING.test(part) ? <em key={i}>{part}</em> : part));
}

/** The generated playbook, the running line lit and the finished ones ticked. */
export function CodePane({ task, step }: PaneProps) {
  return (
    <div className={styles.codePane}>
      <p className={styles.codeFile}>{task.file}</p>
      <ol>
        {task.code.map((line, i) => (
          <li key={line} data-state={i < step ? 'done' : i === step ? 'running' : 'next'}>
            <span className={styles.gutter}>{i < step ? <Check size={12} aria-hidden="true" /> : i + 1}</span>
            <code>
              <CodeLine line={line} />
            </code>
          </li>
        ))}
      </ol>
      <p className={styles.codeDone} data-shown={step === task.code.length}>
        {doneLine(task)}
      </p>
    </div>
  );
}

/** Props for the window bar: the task, the running line, and the pause state. */
interface BarProps extends PaneProps {
  /** Whether the replay is paused. */
  paused: boolean;
  /** Whether motion is off, so there is nothing to pause. */
  still: boolean;
  /** Pauses or resumes the replay. */
  toggle: () => void;
}

/** The window bar: the portal's address, the run's state, and the pause control. */
function WindowBar({ task, step, paused, still, toggle }: BarProps) {
  return (
    <div className={styles.windowBar}>
      <span className={styles.dots} aria-hidden="true">
        <i />
        <i />
        <i />
      </span>
      <span className={styles.address}>
        <Lock size={12} aria-hidden="true" /> {task.address}
      </span>
      <span className={styles.liveChip}>
        <i aria-hidden="true" /> {step < task.code.length ? 'Replaying' : 'Replayed'}, no model
      </span>
      {!still && (
        <button type="button" className={styles.pause} onClick={toggle}>
          {paused ? <Play size={12} aria-hidden="true" /> : <Pause size={12} aria-hidden="true" />}
          {paused ? 'Play replay' : 'Pause replay'}
        </button>
      )}
    </div>
  );
}

/** Props for the showcase: the task it replays. */
interface ShowcaseProps {
  /** The portal task. */
  task: ReplayTask;
}

/** The window: its bar, the two panes, and the meter. */
export function ReplayShowcase({ task }: ShowcaseProps) {
  const { ref, step, paused, still, toggle } = useReplay(task);
  return (
    <figure
      ref={ref}
      className={styles.showcase}
      aria-label={`A saved playbook replaying: ${task.title}, with no model`}
    >
      <WindowBar task={task} step={step} paused={paused} still={still} toggle={toggle} />
      <div className={styles.showcaseBody}>
        <PortalPane task={task} step={step} />
        <CodePane task={task} step={step} />
      </div>
      <Ledger task={task} step={step} />
    </figure>
  );
}
