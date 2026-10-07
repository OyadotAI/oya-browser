/**
 * The billing page's sections: the plan, this period's use against what it
 * includes, the next invoice line by line, and the invoices already issued.
 */
'use client';

import { Section, Table } from '@/components/admin/parts';
import { PERCENT, SALES_EMAIL, UPGRADES } from './constants';
import { dateOf, figure, money, share, usageRows } from './model';
import type { BillingPageState, Invoice, Upcoming } from './use-billing-page';
import type { Billing } from './use-plan';

/** A button that sends the person to Stripe. */
function Go({
  onClick,
  children,
  primary,
}: {
  /** Sends them. */ onClick: () => void;
  /** Its text. */ children: React.ReactNode;
  /** The accent style. */ primary?: boolean;
}) {
  const look = primary ? 'bg-accent text-white' : 'border border-border text-text hover:bg-text/5';
  return (
    <button onClick={onClick} className={`rounded-md px-3 py-1.5 text-xs font-medium ${look}`}>
      {children}
    </button>
  );
}

/** The plan, its period, and upgrade or manage. */
export function PlanCard({ b, s }: { /** The plan. */ b: Billing; /** The page's state. */ s: BillingPageState }) {
  const late = b.status === 'past_due';
  return (
    <Section title="Plan">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <span className="text-2xl font-semibold capitalize text-text">{b.plan}</span>
        <span className="text-xs text-text-dim">
          {b.until ? `Renews ${dateOf(b.until)}` : `This month since ${dateOf(b.since)}`}
        </span>
      </div>
      {b.status === 'admin' && (
        <p className="text-xs text-text-muted">
          Plan access granted by support. Any existing paid subscription remains separate.
        </p>
      )}
      {late && (
        <p className="text-xs text-red-400">Your last payment failed. Update your card to keep using the cloud.</p>
      )}
      <div className="flex flex-wrap gap-2">
        {!(b.canManage ?? b.plan !== 'free') ? (
          UPGRADES.map((p) => (
            <Go key={p.id} primary onClick={() => void s.upgrade(p.id)}>{`${p.label} ${p.price}`}</Go>
          ))
        ) : (
          <Go onClick={() => void s.manage()}>Change card, see receipts or cancel</Go>
        )}
      </div>
      {s.error && <p className="text-xs text-red-400">{s.error}</p>}
      <p className="text-[11px] text-text-dim">
        Need more?{' '}
        <a className="underline" href={`mailto:${SALES_EMAIL}`}>
          {SALES_EMAIL}
        </a>
      </p>
    </Section>
  );
}

/** Each thing the plan counts, used against what it includes. */
export function Usage({ b }: { /** The plan. */ b: Billing }) {
  const rows = usageRows(b.used, b.included).map((r) => [
    r.label,
    `${figure(r.used, r.unit)}${r.included === null ? '' : ` of ${figure(r.included, r.unit)}`} ${r.unit === 'USD' ? '' : r.unit}`,
    r.included === null ? b.included?.overage ? 'billed as used' : '—' : <Bar key={r.label} value={share(r)} />,
  ]);
  return (
    <Section title="This period’s use">
      <Table head={['', 'Used', '']} rows={rows} />
    </Section>
  );
}

/** How much of an allowance is used. */
function Bar({ value }: { /** 0 to 1. */ value: number }) {
  return (
    <div className="h-1.5 w-32 rounded-full bg-text/10">
      <div
        className={`h-1.5 rounded-full ${value >= 1 ? 'bg-red-400' : 'bg-accent'}`}
        style={{ width: `${Math.round(value * PERCENT)}%` }}
      />
    </div>
  );
}

/** The next invoice, line by line. */
export function NextInvoice({ u }: { /** The next invoice. */ u: Upcoming }) {
  const rows = u.lines.map((l) => [l.description, l.quantity ?? '', money(l.amount, u.currency)]);
  return (
    <Section title={`Next invoice: ${money(u.total, u.currency)} on ${dateOf(u.date)}`}>
      <Table head={['Item', 'Quantity', 'Amount']} rows={rows} />
    </Section>
  );
}

/** A link out, opened in a new tab. */
const Out = ({ href, children }: { /** Where. */ href: string | null; /** Its text. */ children: string }) =>
  href ? (
    <a href={href} target="_blank" rel="noopener noreferrer" className="text-accent hover:underline">
      {children}
    </a>
  ) : null;

/** The invoices already issued. */
export function History({ invoices }: { /** Issued invoices. */ invoices: Invoice[] }) {
  const rows = invoices.map((i) => [
    dateOf(i.created),
    i.number || '—',
    i.status,
    money(i.total, i.currency),
    <span key={i.id} className="flex gap-3">
      <Out href={i.url}>View</Out>
      <Out href={i.pdf}>PDF</Out>
    </span>,
  ]);
  return (
    <Section title="Invoices">
      <Table head={['Date', 'Number', 'Status', 'Total', '']} rows={rows} />
    </Section>
  );
}
