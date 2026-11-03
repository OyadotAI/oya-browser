/** Bounded installation and reported-ID tables avoid claiming key issuance or license validity. */
import { Section, Table } from './parts';
import { coverageText, keelCount, runtimeText } from './keel-model';
import type { KeelSnapshot } from './keel-types';
import { KEEL_INSTALL_NUMERIC } from './constants';
import { utcTime } from './model';

/** Opaque identifiers wrap in a bounded column instead of widening the whole page. */
const identifier = (id: string) => (
  <code className="block min-w-32 max-w-48 break-all font-mono text-[11px]">{id}</code>
);

/** Stable explicit times keep historical receipts distinct from snapshot freshness. */
export const reportTime = (time: string | null) => (time ? utcTime(time) : 'Never');

/** Every installation's cells preserve unavailable measurements and show coverage in accessible titles. */
function installationRows(data: KeelSnapshot) {
  return data.installs.list.map((row) => [
    identifier(row.install_id),
    `${row.version} · ${row.os}`,
    reportTime(row.last_seen),
    row.license_id ? identifier(row.license_id) : 'None reported',
    <span
      key="runtime"
      title={coverageText(row.runtime_reports, row.reports)}
      aria-label={row.runtime_ms === null ? 'Not reported' : undefined}
    >
      {runtimeText(row.runtime_ms)}
    </span>,
    <span
      key="projects"
      title={coverageText(row.project_reports, row.reports)}
      aria-label={row.projects === null ? 'Not reported' : undefined}
    >
      {row.projects === null ? '—' : keelCount(row.projects)}
    </span>,
    keelCount(row.steps),
  ]);
}

/** Recent installation metadata and measured usage, with headline totals independent of row limits. */
export function KeelInstallations({ data }: { /** Snapshot. */ data: KeelSnapshot }) {
  return (
    <Section
      title="Installations"
      hint={`Showing ${data.installs.list.length} of ${keelCount(data.installs.total)} installations`}
    >
      <Table
        head={[
          'Installation ID',
          'Version / OS',
          'Last report',
          'Reported license ID',
          'Agent runtime · 30 days',
          'Active projects · 30 days',
          'Reported steps · 30 days',
        ]}
        rows={installationRows(data)}
        numeric={KEEL_INSTALL_NUMERIC}
      />
    </Section>
  );
}

/** No key controls are offered because the receiver only knows client-reported identifiers. */
export function KeelLicenses({ data }: { /** Snapshot. */ data: KeelSnapshot }) {
  const rows = data.licenses.list.map((row) => [
    identifier(row.license_id),
    keelCount(row.installs),
    reportTime(row.last_seen),
  ]);
  return (
    <Section
      title="Reported license IDs"
      hint={`Showing ${rows.length} of ${keelCount(data.licenses.total)} reported IDs`}
    >
      {rows.length ? (
        <Table
          head={['Reported license ID', 'Active installations · 7 days', 'Last report']}
          rows={rows}
          numeric={[1]}
        />
      ) : (
        <p className="text-xs text-text-dim">No license IDs reported by installations active in the last 7 days.</p>
      )}
    </Section>
  );
}
