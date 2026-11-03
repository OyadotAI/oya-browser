/**
 * Customers: the search box in the page's top bar, and the customers tab with
 * the person found (plan, usage this period, keys by prefix only, Stripe,
 * and Login as) above the heaviest users. Its state lives in `use-lookup.ts`.
 */
'use client';

import { useState } from 'react';
import { Search } from 'lucide-react';
import { BillingAdjustment } from './billing-adjustment';
import { Section, Table, Tile } from './parts';
import { dayOf, hours, utcTime } from './model';
import { TopUsers } from './overview-sections';
import type { LookupState } from './use-lookup';
import type { AdminState } from './use-admin';
import type { Found, Overview } from './types';

/** Where a Stripe customer opens in the Stripe dashboard. */
const stripeCustomer = (id: string) => `https://dashboard.stripe.com/customers/${encodeURIComponent(id)}`;

/** Find a customer by email, from anywhere on the page; `onSearch` runs as a search starts. */
export function SearchBox({
  l,
  onSearch,
}: {
  /** The lookup. */ l: LookupState;
  /** Runs on each search. */ onSearch: () => void;
}) {
  const go = () => {
    onSearch();
    void l.search();
  };
  return (
    <div className="flex h-9 w-full max-w-md items-center gap-2 rounded-md border border-border bg-bg-card px-2.5 focus-within:border-accent">
      <Search className="h-3.5 w-3.5 shrink-0 text-text-dim" aria-hidden />
      <input
        value={l.email}
        onChange={(e) => l.setEmail(e.target.value)}
        onKeyDown={(e) => e.key === 'Enter' && go()}
        placeholder="email@example.com"
        aria-label="Customer email"
        className="h-full min-w-0 flex-1 bg-transparent text-sm text-text outline-none placeholder:text-text-dim"
      />
      <button onClick={go} className="text-xs font-medium text-text-muted hover:text-text">
        Look up
      </button>
    </div>
  );
}

/** Login as, and why it was refused. */
function LoginAs({
  p,
  loginAs,
}: {
  /** The person. */ p: Found;
  /** Opens their dashboard. */ loginAs: AdminState['loginAs'];
}) {
  const [error, setError] = useState('');
  return (
    <div className="flex flex-col items-end gap-1">
      <button
        onClick={() => loginAs(p.profile.id).catch((e) => setError(e instanceof Error ? e.message : String(e)))}
        className="h-8 rounded-md bg-accent px-3 text-xs font-semibold text-accent-foreground hover:bg-accent-hover"
      >
        Login as
      </button>
      {error && <span className="text-xs text-red">{error}</span>}
    </div>
  );
}

/** Key inventory exposes prefixes and usage timestamps without displaying credentials. */
function CustomerKeys({ p }: { /** Selected customer's bounded key display. */ p: Found }) {
  return (
    <div className="space-y-2">
      <h3 className="text-xs font-semibold text-text">API keys · prefixes only</h3>
      <Table
        head={['Key prefix', 'Label', 'Created', 'Last used']}
        empty="No API keys created for this customer."
        rows={p.keys.map((key) => [`${key.prefix}…`, key.label, dayOf(key.created_at), dayOf(key.last_used_at)])}
      />
    </div>
  );
}

/** The person found: who, their numbers, Stripe, their keys, and Login as. */
function FoundPerson({
  p,
  loginAs,
  s,
  changed,
}: {
  /** Admin mutations. */ s: AdminState;
  /** Refreshes the customer. */ changed: () => void;
  /** What the server answered. */ p: Found;
  /** Opens their dashboard. */ loginAs: AdminState['loginAs'];
}) {
  const customer = p.subscription?.stripe_customer_id;
  return (
    <div className="flex flex-col gap-4 rounded-lg border border-border bg-bg-card p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="flex flex-col gap-1">
          <span className="font-display text-xl text-text">{p.profile.email}</span>
          {customer && (
            <a
              className="text-xs text-accent hover:underline"
              href={stripeCustomer(customer)}
              target="_blank"
              rel="noopener noreferrer"
            >
              Open in Stripe
            </a>
          )}
        </div>
        <LoginAs p={p} loginAs={loginAs} />
      </div>
      <p className="text-xs text-text-muted">
        Allowance-period usage · {utcTime(p.standing.since)} through {utcTime(p.generated_at)}. Stored in hourly
        buckets; boundary hours may be partial.
      </p>
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        <Tile label="Plan" value={`${p.standing.plan}${p.standing.status ? ` (${p.standing.status})` : ''}`} />
        <Tile label="Cloud hours" value={hours(p.used.cloud_seconds)} />
        <Tile label="Agent steps" value={p.used.agent_steps || 0} />
        <Tile label="Since" value={dayOf(p.standing.since)} />
      </div>
      <CustomerKeys p={p} />
      <BillingAdjustment key={p.profile.id} p={p} s={s} changed={changed} />
    </div>
  );
}

/** The customers tab: who was looked up (or how to), and the heaviest users. */
export function CustomersTab({
  s,
  l,
  o,
}: {
  /** The page's state. */ s: AdminState;
  /** The lookup. */ l: LookupState;
  /** Independently refreshed rankings. */ o: Overview | null;
}) {
  return (
    <div className="flex flex-col gap-6">
      <Section title="Selected customer">
        {l.pending && (
          <p role="status" className="text-xs text-text-muted">
            Looking up {l.pending}…
          </p>
        )}
        {l.error && <p className="text-xs text-red">{l.error}</p>}
        {l.found ? (
          <FoundPerson p={l.found} loginAs={s.loginAs} s={s} changed={() => void l.refresh()} />
        ) : (
          !l.error &&
          !l.pending && (
            <p className="text-xs text-text-dim">Search by email in the bar above, or pick someone below.</p>
          )
        )}
      </Section>
      {o && <TopUsers o={o} pick={(email) => void l.search(email)} />}
    </div>
  );
}
