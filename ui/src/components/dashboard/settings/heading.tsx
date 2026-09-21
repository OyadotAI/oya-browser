/**
 * Pieces every settings panel shares: its heading and its loading state.
 */
'use client';

import type { ReactNode } from 'react';
import { Loader2 } from 'lucide-react';

/** A panel's eyebrow, title and one-line description. */
export function SectionHeading({
  eyebrow,
  title,
  children,
}: {
  /** Small accent line above the title. */
  eyebrow: string;
  /** The panel's promise. */
  title: string;
  /** What the panel is for. */
  children: ReactNode;
}) {
  return (
    <div className="mb-7">
      <p className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent">{eyebrow}</p>
      <h3 className="text-[22px] font-semibold tracking-[-0.035em]">{title}</h3>
      <p className="mt-1.5 text-[13px] leading-6 text-text-muted">{children}</p>
    </div>
  );
}

/** A spinner with a line of text, while a panel loads. */
export function Loading({ children }: { /** What is loading. */ children: ReactNode }) {
  return (
    <div className="flex min-h-[270px] items-center justify-center gap-2 text-sm text-text-muted" role="status">
      <Loader2 className="h-4 w-4 animate-spin" />
      {children}
    </div>
  );
}

/** A failure shown at the top of a panel. */
export function ErrorNote({
  spaced = true,
  children,
}: {
  /** Leaves room below it; false when it is the whole panel. */
  spaced?: boolean;
  /** The message, and any action. */
  children: ReactNode;
}) {
  return (
    <div
      role="alert"
      className={`${spaced ? 'mb-5 ' : ''}rounded-lg border border-red/25 bg-red/5 p-3 text-[13px] text-red`}
    >
      {children}
    </div>
  );
}
