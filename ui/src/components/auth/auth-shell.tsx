/**
 * The frame the sign-in and sign-up pages share: ambient glow, brand, card
 * and the link to the other page, plus the spinner shown while the session
 * is still resolving.
 */
import type { ReactNode } from 'react';
import { Loader2 } from 'lucide-react';

/** The accent each page glows in. Full class names, so Tailwind sees them. */
const GLOWS = {
  accent: { ambient: 'bg-accent/[0.04]', edge: 'via-accent/50' },
  indigo: { ambient: 'bg-indigo/[0.04]', edge: 'via-indigo/50' },
};

/** AuthShell's props. */
interface ShellProps {
  /** The page's accent color. */
  glow: keyof typeof GLOWS;
  /** The card's contents. */
  children: ReactNode;
  /** The line under the card linking to the other page. */
  footer: ReactNode;
}

/** The brand line above the card. */
function Brand() {
  return (
    <div className="mb-8 flex items-center justify-center gap-2.5">
      <span className="relative flex h-3 w-3">
        <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-accent opacity-40" />
        <span className="relative inline-flex h-3 w-3 rounded-full bg-accent" />
      </span>
      <span className="font-display text-xl font-bold tracking-tight text-text">Oya Browser</span>
    </div>
  );
}

/** A centered card with the brand above it and `footer` below. */
export function AuthShell({ glow, children, footer }: ShellProps) {
  const { ambient, edge } = GLOWS[glow];
  return (
    <div className="flex min-h-dvh items-center justify-center bg-bg px-4 py-12">
      {/* Ambient glow */}
      <div className="pointer-events-none fixed inset-0 overflow-hidden">
        <div
          className={`absolute -top-[40%] left-1/2 h-[800px] w-[800px] -translate-x-1/2 rounded-full ${ambient} blur-[120px]`}
        />
      </div>
      <div className="relative w-full max-w-[420px]">
        <Brand />
        <div className="relative rounded-2xl border border-border bg-bg-card/70 p-8 shadow-2xl shadow-black/40 backdrop-blur-xl">
          {/* Gradient top border accent */}
          <div
            className={`pointer-events-none absolute inset-x-0 top-0 h-px rounded-t-2xl bg-gradient-to-r from-transparent ${edge} to-transparent`}
          />
          {children}
        </div>
        <p className="mt-6 text-center text-sm text-text-muted">{footer}</p>
      </div>
    </div>
  );
}

/** Shown until the auth state is resolved. */
export function AuthLoading() {
  return (
    <div className="flex min-h-dvh items-center justify-center bg-bg">
      <Loader2 className="h-6 w-6 animate-spin text-text-muted" />
    </div>
  );
}
