/**
 * Where the admin page's own records are kept: self-hosted installs, download
 * counts per day, and issued licenses. Everything else it shows it reads from
 * the tables the rest of the server keeps.
 */
import { getConnection } from '../../platform/storage/index.ts';
import { DAY_CHARS } from './constants.ts';

/** A ping's fields, as the receiver checked them. */
export type Ping = {
  /** The install's random id. */
  id: string;
  /** The release it runs. */
  version: string;
  /** Browsers connected. */
  browsers: number;
  /** Most cloud browsers at once since the last ping. */
  peak_cloud: number;
  /** Its license id, or null. */
  license: string | null;
};

/** An issued license, as stored. */
export type LicenseRow = Record<string, any> & {
  /** Its id, which the install reports. */
  id: string;
};

/**
 * Records a ping: the install as it is now, keeping when it was first seen and
 * how often it pinged. Answers whether it is the install's first ping today.
 */
export async function recordPing(ping: Ping, now = new Date().toISOString()) {
  const [seen] = await getConnection().select('installs', { install_id: ping.id });
  const { id, license, ...counts } = ping;
  const kept = { first_seen: seen?.first_seen ?? now, pings: (Number(seen?.pings) || 0) + 1 };
  const row = { install_id: id, ...counts, license_id: license, ...kept, last_seen: now };
  await getConnection().upsert('installs', [row], { update: true });
  return String(seen?.last_seen || '').slice(0, DAY_CHARS) !== now.slice(0, DAY_CHARS);
}

/** Installs, most recently seen first. */
export const installs = (limit: number) =>
  getConnection().select('installs', {}, { order: ['last_seen', 'desc'], limit });

/** Adds counts to their days: one read and one write per day, kind and platform. */
export async function addDownloads(counts: Map<string, number>) {
  for (const [key, n] of counts) {
    const [day, kind, platform] = key.split('|');
    const [row] = await getConnection().select('download_counts', { day, kind, platform });
    await getConnection().upsert('download_counts', [{ day, kind, platform, count: (Number(row?.count) || 0) + n }], {
      update: true,
    });
  }
}

/** Download counts from `day` (YYYY-MM-DD) on. */
export async function downloadsSince(day: string) {
  return (await getConnection().select('download_counts', {})).filter((r) => String(r.day) >= day);
}

/** Stores an issued license. */
export const saveLicense = (row: LicenseRow) => getConnection().upsert('licenses', [row], { update: true });

/** Every issued license, newest first. */
export const licenses = (): Promise<LicenseRow[]> =>
  getConnection().select('licenses', {}, { order: ['created_at', 'desc'] }) as Promise<LicenseRow[]>;

/** One issued license, or null. */
export async function license(id: string): Promise<LicenseRow | null> {
  const [row] = await getConnection().select('licenses', { id });
  return (row as LicenseRow) || null;
}

/** Every account's profile. */
export const profiles = () => getConnection().select('profiles', {});

/** The profile with this email, or null. */
export async function profileByEmail(email: string) {
  const [row] = await getConnection().select('profiles', { email });
  return row || null;
}

/** Every usage row since an ISO time. ponytail: scans all keys' rows, a per-person monthly table if usage grows large. */
export const usageSince = (since: string) => getConnection().select('usage', { hour: { gte: since } });

/** Every API key's hash and owner, which ties a key's usage rows to a person. */
export const apiKeys = () => getConnection().select('api_keys', {});

/** Every self-hosted install, for when each was first seen. */
export const allInstalls = () => getConnection().select('installs', {});

/** A profile must exist before an administrator can adjust its billing. */
export async function profileById(id: string) {
  const [row] = await getConnection().select('profiles', { id });
  return row || null;
}
