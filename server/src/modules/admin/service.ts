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
import { revenue } from './revenue.ts';
import { DAYS_SHOWN, INSTALLS_SHOWN, MS_PER_DAY } from './constants.ts';

/** People, signups and plans. */
async function accounts(now: number) {
  const [profiles, subscriptions] = await Promise.all([repo.profiles(), billing.all()]);
  const signups = view.perDay(profiles, 'created_at', view.daysAgo(now, DAYS_SHOWN));
  return { profiles, summary: { total: profiles.length, signups, ...view.plans(subscriptions) } };
}

/** This month's heaviest people by cloud hours and by agent steps, from usage rows reaching back at least that far. */
function heaviest(now: number, profiles: Record<string, any>[], usage: Record<string, any>[]) {
  const people = view.perPerson(usage.filter((r) => String(r.hour) >= monthStart(now)));
  const emails = new Map(profiles.map((p) => [String(p.id), String(p.email)]));
  return { cloud: view.top(people, 'cloud_seconds', emails), steps: view.top(people, 'agent_steps', emails) };
}

/** Self-hosted installs and the downloads of the last days. */
async function reach(now: number) {
  const [installs, downloads] = await Promise.all([
    repo.installs(INSTALLS_SHOWN),
    repo.downloadsSince(view.daysAgo(now, DAYS_SHOWN)),
  ]);
  return { installs: { ...view.installSummary(installs, now, license.SELF_HOST_FREE_CAP), list: installs }, downloads };
}

/** The earliest usage the overview needs: this month's start or the first day shown, whichever is sooner. */
const usageFrom = (now: number) => [monthStart(now), view.daysAgo(now, DAYS_SHOWN)].sort()[0];

/** The day by day series, its trends, the people reached, and revenue as Stripe has it. */
async function trends(now: number, rows: Pick<growth.Sources, 'profiles' | 'usage' | 'downloads'>) {
  const since = now - DAYS_SHOWN * MS_PER_DAY;
  const [keys, installs, money] = await Promise.all([repo.apiKeys(), repo.allInstalls(), revenue(since)]);
  const active = growth.activePeople(rows.usage, keys);
  const days = growth.daily({ ...rows, keys, installs, payments: money.payments }, now, DAYS_SHOWN, active);
  const { payments: _, ...stripe } = money;
  return { growth: { days, ...growth.trends(days), reach: growth.reach(active, now) }, revenue: stripe };
}

/** Everything the overview shows. */
export async function overview(now = Date.now()) {
  const [{ profiles, summary }, usage] = await Promise.all([accounts(now), repo.usageSince(usageFrom(now))]);
  const fleet = view.fleet([...registry.browsers.values()], SERVER_RUN);
  const seen = await reach(now);
  const more = await trends(now, { profiles, usage, downloads: seen.downloads });
  return { accounts: summary, top: heaviest(now, profiles, usage), ...seen, fleet, ...more };
}

/** One person by email: plan, usage this period and keys (never the keys themselves). */
export async function lookup(email: string, now = Date.now()) {
  const profile = await repo.profileByEmail(email.trim().toLowerCase());
  if (!profile) throw notFound('Account');
  const id = String(profile.id);
  const subscription = await billing.find(id);
  const standing = standingOf(id, subscription, now);
  const keys = (await keysOf(id)).map(keyShown);
  const adjustments = { override: await overrideFor(id), grants: await grantsFor(id, standing.since) };
  return { profile, standing, subscription, used: await billing.usageSince(id, standing.since), keys, ...adjustments };
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
