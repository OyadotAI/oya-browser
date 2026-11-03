/**
 * What the admin page asks for: the overview, one person looked up by email,
 * and the self-hosted licenses (issue, list, revoke). Admins only; the routes
 * check that before anything here runs.
 */
import { notFound } from '../../platform/errors.ts';
import * as license from '../../platform/license/index.ts';
import { monthStart, standingOf } from '../billing/standing.ts';
import * as billing from '../billing/repository.ts';
import { grantsFor, overrideFor } from '../billing/index.ts';
import { impersonatedUser, keysOf, mintImpersonation } from '../auth/service.ts';
import { HttpError } from '../../platform/errors.ts';
import { Status } from '../../platform/http-status.ts';
import { audit } from '../../platform/audit.ts';
import { isAdmin } from './access.ts';
import { registry } from '../browsers/registry.ts';
import { SERVER_RUN } from '../browsers/connection/constants.ts';
import * as repo from './repository.ts';
import * as view from './overview.ts';
import * as growth from './growth.ts';
import { adminPeriods } from './periods.ts';
import { ADMIN_OVERVIEW_VERSION, ADMIN_ROWS_SHOWN, DAYS_SHOWN } from './constants.ts';

export { keelOverview } from './keel.ts';

/** Account stocks and completed-date signup counts share one captured snapshot. */
function accountSummary(profiles, subscriptions, now: number) {
  const period = adminPeriods(now).usage;
  const signups = view.perDay(profiles, 'created_at', period.from, period.end);
  return { total: profiles.length, signups, ...view.plans(subscriptions), statuses: view.planStatuses(subscriptions) };
}

/** People, signups and plans. */
async function accounts(now: number) {
  const [profiles, subscriptions] = await Promise.all([repo.profiles(), billing.all()]);
  return { profiles, summary: accountSummary(profiles, subscriptions, now) };
}

/** This month's heaviest people by cloud hours and by agent steps, from usage rows reaching back at least that far. */
function heaviest(now: number, profiles: Record<string, any>[], usage: Record<string, any>[]) {
  const people = view.perPerson(
    usage.filter((r) => String(r.hour) >= monthStart(now) && String(r.hour) <= new Date(now).toISOString()),
  );
  const emails = new Map(profiles.map((p) => [String(p.id), String(p.email)]));
  return { cloud: view.top(people, 'cloud_seconds', emails), steps: view.top(people, 'agent_steps', emails) };
}

/** Self-hosted installs and the downloads of the last days. */
function reach(installs, now: number) {
  const list = [...installs]
    .sort((a, b) => String(b.last_seen).localeCompare(String(a.last_seen)))
    .slice(0, ADMIN_ROWS_SHOWN);
  return { ...view.installSummary(installs, now, license.SELF_HOST_FREE_CAP), list };
}

/** The earliest usage the overview needs: this month's start or the first day shown, whichever is sooner. */
const usageFrom = (now: number) => [monthStart(now), adminPeriods(now).usage.from].sort()[0];

/** The day by day series, its trends, the people reached, and revenue as Stripe has it. */
async function trends(now: number, rows: growth.Sources) {
  const keys = await repo.apiKeys();
  const active = growth.activePeople(rows.usage, keys);
  const days = growth.daily({ ...rows, keys }, now, DAYS_SHOWN, active);
  return { days, ...growth.trends(days, growth.distinctComparison(active, now)), reach: growth.reach(active, now) };
}

/** Independent database reads capture the complete installation population before presentation limits. */
function overviewSources(now: number) {
  return Promise.all([
    accounts(now),
    repo.usageSince(usageFrom(now)),
    repo.allInstalls(),
    repo.downloadsSince(adminPeriods(now).usage.from),
  ]);
}

/** Everything the overview shows, independent of Stripe. */
export async function overview(now = Date.now()) {
  const [{ profiles, summary }, usage, installs, rawDownloads] = await overviewSources(now);
  const periods = adminPeriods(now);
  const downloads = rawDownloads.filter((row) => String(row.day) <= periods.usage.to);
  const activity = await trends(now, { profiles, usage, installs, downloads, keys: [] });
  return overviewResult({ profiles, summary, usage, installs, downloads, growth: activity }, periods, now);
}

