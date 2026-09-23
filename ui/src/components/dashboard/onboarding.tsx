/**
 * First-run setup for a project, as a pre-flight checklist: connect the
 * desktop browser and give Ask an AI model, beside a clip of each step.
 * Logic lives in onboarding/.
 */
'use client';

import type { KeyConfig } from './config';
import type { Persona, BrowserRow } from './types';
import { StepPreview } from './onboarding/preview';
import { ConnectStep, Finish, ModelStep } from './onboarding/steps';
import { useOnboarding } from './onboarding/use-onboarding';
import styles from './onboarding/onboarding.module.css';

/** What the dashboard hands onboarding. */
interface Props {
  /** The key being set up. */
  apiKey: string;
  /** Its settings. */
  config: KeyConfig;
  /** Its personas. */
  personas: Persona[];
  /** Its browsers. */
  browsers: BrowserRow[];
  /** Setup is saved: open the console. */
  onDone: () => void;
}

/** "Pre-flight", and one tick per step, lit once done. */
function Progress({ done }: { /** Steps done, in order. */ done: boolean[] }) {
  return (
    <div className="flex items-center gap-3">
      <span className="font-mono text-[11px] uppercase tracking-[0.22em] text-accent">Pre-flight</span>
      <span className="flex gap-1.5" aria-label={`${done.filter(Boolean).length} of ${done.length} steps done`}>
        {done.map((on, i) => (
          <span key={i} className={`${styles.tick} ${on ? styles.tickOn : ''}`} />
        ))}
      </span>
    </div>
  );
}

/** The onboarding page. */
export default function Onboarding({ apiKey, config, personas, browsers, onDone }: Props) {
  const s = useOnboarding({ apiKey, config, personas, browsers, onDone });
  return (
    <div className="flex-1 overflow-y-auto">
      <main
        className={`${styles.stage} mx-auto grid max-w-6xl items-start gap-12 px-5 py-12 sm:px-8 lg:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)] lg:gap-16 lg:py-20`}
      >
        <div>
          <div className={styles.rise}>
            <Progress done={[!!s.desktop, s.hasModel]} />
            <h1 className="mt-5 font-display text-[40px] leading-[0.98] tracking-tight text-text sm:text-[52px]">
              Get your agent <span className="text-accent">ready.</span>
            </h1>
            <p className="mt-4 max-w-md text-[15px] leading-relaxed text-text-secondary">
              Two quick steps. Then open the desktop browser and ask it to do things on any page.
            </p>
          </div>
          <ol className={`${styles.rise} mt-12`}>
            <ConnectStep s={s} />
            <ModelStep s={s} />
          </ol>
          <div className={styles.rise}>
            <Finish s={s} />
          </div>
        </div>
        <div className={styles.rise}>
          <StepPreview step={s.step} />
        </div>
      </main>
    </div>
  );
}
