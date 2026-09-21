/**
 * Onboarding's three steps: connect the desktop, sign in to accounts on it,
 * and choose where browsers run.
 */
'use client';

import type { ReactNode } from 'react';
import { ArrowRight, Check, Download, Loader2, Monitor } from 'lucide-react';
import type { KeyConfig } from '../config';
import type { Persona } from '../types';
import type { useOnboarding } from './use-onboarding';

/** The onboarding state every step reads. */
export type OnboardingState = ReturnType<typeof useOnboarding>;

/** A numbered step heading, with anything that follows the title. */
function StepTitle({
  n,
  title,
  children,
}: {
  /** Step number. */ n: string;
  /** Title. */ title: string;
  /** After the title. */ children?: ReactNode;
}) {
  return (
    <div className="mb-3 flex items-center gap-3">
      <span className="font-mono text-xs text-accent">{n}</span>
      <h2 className="text-base font-semibold">{title}</h2>
      {children}
    </div>
  );
}

/** Step 1: pick the profile and open (or download) the desktop browser. */
export function ConnectStep({
  s,
  personas,
}: {
  /** State. */ s: OnboardingState;
  /** The key's personas. */ personas: Persona[];
}) {
  return (
    <section>
      <StepTitle n="01" title="Connect your desktop" />
      <label htmlFor="setup-profile" className="label">
        Save accounts to
      </label>
      <select
        id="setup-profile"
        className="field mb-3"
        value={s.profileId}
        onChange={(e) => s.setProfileId(e.target.value)}
      >
        <option value="default">Default profile</option>
        {personas
          .filter((p) => !p.isDefault)
          .map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
      </select>
      <div className="flex flex-wrap gap-2">
        <button className="btn-primary" onClick={s.pair} disabled={!!s.busy}>
          {s.busy === 'pair' ? <Loader2 className="h-4 w-4 animate-spin" /> : <Monitor className="h-4 w-4" />}
          {s.desktop ? 'Open desktop' : 'Connect desktop'}
        </button>
        <a className="btn-ghost" href="/downloads" target="_blank" rel="noreferrer">
          <Download className="h-4 w-4" />
          Download
        </a>
      </div>
      <p className="mt-3 text-xs text-text-muted" role="status">
        {s.desktop
          ? 'Desktop connected. You can sign in now.'
          : 'Already installed? Connect opens your existing Oya window.'}
      </p>
    </section>
  );
}

/** Step 2: sign in on the desktop; shows which sites have saved sessions so far. */
export function SignInStep({ sites }: { /** Sites with a saved session. */ sites: string[] }) {
  return (
    <section className="border-t border-border pt-6">
      <StepTitle n="02" title="Sign in to your accounts">
        {sites.length > 0 && <Check className="ml-auto h-4 w-4 text-accent" />}
      </StepTitle>
      <p className="text-sm text-text-secondary">
        Log in normally in Oya, including any CAPTCHA or MFA. Click{' '}
        <strong className="font-medium text-text">Save profile</strong> in the desktop toolbar when you’re done.
      </p>
      <div className="mt-3 rounded-md border border-border bg-bg-sunken px-4 py-3 text-sm" role="status">
        {sites.length ? (
          <>
            <span className="text-accent">
              Saved state for {sites.length} {sites.length === 1 ? 'site' : 'sites'}
            </span>
            <p className="mt-1 break-words text-xs text-text-secondary">{sites.join(' · ')}</p>
          </>
        ) : (
          <span className="text-text-muted">Waiting for saved account sessions…</span>
        )}
      </div>
      <p className="mt-2 text-xs text-text-muted">
        Cookies and local storage sync. A site may still ask you to verify a new session.
      </p>
    </section>
  );
}

/** Step 3: choose the provider, enter what it needs, save and go. */
export function ProviderStep({
  s,
  config,
}: {
  /** State. */ s: OnboardingState;
  /** The key's settings. */ config: KeyConfig;
}) {
  return (
    <section className="border-t border-border pt-6">
      <StepTitle n="03" title="Choose where browsers run" />
      <label htmlFor="setup-provider" className="sr-only">
        Browser provider
      </label>
      <select id="setup-provider" className="field" value={s.provider} onChange={(e) => s.setProvider(e.target.value)}>
        {config.providers.map((p) => (
          <option key={p.id} value={p.id}>
            {p.label}
            {p.configured ? ' · ready' : ' · setup needed'}
          </option>
        ))}
      </select>
      {s.needs.map((name) => (
        <div key={name} className="mt-3">
          <label className="label" htmlFor={`setup-${name}`}>
            {name.replace(/_/g, ' ')}
          </label>
          <input
            id={`setup-${name}`}
            className="field"
            type="password"
            autoComplete="off"
            value={s.credentials[name] || ''}
            placeholder={s.configured ? 'Already saved, leave blank to keep' : 'Enter credential'}
            onChange={(e) => s.setCredentials({ ...s.credentials, [name]: e.target.value })}
          />
        </div>
      ))}
      <p className="mt-2 text-xs text-text-muted">
        CAPTCHA solver and MFA factors are optional settings in the console.
      </p>
      <button className="btn-primary mt-4" onClick={s.finish} disabled={!!s.busy}>
        {s.busy === 'save' && <Loader2 className="h-4 w-4 animate-spin" />}Save and open console{' '}
        <ArrowRight className="h-4 w-4" />
      </button>
    </section>
  );
}