/** The local connection registry describes this responding server only. */
function localFleet(now: number) {
  return {
    ...view.fleet([...registry.browsers.values()], SERVER_RUN),
    scope: 'responding_server',
    generated_at: new Date(now).toISOString(),
  };
}

/** Customer rankings and install summaries use full source populations. */
function activityResult(rows, now: number) {
  return {
    accounts: rows.summary,
    top: heaviest(now, rows.profiles, rows.usage),
    installs: reach(rows.installs, now),
    downloads: rows.downloads,
    growth: rows.growth,
  };
}

/** Versioned scopes replace ambiguous private administrative totals. */
function overviewResult(rows, periods, now: number) {
  return {
    version: ADMIN_OVERVIEW_VERSION,
    generated_at: new Date(now).toISOString(),
    periods,
    ...activityResult(rows, now),
    fleet: localFleet(now),
  };
}

/** Existing allowance accounting is hourly, with adjustments tied to the same standing boundary. */
async function allowance(id: string, since: string) {
  return {
    used: await billing.usageSince(id, since),
    override: await overrideFor(id),
    grants: await grantsFor(id, since),
  };
}

/** Lookup metadata describes the measurement boundary without exposing whole keys. */
async function customerResult(profile, subscription, standing, now: number) {
  const keys = (await keysOf(String(profile.id))).map(keyShown);
  const measured = { generated_at: new Date(now).toISOString(), granularity: 'hourly_buckets' };
  const details = { keys, ...(await allowance(String(profile.id), standing.since)), ...measured };
  return { profile, standing, subscription, ...details };
}

/** One person by email: allowance-period usage and key prefixes, never whole keys. */
export async function lookup(email: string, now = Date.now()) {
  const profile = await repo.profileByEmail(email.trim().toLowerCase());
  if (!profile) throw notFound('Account');
  const id = String(profile.id),
    subscription = await billing.find(id);
  return customerResult(profile, subscription, standingOf(id, subscription, now), now);
}

/** A one-hour "Login as" token for one customer; never for another admin. Issuing it is audited. `find` is the user lookup, a seam for tests. */
export async function impersonate(id: string, admin, req?, find = impersonatedUser) {
  const user = await find(id);
  if (isAdmin(user)) throw new HttpError(Status.FORBIDDEN, 'Cannot log in as another admin', { code: 'forbidden' });
  const token = mintImpersonation(user.id, admin.id);
  audit({ action: 'admin.impersonate.issued', actorUser: admin.id, targetType: 'user', targetId: user.id, req });
  return { impersonate_token: token, impersonated_user_id: user.id, email: user.email };
}

/** A key as the admin page shows it: its prefix and label, never the key. */
const keyShown = (k) => ({
  prefix: k.key_prefix,
  label: k.label,
  created_at: k.created_at,
  last_used_at: k.last_used_at,
});

/** Issues a license: signed by the licensing module, recorded here without its key, and the key answered once. */
export async function issue(request: Record<string, unknown>, by: string) {
  const { license: issued, token } = license.mint(request);
  const { id, licensee, maxConcurrent: max_concurrent, expiresAt: expires_at } = issued;
  const row = { id, licensee, max_concurrent, expires_at };
  await repo.saveLicense({ ...row, created_at: new Date().toISOString(), created_by: by, revoked_at: null });
  return { ...row, key: token };
}

/** Revokes a license: the install falls back to the free cap at its next ping. */
export async function revoke(id: string) {
  const row = await repo.license(id);
  if (!row) throw notFound('License');
  if (row.revoked_at) return row;
  const revoked = { ...row, revoked_at: new Date().toISOString() };
  await repo.saveLicense(revoked);
  return revoked;
}

/** Whether an issued license has been revoked. */
export async function isRevoked(id: string | null) {
  return Boolean(id && (await repo.license(id))?.revoked_at);
}

/** Every license issued, newest first. */
export const listLicenses = () => repo.licenses();
