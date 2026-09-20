/**
 * The one overlay the console uses, plus Confirm, a yes/no built on it. The
 * focus and keyboard behavior lives in use-dialog.ts.
 */
'use client';

import type { ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { X } from 'lucide-react';
import { motion, AnimatePresence } from 'framer-motion';
import { useDialog } from './use-dialog';

/** How wide the dialog is; a drawer slides in from the right at full height. */
type Size = 'sm' | 'md' | 'lg' | 'drawer';

/** Dialog's props. */
interface DialogProps {
  /** Whether it is showing. */
  open: boolean;
  /** Called by Escape, the backdrop and the close button. */
  onClose: () => void;
  /** The heading; a string also names the dialog for assistive tech. */
  title: ReactNode;
  /** A line under the title. */
  description?: ReactNode;
  /** Width, or a drawer. */
  size?: Size;
  /** The body. */
  children: ReactNode;
  /** Buttons along the bottom. */
  footer?: ReactNode;
}

/** Width classes by size. */
const SIZES: Record<Size, string> = {
  sm: 'w-full max-w-md',
  md: 'w-full max-w-lg',
  lg: 'w-full max-w-3xl',
  drawer: 'h-full w-full max-w-xl',
};

/** The panel's header: title, description and close button. */
function DialogHeader({ title, description, onClose }: Pick<DialogProps, 'title' | 'description' | 'onClose'>) {
  return (
    <div className="flex shrink-0 items-start justify-between gap-4 border-b border-border px-5 py-4">
      <div className="min-w-0">
        <h2 className="text-[15px] font-semibold text-text">{title}</h2>
        {description && <p className="mt-0.5 text-[13px] text-text-muted">{description}</p>}
      </div>
      <button
        onClick={onClose}
        aria-label="Close"
        className="-mr-1 rounded-md p-1 text-text-muted hover:bg-text/5 hover:text-text"
      >
        <X className="h-4 w-4" />
      </button>
    </div>
  );
}

/**
 * The one overlay. Focus is trapped inside, Escape and the backdrop close it,
 * and focus returns to whatever opened it — the three things every hand-rolled
 * modal in this app forgot.
 */
export default function Dialog({ open, onClose, title, description, size = 'md', children, footer }: DialogProps) {
  const panel = useDialog(open, onClose);
  const drawer = size === 'drawer';
  const hidden = drawer ? { x: 24, opacity: 0 } : { y: 8, opacity: 0 };

  if (typeof document === 'undefined') return null;
  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div
          key="backdrop"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.12 }}
          className={`fixed inset-0 z-[60] flex bg-black/60 ${drawer ? 'justify-end' : 'items-start justify-center overflow-y-auto p-4 pt-[8vh]'}`}
          onMouseDown={(e) => {
            if (e.target === e.currentTarget) onClose();
          }}
        >
          <motion.div
            ref={panel}
            role="dialog"
            aria-modal="true"
            aria-label={typeof title === 'string' ? title : undefined}
            tabIndex={-1}
            initial={hidden}
            animate={{ x: 0, y: 0, opacity: 1 }}
            exit={hidden}
            transition={{ duration: 0.15 }}
            className={`${SIZES[size]} flex flex-col border border-border bg-bg-card shadow-[var(--shadow-elevated)] outline-none ${drawer ? 'border-y-0 border-r-0' : 'max-h-[calc(92dvh-2rem)] rounded-xl'}`}
          >
            <DialogHeader title={title} description={description} onClose={onClose} />
            <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain px-5 py-4">{children}</div>
            {footer && (
              <div className="flex shrink-0 items-center justify-end gap-2 border-t border-border px-5 py-3">
                {footer}
              </div>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body,
  );
}

/** Confirm's props. */
interface ConfirmProps {
  /** Whether it is showing. */
  open: boolean;
  /** Cancels. */
  onClose: () => void;
  /** Goes ahead. */
  onConfirm: () => void;
  /** The question. */
  title: ReactNode;
  /** The consequence, spelled out. */
  body: ReactNode;
  /** The go-ahead button's text. */
  confirmLabel?: string;
  /** Draws the go-ahead in red. */
  danger?: boolean;
  /** Disables the go-ahead while the action runs. */
  busy?: boolean;
}

/** A yes/no with the consequence spelled out. */
export function Confirm({
  open,
  onClose,
  onConfirm,
  title,
  body,
  confirmLabel = 'Confirm',
  danger = false,
  busy = false,
}: ConfirmProps) {
  return (
    <Dialog
      open={open}
      onClose={onClose}
      title={title}
      size="sm"
      footer={
        <>
          <button onClick={onClose} className="btn-ghost">
            Cancel
          </button>
          <button onClick={onConfirm} disabled={busy} className={danger ? 'btn-danger' : 'btn-primary'}>
            {busy ? 'Working…' : confirmLabel}
          </button>
        </>
      }
    >
      <div className="text-sm leading-relaxed text-text-secondary">{body}</div>
    </Dialog>
  );
}
