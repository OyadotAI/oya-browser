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

/** The live page for the browser in the URL. */
export default function LiveBrowserPage() {
  const { browserId } = useParams<RouteParams>();
  const { apiKey, live, control, send, reconnect } = useLivePage(browserId);
  return (
    <main className="mx-auto max-w-[1600px] p-4 sm:p-8">
      <header className="mb-6 flex flex-wrap items-center justify-between gap-4">
        <div>
          <Link href="/dashboard" className="inline-flex items-center gap-2 text-xs text-text-muted hover:text-text">
            <ArrowLeft className="h-3.5 w-3.5" />
            Dashboard
          </Link>
          <h1 className="mt-3 text-2xl font-medium tracking-tight">{live.name}</h1>
        </div>
        <button className="btn-ghost" onClick={control.toggle}>
          {control.label}
        </button>
        <button className="btn-ghost" onClick={reconnect}>
          <RefreshCw className="h-4 w-4" />
          Reconnect
        </button>
      </header>
      {apiKey === '' ? (
        <p>
          Connect your Oya key in the{' '}
          <Link href="/dashboard" className="underline">
            dashboard
          </Link>{' '}
          to view this browser.
        </p>
      ) : (
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
      )}
    </main>
  );
}
