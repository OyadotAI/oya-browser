/**
 * The settings dialog's form parts: a labelled row, a styled select, and a
 * secret field that can be revealed.
 */
'use client';

import { useState, type ReactNode } from 'react';
import { ChevronDown, Eye, EyeOff } from 'lucide-react';

/** A labelled settings row. */
export interface RowProps {
  /** The control's id, which the label points at. */
  id?: string;
  /** The row's name. */
  label: string;
  /** A line under the label. */
  hint?: string;
  /** The control. */
  children: ReactNode;
}

/** The row component, as the Slack and webhook panels receive it. */
export type RowComponent = (props: RowProps) => ReactNode;

/** A controlled text field: its id, value and change handler. */
interface FieldProps {
  /** Element id. */
  id: string;
  /** Current value. */
  value: string;
  /** Called with the new value. */
  onChange: (value: string) => void;
}

/** Label and hint on the left, the control on the right. */
export function Row({ id, label, hint, children }: RowProps) {
  return (
    <div className="settings-row">
      <div className="pt-0.5">
        <label htmlFor={id} className="text-[13px] font-medium text-text">
          {label}
        </label>
        {hint && <p className="mt-1 text-[12px] leading-5 text-text-muted">{hint}</p>}
      </div>
      <div className="min-w-0">{children}</div>
    </div>
  );
}

/** A native select with the dialog's chevron. */
export function Select({ id, value, onChange, children }: FieldProps & { /** The options. */ children: ReactNode }) {
  return (
    <div className="relative">
      <select
        id={id}
        className="settings-input appearance-none pr-10"
        value={value}
        onChange={(e) => onChange(e.target.value)}
      >
        {children}
      </select>
      <ChevronDown
        aria-hidden="true"
        className="pointer-events-none absolute right-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-text-muted"
      />
    </div>
  );
}

/** A password field with a show/hide toggle, for API keys. */
export function Secret({
  id,
  value,
  onChange,
  placeholder,
}: FieldProps & { /** Shown when empty. */ placeholder: string }) {
  const [visible, setVisible] = useState(false);
  return (
    <div className="relative">
      <input
        id={id}
        className="settings-input pr-11 font-mono text-[12px]"
        type={visible ? 'text' : 'password'}
        autoComplete="off"
        spellCheck={false}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
      />
      <button
        type="button"
        onClick={() => setVisible(!visible)}
        className="absolute right-1 top-1 flex h-9 w-9 items-center justify-center rounded-md text-text-muted hover:bg-text/5 hover:text-text"
        aria-label={visible ? 'Hide credential' : 'Show credential'}
      >
        {visible ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
      </button>
    </div>
  );
}
