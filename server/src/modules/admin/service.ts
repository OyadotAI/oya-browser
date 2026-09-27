/**
 * What the admin page asks for: the overview, one person looked up by email,
 * and the self-hosted licenses (issue, list, revoke). Admins only; the routes
 * check that before anything here runs.
 */
import { notFound } from '../../platform/errors.ts';
import * as license from '../../platform/license/index.ts';
import { monthStart, standingOf } from '../billing/standing.ts';
import * as billing from '../billing/repository.ts';
import { keysOf } from '../auth/service.ts';
import { registry } from '../browsers/registry.ts';
import { SERVER_RUN } from '../browsers/connection/constants.ts';
import * as repo from './repository.ts';
import * as view from './overview.ts';
import { DAYS_SHOWN, INSTALLS_SHOWN } from './constants.ts';

/** People, signups and plans. */
async function accounts(now: number) {
  const [profiles, subscriptions] = await Promise.all([repo.profiles(), billing.all()]);
  const signups = view.perDay(profiles, 'created_at', view.daysAgo(now, DAYS_SHOWN));
  return { profiles, summary: { total: profiles.length, signups, ...view.plans(subscriptions) } };
}

/** This month's heaviest people by cloud hours and by agent steps. */
async function heaviest(now: number, profiles: Record<string, any>[]) {
  const people = view.perPerson(await repo.usageSince(monthStart(now)));
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

/** Everything the overview shows. */
export async function overview(now = Date.now()) {
  const { profiles, summary } = await accounts(now);
  const fleet = view.fleet([...registry.browsers.values()], SERVER_RUN);
  return { accounts: summary, top: await heaviest(now, profiles), ...(await reach(now)), fleet };
}

/** One person by email: plan, usage this period and keys (never the keys themselves). */
export async function lookup(email: string, now = Date.now()) {
  const profile = await repo.profileByEmail(email.trim().toLowerCase());
  if (!profile) throw notFound('Account');
  const id = String(profile.id);
  const subscription = await billing.find(id);
  const standing = standingOf(id, subscription, now);
  const keys = (await keysOf(id)).map(keyShown);
  return { profile, standing, subscription, used: await billing.usageSince(id, standing.since), keys };
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
