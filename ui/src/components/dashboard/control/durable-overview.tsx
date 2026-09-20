/**
 * The top of Project operations: the project's name and admission state, any
 * error or once-shown secret, and the three headline counts.
 */
import { COST_DECIMALS, DURABLE_BUTTON, ENDED_STATES, RECONCILE_STATES } from './constants';
import type { Durable } from './use-durable';
import type { DurableSession, Overview } from './types';

/** Project name and id, and whether it is accepting sessions. */
export function DurableHeader({ data }: { /** The durable overview. */ data: Overview }) {
  return (
    <header className="flex flex-wrap items-start justify-between gap-4">
      <div>
        <p className="font-mono text-[10px] uppercase tracking-[0.2em] text-accent">Project operations</p>
        <h2 className="mt-1 text-xl font-medium">{data.project.name}</h2>
        <p className="mt-1 font-mono text-xs text-text-dim">{data.project.id}</p>
      </div>
      <span
        className={`rounded border px-3 py-1 text-xs ${data.draining ? 'border-yellow text-yellow' : 'border-border text-text-muted'}`}
      >
        {data.draining ? 'Admission paused · draining' : 'Accepting sessions'}
      </span>
    </header>
  );
}

/** The last error, and a secret the server will not show again. */
export function DurableNotices({ d }: { /** Project operations state and actions. */ d: Durable }) {
  return (
    <>
      {d.error && (
        <p role="alert" className="rounded border border-red/40 p-3 text-sm text-red">
          {d.error}
        </p>
      )}
      {d.secret && (
        <div role="status" className="space-y-2 rounded border border-accent p-4">
          <p className="text-sm">Save this secret now. It is shown once.</p>
          <code className="block break-all select-all text-xs">{d.secret}</code>
          <button className={DURABLE_BUTTON} onClick={() => d.setSecret('')}>
            Dismiss secret
          </button>
        </div>
      )}
    </>
  );
}

/** Sessions still running or reserved, and those needing reconciliation. */
export const sessionCounts = (sessions: DurableSession[]) => ({
  active: sessions.filter((s) => !ENDED_STATES.includes(s.state)).length,
  pending: sessions.filter((s) => RECONCILE_STATES.includes(s.state)).length,
});

/** Active, needs-reconciliation and estimated-cost figures. */
export function DurableSummary({ data }: { /** The durable overview. */ data: Overview }) {
  const { active, pending } = sessionCounts(data.sessions);
  return (
    <div className="grid grid-cols-3 divide-x divide-border border-y border-border py-4">
      <div>
        <p className="text-xs text-text-dim">Active and reserved</p>
        <p className="mt-1 text-3xl tabular-nums">{active}</p>
      </div>
      <div className="pl-5">
        <p className="text-xs text-text-dim">Needs reconciliation</p>
        <p className={`mt-1 text-3xl tabular-nums ${pending ? 'text-yellow' : ''}`}>{pending}</p>
      </div>
      <div className="pl-5">
        <p className="text-xs text-text-dim">Estimated browser cost</p>
        <p className="mt-1 text-3xl tabular-nums">${(data.project.costUsd || 0).toFixed(COST_DECIMALS)}</p>
      </div>
    </div>
  );
}
