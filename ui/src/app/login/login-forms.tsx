/**
 * The sign-in card's contents: the account/key switch and the two forms.
 */
'use client';

import type { FormEvent } from 'react';
import { FormError, PasswordField, SubmitButton, TextField } from '@/components/auth/fields';
import { clearing, type FormState } from '@/components/auth/use-auth-form';
import { Turnstile } from '@/components/auth/turnstile';
import type { Captcha } from '@/components/auth/use-turnstile';
import type { LoginMode, useLogin } from './use-login';

/** The typed values and their setters. */
type Fields = ReturnType<typeof useLogin>['fields'];

/** A form's props. */
interface FormProps {
  /** The typed values. */
  fields: Fields;
  /** Error and busy state. */
  form: FormState;
  /** Handles submit. */
  onSubmit: (e: FormEvent) => void;
}

/** The modes' labels. */
const MODES: Array<[LoginMode, string]> = [
  ['account', 'Human'],
  ['key', 'Machine'],
  ['agent', 'Agent'],
];

/** Switches between human, machine and agent sign-in, clearing any error. */
export function ModeSwitch({ fields, form }: Omit<FormProps, 'onSubmit'>) {
  return (
    <div className="mb-6 grid grid-cols-3 gap-1 rounded-lg border border-border p-1">
      {MODES.map(([m, label]) => (
        <button
          key={m}
          type="button"
          onClick={() => {
            fields.setMode(m);
            form.setError('');
          }}
          className={`rounded-md px-3 py-1.5 text-sm transition-colors ${
            fields.mode === m ? 'bg-accent/10 text-text' : 'text-text-dim hover:text-text-muted'
          }`}
        >
          {label}
        </button>
      ))}
    </div>
  );
}

/** Sign-in with an API key from this deployment. */
export function KeyForm({ fields, form, onSubmit }: FormProps) {
  return (
    <form onSubmit={onSubmit} className="space-y-5">
      <TextField
        id="apiKey"
        label="API key"
        type="password"
        autoComplete="off"
        placeholder="Paste your key"
        value={fields.apiKey}
        onChange={clearing(form, fields.setApiKey)}
      />
      <FormError error={form.error} />
      <SubmitButton busy={form.submitting} label="Continue" busyLabel="Checking..." />
    </form>
  );
}

/** The account form's props: a form plus its captcha. */
interface AccountProps extends FormProps {
  /** The captcha, off when there is no site key. */
  captcha: Captcha;
}

/** Sign-in with email and password, behind the captcha when it is on. */
export function AccountForm({ fields, form, captcha, onSubmit }: AccountProps) {
  return (
    <form onSubmit={onSubmit} className="space-y-5">
      <TextField
        id="email"
        label="Email"
        type="email"
        autoComplete="email"
        placeholder="you@example.com"
        value={fields.email}
        onChange={clearing(form, fields.setEmail)}
      />
      <PasswordField
        autoComplete="current-password"
        placeholder="Enter your password"
        value={fields.password}
        onChange={clearing(form, fields.setPassword)}
      />
      <Turnstile captcha={captcha} />
      <FormError error={form.error} />
      <SubmitButton busy={form.submitting} label="Sign in" busyLabel="Signing in..." />
    </form>
  );
}
