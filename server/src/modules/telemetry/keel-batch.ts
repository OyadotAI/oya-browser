/** Validates complete versioned batches before any anonymous usage reaches persistence. */
import { invalid } from '../../platform/errors.ts';
import { parseKeelPing } from './keel-ping.ts';
import {
  KEEL_BATCH_DAYS,
  KEEL_MAX_RUNTIME_MS,
  KEEL_PROJECTS_PER_DAY,
  KEEL_REPORT_MAX_BYTES,
  KEEL_SCHEMA_VERSION,
  KEEL_DAY_CHARS,
} from './constants.ts';

/** A completed day; null measurements distinguish legacy/partial instrumentation from measured zero. */
export interface KeelDay {
  /** The completed UTC day. */ day: string;
  /** Accepted effectful agent steps. */ steps: number;
  /** Summed supervised agent runtime, or null when unmeasured. */ runtime_ms: number | null;
  /** Installation-scoped opaque identities, or null when unmeasured/overflowed. */ project_ids: string[] | null;
}

/** A request's current reporting identity and immutable completed-day snapshots. */
export interface KeelReport {
  /** Current installation metadata, checked by the legacy boundary. */ identity: Omit<
    ReturnType<typeof parseKeelPing>,
    'day' | 'steps'
  >;
  /** Completed usage days; empty for a presence-only request. */ reports: KeelDay[];
}

/** Runtime must remain an exact nonnegative integer across JSON and storage. */
function runtime(value: unknown): number | null {
  if (value === null) return null;
  if (!Number.isSafeInteger(value) || Number(value) < 0 || Number(value) > KEEL_MAX_RUNTIME_MS)
    throw invalid('runtime_ms', 'bounded nonnegative milliseconds or null', value);
  return Number(value);
}

/** Projects contain only bounded random-looking identifiers, never names or paths. */
function projects(value: unknown): string[] | null {
  if (value === null) return null;
  if (!validProjects(value)) throw invalid('project_ids', 'bounded opaque project identifiers or null', undefined);
  if (new Set(value).size !== value.length) throw invalid('project_ids', 'distinct identifiers', undefined);
  return [...value].sort();
}

/** Every bounded identifier has the sender's exact installation-local HMAC shape. */
const validProjects = (value: unknown): value is string[] =>
  Array.isArray(value) &&
  value.length <= KEEL_PROJECTS_PER_DAY &&
  value.every((id) => typeof id === 'string' && /^[0-9a-f]{64}$/.test(id));

/** A v2 report cannot claim unfinished or future days. */
function completed(row, identity, today: string): KeelDay {
  const checked = parseKeelPing({ ...identity, day: row?.day, steps: row?.steps });
  if (!checked.day || checked.day >= today) throw invalid('day', 'a completed UTC day', row?.day);
  return {
    day: checked.day,
    steps: checked.steps,
    runtime_ms: runtime(row?.runtime_ms),
    project_ids: projects(row?.project_ids),
  };
}

/** Checks serialized size before iterating a batch; arbitrary unknown fields are never retained. */
function bounded(body) {
  if (Buffer.byteLength(JSON.stringify(body) ?? '') > KEEL_REPORT_MAX_BYTES)
    throw invalid('body', 'a bounded usage report', undefined);
  return body;
}

/** Accepts the original wire format and the explicit v2 envelope without weakening either. */
export function parseKeelReport(input, now = new Date().toISOString()): KeelReport {
  const body = bounded(input);
  return body?.schema_version === undefined ? legacy(body) : versioned(body, now);
}

/** Versions are explicit so an unsupported sender cannot silently lose fields. */
function versioned(body, now: string): KeelReport {
  if (body.schema_version !== KEEL_SCHEMA_VERSION)
    throw invalid('schema_version', 'supported version', body.schema_version);
  const { day: _, steps: __, ...identity } = parseKeelPing({ ...body, day: '', steps: 0 });
  if (!Array.isArray(body.reports) || body.reports.length > KEEL_BATCH_DAYS)
    throw invalid('reports', 'bounded daily reports', undefined);
  return { identity, reports: dailyReports(body.reports, identity, now) };
}

/** Duplicate day identities are rejected before the repository starts any writes. */
function dailyReports(rows, identity, now: string): KeelDay[] {
  const reports = rows.map((row) => completed(row, identity, now.slice(0, KEEL_DAY_CHARS)));
  if (new Set(reports.map((row) => row.day)).size !== reports.length)
    throw invalid('reports', 'distinct days', undefined);
  return reports;
}

/** Preserves the old report, including its first-run presence marker, with unknown measurements. */
function legacy(body): KeelReport {
  const { day, steps, ...identity } = parseKeelPing(body);
  return { identity, reports: [{ day, steps, runtime_ms: null, project_ids: null }] };
}
