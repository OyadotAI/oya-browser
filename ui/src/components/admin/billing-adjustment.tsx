/** Compact support controls, with the charge and expiry semantics beside each action. */
'use client';
import type { InputHTMLAttributes } from 'react';
import { useBillingAdjustment } from './use-billing-adjustment';
import { BILLING_REASON_MAX, CREDIT_DECIMALS, CREDIT_STEP, HOURS_STEP, MICRO_USD_PER_DOLLAR } from './constants';
import { dayOf, hours } from './model';
import { Table } from './parts';
import type { Found } from './types';
import type { AdminState } from './use-admin';

/** An accessible field that follows the existing support console's compact styling. */
function Field({ label, ...input }: { /** Visible label. */ label: string } & InputHTMLAttributes<HTMLInputElement>) {
  return (
    <label className="flex flex-col gap-1 text-xs text-text-muted">
      {label}
      <input
        {...input}
        className="h-9 rounded-md border border-border bg-bg px-2 text-sm text-text focus:border-accent focus:outline-none"
      />
    </label>
  );
}

/** The form state shared by the two sections. */
type Form = ReturnType<typeof useBillingAdjustment>;

/** Explicit access override selection, including restoration to the real subscription. */
function PlanControl({ f }: { /** Pending state and fields. */ f: Form }) {
  return (
    <div className="flex flex-col gap-2">
      <h3 className="text-sm font-semibold text-text">Plan access</h3>
      <p className="text-xs text-text-muted">
        Override access until removed. Stripe subscriptions and charges stay unchanged. Complimentary access stops at
        the included allowance. Hosted AI keeps the Free credit allowance; grant additional AI credits when needed.
      </p>
      <div className="flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs text-text-muted">
          Access plan
          <select
            value={f.fields.plan}
            onChange={(e) => f.set({ plan: e.target.value })}
            className="h-9 rounded-md border border-border bg-bg px-2 text-sm text-text"
          >
            <option value="">Follow subscription</option>
            <option value="free">Free</option>
            <option value="developer">Developer</option>
            <option value="startup">Startup</option>
          </select>
        </label>
        <button
          type="button"
          disabled={f.busy || !f.fields.reason.trim()}
          onClick={() => void f.submit('plan')}
          className="h-9 rounded-md border border-border px-3 text-xs font-semibold text-text hover:border-accent disabled:opacity-50"
        >
          Save plan access
        </button>
      </div>
    </div>
  );
}

/** Adds allowance for this period, with dollar units and billing limitations made explicit. */
function GrantControl({
  f,
  used,
}: {
  /** Pending state and fields. */ f: Form;
  /** Steps used this period. */ used: number;
}) {
  return (
    <div className="flex flex-col gap-2">
      <h3 className="text-sm font-semibold text-text">Extra allowance</h3>
      <p className="text-xs text-text-muted">
        Adds to this period only; unused grants expire when it ends. Credits cover hosted AI cost in USD. Usage already
        reported to Stripe is not refunded.
      </p>
      <StepReset f={f} used={used} />
      <div className="flex flex-wrap items-end gap-2">
        <Field
          label="Extra agent steps"
          type="number"
          min={0}
          step={1}
          value={f.fields.steps}
          onChange={(e) => f.set({ steps: e.target.value })}
        />
        <Field
          label="Extra cloud hours"
          type="number"
          min={0}
          step={HOURS_STEP}
          value={f.fields.hours}
          onChange={(e) => f.set({ hours: e.target.value })}
        />
        <Field
          label="Hosted AI credits (USD)"
          type="number"
          min={0}
          step={CREDIT_STEP}
          value={f.fields.credits}
          onChange={(e) => f.set({ credits: e.target.value })}
        />
        <GrantButton f={f} />
      </div>
    </div>
  );
}

/** Billing controls and the current period's grant history for the selected customer. */
export function BillingAdjustment({
  p,
  s,
  changed,
}: {
  /** Selected customer. */ p: Found;
  /** Authenticated admin actions. */ s: AdminState;
  /** Refreshes the customer after saving. */ changed: () => void;
}) {
  const f = useBillingAdjustment(p, s, changed);
  return (
    <section aria-label="Manage customer billing" className="flex flex-col gap-4 border-t border-border pt-4">
      <Field
        label="Reason for adjustment"
        maxLength={BILLING_REASON_MAX}
        value={f.fields.reason}
        onChange={(e) => f.set({ reason: e.target.value })}
        placeholder="e.g. Trial extension or support credit"
        disabled={f.busy}
      />
      <fieldset disabled={f.busy} className="grid min-w-0 gap-5 md:grid-cols-2">
        <PlanControl f={f} />
        <GrantControl f={f} used={p.used.agent_steps || 0} />
      </fieldset>
      {f.busy && (
        <p role="status" className="text-xs text-text-muted">
          Saving…
        </p>
      )}
      {f.message && (
        <p role="status" className="text-xs text-accent">
          {f.message}
        </p>
      )}
      {f.error && (
        <p role="alert" className="text-xs text-red">
          {f.error}
        </p>
      )}
      <GrantHistory p={p} />
    </section>
  );
}

/** Current-period grants with their reason and exact displayed units. */
function GrantHistory({ p }: { /** Selected customer. */ p: Found }) {
  if (!p.grants?.length) return null;
  const rows = p.grants.map((g) => [
    dayOf(g.created_at),
    hours(g.cloud_seconds),
    (g.hosted_llm_microusd / MICRO_USD_PER_DOLLAR).toFixed(CREDIT_DECIMALS),
    String(g.agent_steps || 0),
    g.reason,
  ]);
  return <Table head={['Granted', 'Cloud hours', 'AI credit (USD)', 'Agent steps', 'Reason']} rows={rows} />;
}

/** Prepares an explicit grant that restores spent steps while retaining the usage ledger. */
function StepReset({ f, used }: { /** Editable grant. */ f: Form; /** Metered steps this period. */ used: number }) {
  return (
    <div className="text-xs text-text-muted">
      <button
        type="button"
        disabled={f.busy || !used}
        className="underline disabled:opacity-50"
        onClick={() => f.set({ steps: String(used) })}
      >
        Restore used steps
      </button>
      <span>
        {' '}
        fills {used.toLocaleString()} extra steps. Add a reason and choose Grant allowance to apply. Usage history stays
        intact.
      </span>
    </div>
  );
}

/** Keeps the grant disabled until there is an attributed positive allowance. */
function GrantButton({ f }: { /** Form state. */ f: Form }) {
  return (
    <button
      type="button"
      disabled={
        f.busy ||
        !f.fields.reason.trim() ||
        !(Number(f.fields.hours) > 0 || Number(f.fields.credits) > 0 || Number(f.fields.steps) > 0)
      }
      onClick={() => void f.submit('grant')}
      className="h-9 rounded-md bg-accent px-3 text-xs font-semibold text-accent-foreground hover:bg-accent-hover disabled:opacity-50"
    >
      Grant allowance
    </button>
  );
}
