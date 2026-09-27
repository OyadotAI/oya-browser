/**
 * The account dialog's plan: what you are on, how much of it you used this
 * month, and a way to upgrade or manage it. Hidden on a self-hosted server.
 * Its state lives in `use-plan.ts`.
 */
'use client';

import Link from 'next/link';
import { BILLING_PAGE, SALES_EMAIL, UPGRADES } from './constants';
import { hours, usePlan } from './use-plan';

/** One line of use against an allowance. */
function Meter({
  label,
  used,
  of,
}: {
  /** What is counted. */ label: string;
  /** Used so far. */ used: number;
  /** Included. */ of: number;
}) {
  return (
    <div className="flex items-baseline justify-between gap-4">
      <span className="text-xs text-text-dim">{label}</span>
      <span className="text-xs text-text">
        {used.toLocaleString()} / {of.toLocaleString()}
      </span>
    </div>
  );
}

/** Your plan, your use this month, and upgrade or manage. */
export default function PlanSection({ open }: { /** Whether the dialog is showing. */ open: boolean }) {
  const { billing, error, upgrade, manage } = usePlan(open);
  if (!billing) return null;
  const free = billing.plan === 'free';
  return (
    <div id="billing" className="flex flex-col gap-2 rounded-md border border-border bg-bg-elevated/40 p-3">
      <div className="flex items-baseline justify-between">
        <span className="text-xs font-medium text-text-muted">Plan</span>
        <span className="text-xs font-medium capitalize text-text">
          {billing.plan}
          {billing.status === 'past_due' ? ' (payment failed)' : ''}
        </span>
      </div>
      <Meter label="Cloud hours" used={hours(billing.used?.cloud_seconds)} of={hours(billing.included?.cloudSeconds)} />
      <Meter label="Agent steps" used={billing.used?.agent_steps || 0} of={billing.included?.steps || 0} />
      <div className="flex flex-wrap gap-2 pt-1">
        {free ? (
          UPGRADES.map((p) => (
            <button
              key={p.id}
              onClick={() => void upgrade(p.id)}
              className="rounded-md bg-accent px-3 py-1.5 text-xs font-medium text-white"
            >
              {p.label} {p.price}
            </button>
          ))
        ) : (
          <button
            onClick={() => void manage()}
            className="rounded-md border border-border px-3 py-1.5 text-xs text-text hover:bg-text/5"
          >
            Manage billing
          </button>
        )}
      </div>
      <Link href={BILLING_PAGE} className="text-xs text-accent hover:underline">
        Usage, invoices and history
      </Link>
      <p className="text-[11px] text-text-dim">
        Need more?{' '}
        <a className="underline" href={`mailto:${SALES_EMAIL}`}>
          {SALES_EMAIL}
        </a>
      </p>
      {error && <p className="text-xs text-red-400">{error}</p>}
    </div>
  );
}
