/**
 * The form controls of the sign-in and sign-up pages: labeled inputs, a
 * password input with a show/hide toggle, the error line and the submit
 * button.
 */
import { useState, type ReactNode } from 'react';
import { ArrowRight, Eye, EyeOff, Loader2 } from 'lucide-react';

/** The input style every auth field shares; the password input adds room for its toggle in between. */
const INPUT_BOX = 'block w-full rounded-lg border border-border bg-bg-input px-3.5 py-2.5';
/** The rest of the input style. */
const INPUT_TEXT =
  'text-sm text-text placeholder:text-text-dim focus:border-border-focus focus:outline-none focus:ring-1 focus:ring-border-focus';
/** A plain field's input style. */
const INPUT = `${INPUT_BOX} ${INPUT_TEXT}`;

/** A field's props. */
interface FieldProps {
  /** The input's id, which the label points at. */
  id: string;
  /** The label's contents. */
  label: ReactNode;
  /** The input type. */
  type?: string;
  /** The browser autofill hint. */
  autoComplete: string;
  /** Placeholder text. */
  placeholder: string;
  /** The current value. */
  value: string;
  /** Receives each new value. */
  onChange: (value: string) => void;
}

/** A labeled text input. */
export function TextField({ id, label, type = 'text', autoComplete, placeholder, value, onChange }: FieldProps) {
  return (
    <div className="space-y-1.5">
      <label htmlFor={id} className="block text-sm font-medium text-text-muted">
        {label}
      </label>
      <input
        id={id}
        type={type}
        autoComplete={autoComplete}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className={INPUT}
      />
    </div>
  );
}

/** A password input's props: a field plus what to show under it. */
interface PasswordProps extends Omit<FieldProps, 'id' | 'label' | 'type'> {
  /** Shown under the input, e.g. a strength hint. */
  hint?: ReactNode;
}

/** The password input, with a button that shows or hides what was typed. */
export function PasswordField({ autoComplete, placeholder, value, onChange, hint }: PasswordProps) {
  const [shown, setShown] = useState(false);
  return (
    <div className="space-y-1.5">
      <label htmlFor="password" className="block text-sm font-medium text-text-muted">
        Password
      </label>
      <div className="relative">
        <input
          id="password"
          type={shown ? 'text' : 'password'}
          autoComplete={autoComplete}
          placeholder={placeholder}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          className={`${INPUT_BOX} pr-11 ${INPUT_TEXT}`}
        />
        <button
          type="button"
          aria-label={shown ? 'Hide password' : 'Show password'}
          onClick={() => setShown((v) => !v)}
          className="absolute right-3 top-1/2 -translate-y-1/2 text-text-dim hover:text-text-muted"
        >
          {shown ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
        </button>
      </div>
      {hint}
    </div>
  );
}

/** FormError's props. */
interface ErrorProps {
  /** The message, or empty for none. */
  error: string;
}

/** The form's error line, announced to assistive tech. */
export function FormError({ error }: ErrorProps) {
  if (!error) return null;
  return (
    <p className="text-sm text-red" role="alert">
      {error}
    </p>
  );
}

/** SubmitButton's props. */
interface SubmitProps {
  /** Whether the form is being submitted. */
  busy: boolean;
  /** The label while idle. */
  label: string;
  /** The label while busy. */
  busyLabel: string;
}

/** The full-width submit button, with a spinner while busy. */
export function SubmitButton({ busy, label, busyLabel }: SubmitProps) {
  return (
    <button
      type="submit"
      disabled={busy}
      className="flex w-full items-center justify-center gap-2 rounded-lg bg-accent px-4 py-2.5 text-sm font-semibold text-accent-foreground shadow-lg shadow-accent/25 hover:bg-accent-hover disabled:cursor-not-allowed disabled:opacity-60"
    >
      {busy ? (
        <>
          <Loader2 className="h-4 w-4 animate-spin" />
          {busyLabel}
        </>
      ) : (
        <>
          {label}
          <ArrowRight className="h-4 w-4" />
        </>
      )}
    </button>
  );
}
