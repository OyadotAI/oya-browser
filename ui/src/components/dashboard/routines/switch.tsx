/**
 * An on/off switch: a real button with role="switch", so Space and Enter flip it
 * and a screen reader says whether it is on.
 */
'use client';

/** Props for the switch. */
interface Props {
  /** Whether it is on. */
  checked: boolean;
  /** What it turns on, for a screen reader. */
  label: string;
  /** Holds it while a change is saving. */
  disabled?: boolean;
  /** Flips it. */
  onChange: () => void;
}

/** A small track and thumb, accent when on. */
export default function Switch({ checked, label, disabled, onChange }: Props) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={onChange}
      className={`relative inline-flex h-5 w-9 shrink-0 items-center rounded-full transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-border-focus disabled:opacity-60 ${
        checked ? 'bg-accent' : 'bg-border'
      }`}
    >
      <span
        className={`inline-block h-4 w-4 rounded-full bg-text transition-transform ${
          checked ? 'translate-x-[18px]' : 'translate-x-[2px]'
        }`}
      />
    </button>
  );
}
