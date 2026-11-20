/**
 * A single browser's live view on its own page, reachable from the console or
 * through a shared link that carries its own scoped token.
 */
'use client';

import Link from 'next/link';
import { useParams } from 'next/navigation';
import { ArrowLeft, RefreshCw } from 'lucide-react';
import LiveView from '@/components/dashboard/live-view';
import { useLivePage } from './use-live-page';

/** The dynamic segment. */
type RouteParams = {
  /** The browser to show. */
  browserId: string;
};

/** Props for the live page header. */
interface HeaderProps {
  /** Browser heading. */
  name: string;
  /** Whether an active credential is available. */
  authenticated: boolean;
  /** Current control action label. */
  controlLabel: string;
  /** Toggles human/agent control. */
  onToggleControl: () => void;
  /** Reopens the live stream. */
  onReconnect: () => void;
}

/** Header with back link, browser name, and authenticated control buttons. */
function LiveHeader({ name, authenticated, controlLabel, onToggleControl, onReconnect }: HeaderProps) {
  return (
    <header className="mb-6 flex flex-wrap items-center justify-between gap-4">
      <div>
        <Link href="/dashboard" className="inline-flex items-center gap-2 text-xs text-text-muted hover:text-text">
          <ArrowLeft className="h-3.5 w-3.5" />
          Dashboard
        </Link>
        <h1 className="mt-3 text-2xl font-medium tracking-tight">{name}</h1>
      </div>
      {authenticated && (
        <div className="flex items-center gap-2">
          <button className="btn-ghost" onClick={onToggleControl}>
            {controlLabel}
          </button>
          <button className="btn-ghost" onClick={onReconnect}>
            <RefreshCw className="h-4 w-4" />
            Reconnect
          </button>
        </div>
      )}
    </header>
  );
}

/** Body shown while hydrating, unauthenticated, or streaming live frames. */
function LiveBody({ page }: { /** State returned by useLivePage. */ page: ReturnType<typeof useLivePage> }) {
  const { apiKey, live, control, send } = page;
  if (apiKey === null) {
    return (
      <p role="status" className="text-sm text-text-muted">
        Loading live view…
      </p>
    );
  }
  if (apiKey === '') {
    return (
      <p className="rounded-xl border border-border bg-bg-card p-6 text-sm text-text-muted">
        Connect your Oya key in the{' '}
        <Link href="/dashboard" className="font-medium text-accent underline hover:text-accent-hover">
          dashboard
        </Link>{' '}
        to view this browser. Or open a valid share link.
      </p>
    );
  }
  return (
    <>
      {live.error && (
        <p role="alert" className="mb-4 rounded-lg border border-red/20 bg-red/5 p-3 text-sm text-red">
          {live.error}
        </p>
      )}
      <LiveView
        frameSrc={live.frame}
        fps={live.fps}
        frameAgeMs={live.age}
        send={send}
        interactive={control.mode === 'human'}
        large
      />
    </>
  );
}

/** The live page for the browser in the URL. */
export default function LiveBrowserPage() {
  const { browserId } = useParams<RouteParams>();
  const page = useLivePage(browserId);
  return (
    <main className="workspace-page mx-auto max-w-[1600px] p-4 sm:p-8">
      <LiveHeader
        name={page.live.name}
        authenticated={Boolean(page.apiKey)}
        controlLabel={page.control.label}
        onToggleControl={page.control.toggle}
        onReconnect={page.reconnect}
      />
      <LiveBody page={page} />
    </main>
  );
}
