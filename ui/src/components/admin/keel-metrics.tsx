/** Keel's four headline measurements keep reporting coverage beside every measured total. */
import { Kpi } from './parts';
import { coverageText, keelCount, measuredLabel, runtimeText } from './keel-model';
import type { KeelSnapshot } from './keel-types';

/** Accessible missing values remain distinct from a measured zero. */
export function Measurement({
  value,
  text,
}: {
  /** Measurement, or unavailable. */ value: number | null;
  /** Visible value. */ text: string;
}) {
  return <span aria-label={measuredLabel(value, text)}>{text}</span>;
}

/** Existing admin headline tiles, with reporting and usage windows in their labels. */
export function KeelMetrics({ data }: { /** Independent Keel snapshot. */ data: KeelSnapshot }) {
  const usage = data.usage;
  return (
    <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
      <Kpi
        label="Active installs · 7 days"
        value={keelCount(data.installs.active)}
        sub={`${keelCount(data.installs.total)} installations seen in total.`}
      />
      <Kpi
        label="Reported license IDs · 7 days"
        value={keelCount(data.licenses.total)}
        sub="Reported IDs, not issued keys."
      />
      <Kpi
        label="Agent runtime · 30 days"
        value={<Measurement value={usage.runtime_ms} text={runtimeText(usage.runtime_ms)} />}
        sub={coverageText(usage.runtime_reports, usage.reports)}
      />
      <Kpi
        label="Active projects · 30 days"
        value={<Measurement value={usage.projects} text={usage.projects === null ? '—' : keelCount(usage.projects)} />}
        sub={
          <>
            <span className="block">{coverageText(usage.project_reports, usage.reports)}</span>
            <span>The same repository on two installs counts twice.</span>
          </>
        }
      />
    </div>
  );
}
