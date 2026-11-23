/**
 * The sign-up form: optional display name, email, password with a length
 * hint, the submit button, and the line saying what creating an account agrees to.
 */
'use client';

import type { FormEvent } from 'react';
import Link from 'next/link';
import { FormError, PasswordField, SubmitButton, TextField } from '@/components/auth/fields';
import { clearing, type FormState } from '@/components/auth/use-auth-form';
import { Turnstile } from '@/components/auth/turnstile';
import type { Captcha } from '@/components/auth/use-turnstile';
import { MIN_PASSWORD_LENGTH, type SignupFields } from './use-signup';

/** SignupForm's props. */
interface Props {
  /** The typed values. */
  fields: SignupFields;
  /** Error and busy state. */
  form: FormState;
  /** The captcha, off when there is no site key. */
  captcha: Captcha;
  /** Handles submit. */
  onSubmit: (e: FormEvent) => void;
}

/** HintProps' props. */
interface HintProps {
  /** The password typed so far. */
  password: string;
}

/** How many more characters the password needs, once typing has started. */
function LengthHint({ password }: HintProps) {
  const missing = MIN_PASSWORD_LENGTH - password.length;
  if (!(password.length > 0 && missing > 0)) return null;
  return (
    <p className="text-xs text-yellow">
      {missing} more character{missing !== 1 ? 's' : ''} needed
    </p>
  );
}

/** What creating an account agrees to, with both documents a click away. */
function Consent() {
  return (
    <p className="text-center text-xs text-text-dim">
      By creating an account you agree to the{' '}
      <Link href="/terms" className="whitespace-nowrap underline hover:text-text">
        Terms of Service
      </Link>{' '}
      and{' '}
      <Link href="/privacy" className="whitespace-nowrap underline hover:text-text">
        Privacy Policy
      </Link>
      .
    </p>
  );
}

/** The sign-up form. */
export function SignupForm({ fields, form, captcha, onSubmit }: Props) {
  return (
    <form onSubmit={onSubmit} className="space-y-5">
      <TextField
        id="displayName"
        label={
          <>
            Display name <span className="text-text-dim">(optional)</span>
          </>
        }
        autoComplete="name"
        placeholder="How should we call you?"
        value={fields.displayName}
        onChange={fields.setDisplayName}
      />
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
        autoComplete="new-password"
        placeholder="Min. 8 characters"
        value={fields.password}
        onChange={clearing(form, fields.setPassword)}
        hint={<LengthHint password={fields.password} />}
      />
      <Turnstile captcha={captcha} />
      <FormError error={form.error} />
      <SubmitButton busy={form.submitting} label="Create account" busyLabel="Creating account..." />
      <Consent />
    </form>
  );
}
