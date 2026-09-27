/**
 * Look a person up by email: their plan, what they used this period, their
 * keys (prefixes only) and their Stripe customer. Its state lives in `use-lookup.ts`.
 */
'use client';

import { Section, Table, Tile } from './parts';
import { dayOf, hours } from './model';
import { useLookup } from './use-lookup';
import type { AdminState } from './use-admin';
import type { Found } from './types';

/** Where a Stripe customer opens in the Stripe dashboard. */
const stripeCustomer = (id: string) => `https://dashboard.stripe.com/customers/${encodeURIComponent(id)}`;

/** The person found. */
function FoundPerson({ p }: { /** What the server answered. */ p: Found }) {
  const customer = p.subscription?.stripe_customer_id;
  return (
    <div className="flex flex-col gap-2">
      <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
        <Tile label="Plan" value={`${p.standing.plan}${p.standing.status ? ` (${p.standing.status})` : ''}`} />
        <Tile label="Cloud hours" value={hours(p.used.cloud_seconds)} />
        <Tile label="Agent steps" value={p.used.agent_steps || 0} />
        <Tile label="Since" value={dayOf(p.standing.since)} />
      </div>
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
      <Table
        head={['Key', 'Label', 'Created', 'Last used']}
        rows={p.keys.map((k) => [`${k.prefix}…`, k.label, dayOf(k.created_at), dayOf(k.last_used_at)])}
      />
    </div>
  );
}

/** Search by email, and what was found. */
export default function Lookup({ s }: { /** The page's state. */ s: AdminState }) {
  const l = useLookup(s.lookup);
  return (
    <Section title="Look up a person">
      <div className="flex gap-2">
        <input
          value={l.email}
          onChange={(e) => l.setEmail(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && void l.search()}
          placeholder="email@example.com"
          className="h-8 flex-1 rounded-md border border-border bg-bg px-2 text-sm text-text outline-none focus:border-accent"
        />
        <button
          onClick={() => void l.search()}
          className="h-8 rounded-md border border-border px-3 text-xs text-text hover:bg-text/5"
        >
          Look up
        </button>
      </div>
      {l.error && <p className="text-xs text-red-400">{l.error}</p>}
      {l.found && <FoundPerson p={l.found} />}
    </Section>
  );
}
