/**
 * A modal dialog over the page: the backdrop (a press on it closes), the
 * window-drag strip, and the caller's content. While open, focus starts on
 * one control, Escape closes, Tab stays inside, and focus returns on close.
 */
import { useRef, type ReactNode } from 'react';
import { useDialog } from '../hooks/index.ts';

/** What a Dialog is given. */
export interface DialogProps {
  /** Whether it is open. */
  open: boolean;
  /** Closes it. */
  onClose: () => void;
  /** The id of the control focused when it opens. */
  focusId: string;
  /** The backdrop's id. */
  id: string;
  /** The backdrop's classes. */
  className: string;
  /** Its accessible name. */
  label: string;
  /** Hide the backdrop (for a dialog the stylesheet does not hide by class). */
  hidden?: boolean;
  /** What it shows. */
  children: ReactNode;
}

/** A modal dialog. */
export function Dialog({ open, onClose, focusId, id, className, label, hidden, children }: DialogProps) {
  const root = useRef<HTMLDivElement>(null);
  useDialog(root, open, onClose, focusId);
  return (
    <div
      className={className}
      id={id}
      hidden={hidden}
      role="dialog"
      aria-modal="true"
      aria-label={label}
      ref={root}
      onClick={(event) => event.target === event.currentTarget && onClose()}
    >
      <div className="window-drag" aria-hidden="true"></div>
      {children}
    </div>
  );
}
