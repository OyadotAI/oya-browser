/** Independent Keel reporting view: local failures never erase navigation or a previous successful snapshot. */
'use client';
import { useAuth } from '@/components/auth-provider';
import { useKeel } from './use-keel';
import { KeelMetrics } from './keel-metrics';
import { KeelCharts } from './keel-charts';
import { KeelInstallations, KeelLicenses, reportTime } from './keel-tables';
import { measurementNote } from './keel-model';
import type { KeelSnapshot } from './keel-types';

/** Refresh preserves focus and exposes its current purpose to assistive technology. */
function Refresh({
  loading,
  error,
  refresh,
}: {
  /** Request pending. */ loading: boolean;
  /** Last failure. */ error: string;
  /** Starts a fresh request. */ refresh: () => void;
}) {
  return (
    <button
      type="button"
      onClick={refresh}
      disabled={loading}
      aria-label={error ? 'Retry Keel usage' : 'Refresh Keel usage'}
      className="min-h-11 rounded-md border border-border px-4 text-xs text-text hover:bg-bg-card focus-visible:ring-2 focus-visible:ring-accent disabled:opacity-50"
    >
      {loading ? 'Refreshing…' : error ? 'Retry' : 'Refresh'}
    </button>
  );
}

/** Loading placeholders mirror the final four-headline layout without invented values. */
function Loading() {
  return (
    <div role="status" className="flex flex-col gap-3">
      <p className="text-sm text-text-muted">Loading Keel usage…</p>
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4" aria-hidden="true">
        {['installs', 'licenses', 'runtime', 'projects'].map((name) => (
          <div key={name} className="h-36 rounded-lg border border-border bg-bg-card" />
        ))}
      </div>
    </div>
  );
}

/** Successful data explains both daily reporting latency and incomplete measurement history. */
function Snapshot({ data }: { /** Last successful snapshot. */ data: KeelSnapshot }) {
  return (
    <>
      <KeelMetrics data={data} />
      <div className="space-y-2 text-xs text-text-muted">
        <p>{measurementNote(data.usage)}</p>
        <p>
          Agent-hours include tool execution and approval waits; concurrent runs add together. Idle app time is
          excluded.
        </p>
        <p>
          Active projects are repositories where a supervised run started during the period; worktrees on one install
          count once.
        </p>
        <p>Reports arrive daily and can arrive late. Active means recently reporting, not online now.</p>
        <p>Latest report {reportTime(data.latest_report)}.</p>
        {!data.installs.total ? (
          <p>No Keel reports received yet.</p>
        ) : (
          !data.installs.active && <p>No reports received in the last 7 days.</p>
        )}
      </div>
      <KeelCharts data={data} />
      <KeelInstallations data={data} />
      <KeelLicenses data={data} />
    </>
  );
}

/** Opening this tab starts its own guarded request independently of overview and billing. */
export function KeelView({ state }: { /** Independent protected feed. */ state: ReturnType<typeof useKeel> }) {
  return (
    <div className="flex flex-col gap-6" aria-busy={state.loading}>
      <div className="flex flex-col justify-between gap-3 sm:flex-row sm:items-center">
        <div>
          <h2 className="font-display text-xl text-text">Keel</h2>
          <p className="text-xs text-text-muted">Reported installation and agent activity</p>
        </div>
        <Refresh {...state} />
      </div>
      <p role="status" aria-live="polite" className="text-xs text-text-dim">
        {state.snapshot ? `Updated ${reportTime(state.snapshot.generated_at)}` : ''}
      </p>
      {state.error && (
        <p role="alert" className="text-sm text-red">
          {state.snapshot
            ? `Couldn’t refresh. Showing data updated ${reportTime(state.snapshot.generated_at)}.`
            : 'Keel usage could not be loaded.'}{' '}
          {state.error}
        </p>
      )}
      {!state.snapshot && state.loading ? <Loading /> : state.snapshot && <Snapshot data={state.snapshot} />}
    </div>
  );
}

/** Standalone tab callers own the same credential-scoped request behavior. */
export function KeelTab() {
  const { token } = useAuth();
  return <KeelView state={useKeel(token)} />;
}
