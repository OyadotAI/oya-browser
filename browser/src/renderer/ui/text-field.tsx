/**
 * A labelled text input in a `.field` box: the label above, the input, and an
 * optional hint under it. The input takes every native attribute.
 */
import type { InputHTMLAttributes, ReactNode } from 'react';
import './text-field.css';

/** What a TextField is given: the input's attributes, its id, its label and a hint. */
export interface TextFieldProps extends InputHTMLAttributes<HTMLInputElement> {
  /** The input's id, which the label points at. */
  id: string;
  /** The words above it. */
  label: string;
  /** What goes under it, if anything. */
  hint?: ReactNode;
}

/** A labelled field. */
export function TextField({ id, label, hint, ...input }: TextFieldProps) {
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      <input id={id} {...input} />
      {hint}
    </div>
  );
}
