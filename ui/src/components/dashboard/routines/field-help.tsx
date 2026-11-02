/**
 * A field's help line, which its error replaces once a save was tried.
 */
'use client';

/** Props for the help line. */
interface Props {
  /** The id the field points at with aria-describedby. */
  id: string;
  /** What the field is for. */
  help?: string;
  /** What is wrong with it. */
  error?: string;
}

/** The error in red, or the help, or nothing. */
export default function FieldHelp({ id, help, error }: Props) {
  if (!error && !help) return null;
  return (
    <p id={id} className={`mt-1 text-[12px] ${error ? 'text-red' : 'text-text-muted'}`}>
      {error || help}
    </p>
  );
}
