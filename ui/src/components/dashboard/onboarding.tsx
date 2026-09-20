/**
 * First-run setup for a key: connect a desktop browser, sign in to accounts
 * on it, and choose where browsers run. Logic lives in onboarding/.
 */
'use client';

import { ArrowRight } from 'lucide-react';
import type { KeyConfig } from './config';
import type { Persona, BrowserRow } from './types';
import FirstBrowser from './onboarding/first-browser';
import { ConnectStep, ProviderStep, SignInStep } from './onboarding/steps';
import { useOnboarding } from './onboarding/use-onboarding';

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

/** The onboarding page. */
export default function Onboarding({ apiKey, config, personas, browsers, onDone }: Props) {
  const s = useOnboarding({ apiKey, config, personas, browsers, onDone });
  return (
    <div className="flex-1 overflow-y-auto">
      <main className="mx-auto max-w-6xl px-5 py-8 sm:px-8 sm:py-12">
        <div className="mb-9 flex flex-wrap items-end justify-between gap-4">
          <div>
            <p className="mb-3 font-mono text-[11px] uppercase tracking-[0.18em] text-accent">
              Your browser control plane
            </p>
            <h1 className="font-display text-3xl tracking-tight text-text sm:text-4xl">
              Sign in once. Build from there.
            </h1>
            <p className="mt-3 max-w-xl text-sm text-text-secondary">
              One desktop browser saves your accounts to a profile. Start browsers with those sessions from your code.
            </p>
          </div>
          <button className="btn-ghost" onClick={s.finish} disabled={!!s.busy}>
            Go to console <ArrowRight className="h-4 w-4" />
          </button>
        </div>
        <div className="grid gap-8 lg:grid-cols-[1fr_1.12fr] lg:gap-12">
          <div className="space-y-7">
            <ConnectStep s={s} personas={personas} />
            <SignInStep sites={s.sites} />
            <ProviderStep s={s} config={config} />
          </div>
          <FirstBrowser apiKey={apiKey} profileId={s.profileId} />
        </div>
      </main>
    </div>
  );
}
