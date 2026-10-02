/**
 * The frame the sign-in and sign-up pages share: ambient glow, brand, card
 * and the link to the other page, plus the spinner shown while the session
 * is still resolving. Given a pitch, the card moves right and the pitch
 * takes the left, the layout sign-in and both sides of sign-up share.
 */
import type { ReactNode } from 'react';
import { Loader2 } from 'lucide-react';
import { OyaWordmark } from '@/components/oya-logo';

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
  /** Shown beside the card, which widens the page to two columns. */
  pitch?: ReactNode;
  /** Shown between the brand and the columns, such as a switch between pages. */
  above?: ReactNode;
}

/** The Oya wordmark above the card, linking home. */
function Brand() {
  return (
    <div className="mb-8 flex justify-center text-text">
      <OyaWordmark />
    </div>
  );
}

/** Card's props. */
interface CardProps {
  /** The gradient class of the top edge. */
  edge: string;
  /** The card's contents. */
  children: ReactNode;
}

/** The card, with its gradient top edge. */
function Card({ edge, children }: CardProps) {
  return (
    <div className="relative rounded-2xl border border-border bg-bg-card/70 p-8 shadow-2xl shadow-black/40 backdrop-blur-xl">
      {/* Gradient top border accent */}
      <div
        className={`pointer-events-none absolute inset-x-0 top-0 h-px rounded-t-2xl bg-gradient-to-r from-transparent ${edge} to-transparent`}
      />
      {children}
    </div>
  );
}

/** A card with the brand above it and `footer` below; centered alone, or right of `pitch`. */
export function AuthShell({ glow, children, footer, pitch, above }: ShellProps) {
  const { ambient, edge } = GLOWS[glow];
  return (
    <div className="flex min-h-dvh items-center justify-center bg-bg px-4 py-12">
      {/* Ambient glow */}
      <div className="pointer-events-none fixed inset-0 overflow-hidden">
        <div
          className={`absolute -top-[40%] left-1/2 h-[800px] w-[800px] -translate-x-1/2 rounded-full ${ambient} blur-[120px]`}
        />
      </div>
      <div className={`relative flex w-full flex-col items-center ${pitch ? 'max-w-[1100px]' : 'max-w-[420px]'}`}>
        <Brand />
        {above}
        <div className="flex w-full flex-wrap items-start gap-12">
          {pitch}
          <div className={`min-w-0 flex-1 ${pitch ? 'basis-[440px]' : ''}`}>
            <Card edge={edge}>{children}</Card>
            <p className="mt-6 text-center text-sm text-text-muted">{footer}</p>
          </div>
        </div>
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
