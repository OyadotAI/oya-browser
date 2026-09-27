/**
 * The hero's product shot: a saved playbook replaying on a payer portal. The
 * form fills on the left as each line of the generated Playwright code runs on
 * the right, and the meter underneath never counts a model call. It plays only
 * while on screen, and shows the finished run to anyone who asked for reduced
 * motion.
 */
'use client';

import { useInView, useReducedMotion } from 'framer-motion';
import { Check, ChevronDown, Lock } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { SHOWCASE } from './content';
import { REVEAL_MARGIN, SHOWCASE_HOLD, SHOWCASE_TICK_MS } from './constants';
import styles from '../page.module.css';

/** The step after the last line: the run is done. */
const DONE = SHOWCASE.code.length;
/** The line that presses the button, right after the fields. */
const PRESS = SHOWCASE.fields.length + 1;
/** A string literal inside a line of code, kept by the split so it can be coloured. */
const STRING = /("[^"]*")/;

/** Which line is running: it steps while the shot is in view, and holds a moment on the finished run. */
function useReplayStep() {
  const ref = useRef<HTMLElement>(null);
  const seen = useInView(ref, { margin: REVEAL_MARGIN });
  const still = useReducedMotion();
  const [tick, setTick] = useState(0);
  useEffect(() => {
    if (!seen || still) return;
    const id = setInterval(() => setTick((t) => (t + 1) % (DONE + SHOWCASE_HOLD + 1)), SHOWCASE_TICK_MS);
    return () => clearInterval(id);
  }, [seen, still]);
  return { ref, step: still ? DONE : Math.min(tick, DONE) };
}

/** Props for each pane: the line running now. */
interface StepProps {
  /** The index of the running line, or the line count once done. */
  step: number;
}

/** The portal's form, filled up to the running line. */
function PortalPane({ step }: StepProps) {
  return (
    <div className={styles.portalPane}>
      <p className={styles.portalTitle}>{SHOWCASE.title}</p>
      {SHOWCASE.fields.map(({ label, value, select }, i) => (
        <div key={label} className={styles.field} data-active={step === i + 1}>
          <span>{label}</span>
          <span className={styles.fieldBox}>
            {step > i && (
              <span key={step === i + 1 ? 'typing' : 'typed'} className={styles.typed}>
                {value}
              </span>
            )}
            {select && <ChevronDown size={14} aria-hidden="true" />}
          </span>
        </div>
      ))}
      <span className={styles.portalButton} data-active={step === PRESS}>
        {SHOWCASE.button}
      </span>
      <p className={styles.portalResult} data-shown={step > PRESS}>
        <Check size={15} aria-hidden="true" /> {SHOWCASE.result}
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
function CodeLine({ line }: LineProps) {
  return line.split(STRING).map((part, i) => (STRING.test(part) ? <em key={i}>{part}</em> : part));
}

/** The generated playbook, the running line lit and the finished ones ticked. */
function CodePane({ step }: StepProps) {
  return (
    <div className={styles.codePane}>
      <p className={styles.codeFile}>{SHOWCASE.file}</p>
      <ol>
        {SHOWCASE.code.map((line, i) => (
          <li key={line} data-state={i < step ? 'done' : i === step ? 'running' : 'next'}>
            <span className={styles.gutter}>{i < step ? <Check size={12} aria-hidden="true" /> : i + 1}</span>
            <code>
              <CodeLine line={line} />
            </code>
          </li>
        ))}
      </ol>
      <p className={styles.codeDone} data-shown={step === DONE}>
        {SHOWCASE.done}
      </p>
    </div>
  );
}

/** What the run has cost so far: the steps, and never a model call. */
function Meter({ step }: StepProps) {
  const items = [
    ['Step', `${Math.min(step + 1, DONE)} of ${DONE}`],
    ['Model calls', '0'],
    ['Tokens', '$0.00'],
    ['Signed in as', 'your staff'],
  ];
  return (
    <dl className={styles.meter}>
      {items.map(([label, value]) => (
        <div key={label}>
          <dt>{label}</dt>
          <dd>{value}</dd>
        </div>
      ))}
    </dl>
  );
}

/** The window: its bar, the two panes, and the meter. */
export function ReplayShowcase() {
  const { ref, step } = useReplayStep();
  return (
    <figure
      ref={ref}
      className={styles.showcase}
      aria-label="A saved playbook replaying on a payer portal, with no model"
    >
      <div className={styles.windowBar}>
        <span className={styles.dots} aria-hidden="true">
          <i />
          <i />
          <i />
        </span>
        <span className={styles.address}>
          <Lock size={12} aria-hidden="true" /> {SHOWCASE.address}
        </span>
        <span className={styles.liveChip}>
          <i aria-hidden="true" /> {step < DONE ? 'Replaying' : 'Replayed'}, no model
        </span>
      </div>
      <div className={styles.showcaseBody}>
        <PortalPane step={step} />
        <CodePane step={step} />
      </div>
      <Meter step={step} />
    </figure>
  );
}
