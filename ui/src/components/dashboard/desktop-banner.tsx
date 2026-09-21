/**
 * The banner that asks a cloud-provider user to sign in once on a desktop
 * browser, whose logins their cloud browsers then inherit.
 */
'use client';

import { Download, ArrowRight, X, Loader2, MonitorSmartphone } from 'lucide-react';
import { useDesktopSignIn } from './hooks/use-desktop-sign-in';

/** Whose desktop to open, and how to hide the banner. */
interface Props {
  /** The key the desktop signs in with. */
  apiKey: string;
  /** Hides the banner for now. */
  onDismiss: () => void;
}

/**
 * Oya browsers inherit the logins of the desktop browser paired with this
 * key. Until one has signed in, every cloud browser starts logged out, which
 * is the one thing a customer on this provider did not sign up for.
 */
export default function DesktopBanner({ apiKey, onDismiss }: Props) {
  const { busy, open } = useDesktopSignIn(apiKey);

  return (
    <div
      role="status"
      className="flex items-center gap-3 border-b border-accent/30 bg-gradient-to-r from-accent/[0.12] via-accent/[0.06] to-transparent px-4 py-2.5 lg:px-6"
    >
      <MonitorSmartphone className="h-4 w-4 shrink-0 text-accent" />
      <div className="min-w-0 flex-1 text-[13px] leading-snug">
        <span className="font-medium text-text">Sign in once on your own machine.</span>{' '}
        <span className="text-text-secondary">
          Your cloud browsers run as this identity and inherit its logins, until a desktop browser has signed in, they
          start logged out.
        </span>
      </div>
      <a href="/downloads" target="_blank" rel="noreferrer" className="btn-ghost h-7">
        <Download className="h-3.5 w-3.5" /> Download
      </a>
      <button className="btn-primary h-7" onClick={() => open()} disabled={busy}>
        {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <ArrowRight className="h-3.5 w-3.5" />} Open desktop
        browser
      </button>
      <button className="btn-icon h-7 w-7" onClick={onDismiss} aria-label="Dismiss for now" title="Dismiss for now">
        <X className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}
