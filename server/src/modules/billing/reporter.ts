/**
 * Reports each paying person's usage to Stripe's meters. It sends the running
 * total's growth since the last report, never an event per use, and records
 * each meter's new total as soon as it is sent, so a failure part way through
 * never sends the same growth twice. Each event is named after the total it
 * brings the meter to, so a retry Stripe already has is dropped.
 *
 * ponytail: every replica reports; with more than one server replica, pick one
 * (a lease in the control store) before scaling the server out.
 */
import { createHash } from 'node:crypto';
import { IDENTIFIER_CHARS, METERS, REPORTABLE, type Meter } from './constants.ts';
import type { Stripe } from './stripe.ts';
import type { Subscription } from './repository.ts';

/** What reporting needs. */
export type ReporterDeps = {
  /** Every subscription. */
  all(): Promise<Subscription[]>;
  /** Records what has been reported for a person. */
  saveReported(userId: string, reported: Record<string, unknown>): Promise<void>;
  /** A person's billed usage from one ISO time to another. */
  usageSince(userId: string, since: string, until?: string): Promise<Record<string, number>>;
  /** Stripe. */
  stripe: Stripe;
};

/** What has been reported for one period: the period, and each meter's total sent. */
type Reported = Record<string, any> & {
  /** The period's start. */
  period: string;
};

/** A stable id for "this person's meter reached this total in this period". */
const identifier = (userId: string, meter: Meter, period: string, total: number) =>
  createHash('sha256').update(`${userId}:${meter.event}:${period}:${total}`).digest('hex').slice(0, IDENTIFIER_CHARS);

/** Sends paying people's usage to Stripe. */
export class UsageReporter {
  /** What reporting reads and writes. */
  declare deps: ReporterDeps;

  /** Everything goes through `deps`. */
  constructor(deps: ReporterDeps) {
    this.deps = deps;
  }

  /** Reports everyone whose usage is billed; one person's failure leaves the rest to go out. */
  async report() {
    const rows = (await this.deps.all()).filter(
      (r) => REPORTABLE.has(r.status) && r.stripe_customer_id && r.period_start,
    );
    for (const row of rows)
      await this.reportOne(row).catch((e) => console.error(`[billing] usage report failed: ${e.message}`));
  }

  /** Settles a period that has just ended, then reports the current one. */
  async reportOne(row: Subscription) {
    const reported: Reported = row.reported?.period ? row.reported : { period: row.period_start };
    const current = reported.period === row.period_start ? reported : await this.settle(row, reported);
    await this.reportPeriod(row, current, await this.deps.usageSince(row.user_id, row.period_start));
  }

  /** Sends what an ended period grew since its last report, then starts the new period from nothing. */
  async settle(row: Subscription, ended: Reported) {
    await this.reportPeriod(row, ended, await this.deps.usageSince(row.user_id, ended.period, row.period_start));
    const fresh = { period: row.period_start };
    await this.deps.saveReported(row.user_id, fresh);
    return fresh;
  }

  /** Brings every meter up to the period's usage, one at a time. */
  async reportPeriod(row: Subscription, reported: Reported, used: Record<string, number>) {
    let now = reported;
    for (const m of METERS) now = await this.advance(row, now, m, Math.floor((used[m.field] || 0) / m.per));
    return now;
  }

  /** Sends one meter's growth to `total`, if any, and records it at once. */
  async advance(row: Subscription, reported: Reported, meter: Meter, total: number) {
    const already = Number(reported[meter.event]) || 0;
    if (total <= already) return reported;
    const payload = { stripe_customer_id: row.stripe_customer_id, value: total - already };
    const id = identifier(row.user_id, meter, reported.period, total);
    await this.deps.stripe.post('/billing/meter_events', { event_name: meter.event, identifier: id, payload });
    const next = { ...reported, [meter.event]: total };
    await this.deps.saveReported(row.user_id, next);
    return next;
  }
}
