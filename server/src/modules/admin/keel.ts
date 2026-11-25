/** Builds the staff-only Keel snapshot with explicit reporting windows and bounded identity tables. */
import { ACTIVE_DAYS, MS_PER_DAY, KEEL_ROWS_SHOWN } from './constants.ts';
import { keelRows } from './keel-repository.ts';
import { keelDays, keelPeriod, keelTotals } from './keel-usage.ts';

/** Only current reported identifiers and aggregate counts enter the license table. */
interface LicenseGroup {
  /** Opaque reported ID, not a signed key. */ license_id: string;
  /** Recently reporting installations naming this ID. */ installs: number;
  /** Latest server receipt. */ last_seen: string;
}

/** Merge one current installation without retaining its historical license identities. */
function addLicense(groups: Map<string, LicenseGroup>, row) {
  const previous = groups.get(row.license_id);
  groups.set(row.license_id, {
    license_id: row.license_id,
    installs: (previous?.installs ?? 0) + 1,
    last_seen: [previous?.last_seen ?? '', row.last_seen].sort().at(-1)!,
  });
}

/** Current reported license IDs use latest presence, never the union of previously installed keys. */
function licensesOf(active: Record<string, any>[]) {
  const groups = new Map<string, LicenseGroup>();
  for (const row of active.filter((row) => row.license_id)) addLicense(groups, row);
  const rows = [...groups.values()].sort((a, b) => b.last_seen.localeCompare(a.last_seen));
  return { total: rows.length, list: rows.slice(0, KEEL_ROWS_SHOWN) };
}

/** The installation table contains checked metadata and measured counters, never complete keys or project names. */
function installRows(installs, usage) {
  return installs.slice(0, KEEL_ROWS_SHOWN).map((row) => ({
    ...installMetadata(row),
    ...keelTotals(usage.filter((day) => day.install_id === row.install_id)),
  }));
}

/** Metadata is an explicit allowlist, independent of future columns in storage. */
function installMetadata(row) {
  const { install_id, version, os, license_id, first_seen, last_seen } = row;
  return { install_id, version, os, license_id, first_seen, last_seen };
}

/** A fresh independent view prevents unrelated billing or overview failures from hiding Keel usage. */
export async function keelOverview(now = Date.now()) {
  const period = keelPeriod(now);
  const rows = await keelRows(period.from);
  return snapshot(rows, period, now);
}

/** Scope and freshness describe this snapshot rather than a potentially late usage date. */
function stamp(rows, period, now: number) {
  return {
    generated_at: new Date(now).toISOString(),
    latest_report: rows.installs[0]?.last_seen ?? null,
    period: { ...period, active_days: ACTIVE_DAYS },
  };
}

/** Period selection is shared by headlines, day rows and installation counters. */
function snapshot(rows, period, now: number) {
  const usage = rows.usage.filter((row) => row.day <= period.to);
  const active = rows.installs.filter(
    (row) => Date.parse(row.last_seen) >= now - ACTIVE_DAYS * MS_PER_DAY && Date.parse(row.last_seen) <= now,
  );
  return { ...stamp(rows, period, now), ...activityView(rows, usage, active, period) };
}

/** Table limits never constrain headline totals or the selected usage population. */
function activityView(rows, usage, active, period) {
  return {
    usage: keelTotals(usage),
    days: keelDays(usage, period.from),
    installs: { total: rows.installs.length, active: active.length, list: installRows(rows.installs, usage) },
    licenses: licensesOf(active),
  };
}
