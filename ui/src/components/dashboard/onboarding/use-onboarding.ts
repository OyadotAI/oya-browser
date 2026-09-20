/**
 * Onboarding's state: the chosen profile and provider, the credentials typed
 * for it, what the desktop has saved so far, and the pair and finish actions.
 */
import { useState } from 'react';
import { saveConfig, type KeyConfig } from '../config';
import { useDesktopSignIn } from '../hooks/use-desktop-sign-in';
import { useBusyAction } from '../hooks/use-busy-action';
import type { BrowserRow, Persona } from '../types';

/** What onboarding is given. */
export interface OnboardingInput {
  /** The key being set up. */
  apiKey: string;
  /** Its settings. */
  config: KeyConfig;
  /** Its personas. */
  personas: Persona[];
  /** Its browsers, to see whether a desktop has connected. */
  browsers: BrowserRow[];
  /** Setup is saved: open the console. */
  onDone: () => void;
}

/** Everything the onboarding page shows and does. */
export function useOnboarding(input: OnboardingInput) {
  const form = useOnboardingForm(input.config);
  const pairing = useDesktopSignIn(input.apiKey);
  const saving = useFinish(input, form.provider, form.credentials);
  const busy = busyWith(pairing.busy, saving.busy);
  const pair = () => pairing.open(form.profileId);
  return { ...form, ...derived(input, form.profileId, form.provider), busy, pair, finish: saving.finish };
}

/** The choices: profile, provider, and credentials typed for it. */
function useOnboardingForm(config: KeyConfig) {
  const [profileId, setProfileId] = useState('default');
  const [provider, setProvider] = useState(config.browser_provider || 'oya-cloud');
  const [credentials, setCredentials] = useState<Record<string, string>>({});
  return { profileId, setProfileId, provider, setProvider, credentials, setCredentials };
}

/** Which action is running, so only its button shows a spinner. */
const busyWith = (pairing: boolean, saving: boolean) => (pairing ? 'pair' : saving ? 'save' : null);

/** What follows from the choices: the profile, its desktop, its saved sites, and what the provider needs. */
function derived({ personas, browsers, config }: OnboardingInput, profileId: string, provider: string) {
  const profile = personas.find((p) => (profileId === 'default' ? p.isDefault : p.id === profileId));
  const desktop = browsers.find((b) => b.provider === 'oya-desktop' && b.persona === profile?.id);
  const chosen = config.providers.find((p) => p.id === provider);
  return { desktop, sites: profile?.login?.sites || [], needs: chosen?.needs || [], configured: !!chosen?.configured };
}

/** Saves the provider and its credentials, marks the key onboarded, then calls `onDone`. */
function useFinish({ apiKey, onDone }: OnboardingInput, provider: string, credentials: Record<string, string>) {
  const { busy, run } = useBusyAction();
  const finish = () =>
    run(async () => {
      await saveConfig(apiKey, { ...credentials, browser_provider: provider, onboarded: 'true' });
      onDone();
    });
  return { busy, finish };
}
