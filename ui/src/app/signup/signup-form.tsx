/**
 * The sign-up form: optional display name, email, password with a length
 * hint, and the submit button.
 */
'use client';

import type { FormEvent } from 'react';
import { FormError, PasswordField, SubmitButton, TextField } from '@/components/auth/fields';
import { clearing, type FormState } from '@/components/auth/use-auth-form';
import { MIN_PASSWORD_LENGTH, type SignupFields } from './use-signup';

/** SignupForm's props. */
interface Props {
  /** The typed values. */
  fields: SignupFields;
  /** Error and busy state. */
  form: FormState;
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

/** The sign-up form. */
export function SignupForm({ fields, form, onSubmit }: Props) {
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
      <FormError error={form.error} />
      <SubmitButton busy={form.submitting} label="Create account" busyLabel="Creating account..." />
    </form>
  );
}
