/**
 * Onboarding's two steps, hung on a rail: connect the desktop, and give Ask an
 * AI model. A done step's node fills and the rail below it lights; only the
 * next thing to do gets the bright button. Pointing at a step shows its clip.
 */
'use client';

import type { ReactNode } from 'react';
import { ArrowRight, Check, Download, Loader2, Monitor } from 'lucide-react';
import type { Step, useOnboarding } from './use-onboarding';
import DesktopNotOpened from '../desktop-not-opened';
import styles from './onboarding.module.css';

/** The onboarding state every step reads. */
export type OnboardingState = ReturnType<typeof useOnboarding>;

/** What a step shows. */
interface StepProps {
  /** The onboarding state, for which step the preview shows. */
  s: OnboardingState;
  /** Which step this is. */
  id: Step;
  /** Its number, as shown. */
  n: number;
  /** Title. */
  title: string;
  /** One line under the title. */
  hint: string;
  /** Done: the node fills and `status` shows. */
  done: boolean;
  /** Said once done. */
  status: string;
  /** Said while it waits on the person. */
  waiting: string;
  /** The step's controls. */
  children: ReactNode;
}

/** Done or waiting, as a small mono status with a dot. */
function Status({ done, text }: { /** Done. */ done: boolean; /** What to say. */ text: string }) {
  return (
    <span
      className={`inline-flex items-center gap-1.5 font-mono text-[10.5px] uppercase tracking-[0.14em] ${done ? 'text-accent' : 'text-text-muted'}`}
    >
      <span className={`h-1.5 w-1.5 rounded-full ${done ? 'bg-accent' : `bg-text-muted ${styles.pulse}`}`} />
      {text}
    </span>
  );
}

/** One step on the rail: its node, label, title, hint and status, then its controls. */
function RailStep({ s, id, n, title, hint, done, status, waiting, children }: StepProps) {
  const node = done ? styles.nodeDone : s.step === id ? styles.nodeActive : '';
  return (
    <li
      className={`${styles.step} ${done ? styles.stepDone : ''}`}
      onMouseEnter={() => s.setFocused(id)}
      onFocus={() => s.setFocused(id)}
    >
      <span className={`${styles.node} ${node}`}>{done ? <Check className="h-4 w-4" strokeWidth={3} /> : n}</span>
      <div className="flex items-center justify-between gap-3">
        <span className="font-mono text-[10.5px] uppercase tracking-[0.18em] text-text-dim">Step 0{n}</span>
        <Status done={done} text={done ? status : waiting} />
      </div>
      <h2 className="mt-1.5 text-lg font-semibold tracking-tight text-text">{title}</h2>
      <p className="mt-1 text-[13.5px] leading-relaxed text-text-muted">{hint}</p>
      <div className="mt-4">{children}</div>
    </li>
  );
}

/** Step 1: open (or download) the desktop browser, signed in to this project. */
export function ConnectStep({ s }: { /** State. */ s: OnboardingState }) {
  const done = !!s.desktop;
  const hint = 'Ask lives in its side panel. One click signs it in to this project.';
  return (
    <RailStep
      s={s}
      id="desktop"
      n={1}
      title="Connect the desktop browser"
      hint={hint}
      done={done}
      status="Connected"
      waiting="Waiting"
    >
      <div className="flex flex-wrap gap-2">
        <button className={done ? 'btn-ghost' : 'btn-primary'} onClick={s.pair} disabled={!!s.busy}>
          {s.busy === 'pair' ? <Loader2 className="h-4 w-4 animate-spin" /> : <Monitor className="h-4 w-4" />}
          {done ? 'Open desktop' : 'Connect desktop'}
        </button>
        {!done && (
          <a className="btn-ghost" href="/downloads" target="_blank" rel="noreferrer">
            <Download className="h-4 w-4" />
            Download
          </a>
        )}
      </div>
      {!done && s.notOpened && <DesktopNotOpened />}
    </RailStep>
  );
}

/** A segmented choice of AI provider. */
function ProviderChoice({ s }: { /** State. */ s: OnboardingState }) {
  return (
    <div
      role="group"
      aria-label="AI provider"
      className="grid grid-cols-4 rounded-lg border border-border bg-bg-sunken p-1"
    >
      {s.providers.map((p) => (
        <button
          key={p.id}
          type="button"
          aria-pressed={s.provider === p.id}
          onClick={() => s.setProvider(p.id)}
          className={`rounded-md py-2 text-[13px] font-medium transition-colors ${s.provider === p.id ? 'bg-bg-elevated text-text shadow-[inset_0_0_0_1px_var(--color-border)]' : 'text-text-muted hover:text-text'}`}
        >
          {p.label}
        </button>
      ))}
    </div>
  );
}

/** Step 2: the AI model Ask runs on, as a provider and its key. */
export function ModelStep({ s }: { /** State. */ s: OnboardingState }) {
  const preset = s.providers.find((p) => p.id === s.provider);
  const hint = 'Ask needs one to think. Your key stays in this project.';
  return (
    <RailStep
      s={s}
      id="model"
      n={2}
      title="Connect an AI model"
      hint={hint}
      done={s.hasModel}
      status="Ready"
      waiting="Needs a key"
    >
      <ProviderChoice s={s} />
      <label htmlFor="setup-model-key" className="sr-only">
        API key
      </label>
      <input
        id="setup-model-key"
        className="field mt-2.5 font-mono text-[13px]"
        type="password"
        autoComplete="off"
        value={s.key}
        placeholder={
          s.hasModel ? 'Paste a new key to replace the saved one' : `${preset?.label} API key (${preset?.hint})`
        }
        onChange={(e) => s.setKey(e.target.value)}
      />
    </RailStep>
  );
}

/** The finish row: the bright button only once there is a key to save or everything is ready. */
export function Finish({ s }: { /** State. */ s: OnboardingState }) {
  const ready = !!s.key.trim() || (!!s.desktop && s.hasModel);
  return (
    <div className="mt-10 flex items-center justify-between gap-4 border-t border-border pt-6">
      <p className="text-xs text-text-muted">You can change any of this later in Settings.</p>
      <button className={ready ? 'btn-primary' : 'btn-ghost'} onClick={s.finish} disabled={!!s.busy}>
        {s.busy === 'save' && <Loader2 className="h-4 w-4 animate-spin" />}
        {s.key.trim() ? 'Save and continue' : ready ? 'Open console' : 'Skip for now'}
        {ready && s.busy !== 'save' && <ArrowRight className="h-4 w-4" />}
      </button>
    </div>
  );
}
