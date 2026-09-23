/**
 * The monitor beside onboarding's steps: the step in view, as a looping clip
 * of the real desktop app, in window chrome with a lit edge.
 */
'use client';

import type { Step } from './use-onboarding';
import styles from './onboarding.module.css';

/** A clip of one step. */
interface Clip {
  /** The looping video. */
  src: string;
  /** Its first look, shown before it plays. */
  poster: string;
  /** What it shows, also its accessible name. */
  caption: string;
  /** What the window's title bar says. */
  title: string;
  /** Taller than wide (the Ask panel alone), so sized by height. */
  portrait: boolean;
}

/** Each step's clip, recorded in the desktop app. */
const CLIPS: Record<Step, Clip> = {
  desktop: {
    src: '/onboarding-ask-demo.mp4',
    poster: '/onboarding-ask-demo.jpg',
    caption: 'Ask, in the desktop browser’s side panel, doing a task on the page.',
    title: 'oya browser',
    portrait: false,
  },
  model: {
    src: '/onboarding-ask-key.mp4',
    poster: '/onboarding-ask-key.jpg',
    caption: 'No key yet? Ask asks for one the first time, then answers.',
    title: 'oya browser · ask',
    portrait: true,
  },
};

/** The macOS window buttons, for the monitor's chrome. */
const LIGHTS = ['#ff5f57', '#febc2e', '#28c840'];

/** The window's title bar: traffic lights, the title, and a live marker. */
function TitleBar({ title }: { /** What it says. */ title: string }) {
  return (
    <div className="flex items-center gap-3 border-b border-border px-4 py-2.5">
      <span className="flex gap-1.5">
        {LIGHTS.map((c) => (
          <span key={c} className="h-2.5 w-2.5 rounded-full opacity-80" style={{ background: c }} />
        ))}
      </span>
      <span className="flex-1 text-center font-mono text-[11px] text-text-dim">{title}</span>
      <span className="flex items-center gap-1.5 font-mono text-[10px] uppercase tracking-[0.16em] text-accent">
        <span className="h-1.5 w-1.5 rounded-full bg-accent" />
        Live
      </span>
    </div>
  );
}

/** The clip for `step`. */
export function StepPreview({ step }: { /** The step in view. */ step: Step }) {
  const clip = CLIPS[step];
  return (
    <figure className="lg:sticky lg:top-10">
      <div className={styles.monitor}>
        <TitleBar title={clip.title} />
        <div className={styles.screen}>
          <video
            key={clip.src}
            className={clip.portrait ? 'my-5 h-[380px] w-auto rounded-lg border border-border' : 'w-full'}
            src={clip.src}
            poster={clip.poster}
            autoPlay
            muted
            loop
            playsInline
            preload="metadata"
            aria-label={clip.caption}
          />
        </div>
      </div>
      <figcaption className="mt-4 flex gap-3 text-[13px] text-text-muted">
        <span className="font-mono text-accent">{step === 'desktop' ? '01' : '02'}</span>
        {clip.caption}
      </figcaption>
    </figure>
  );
}
