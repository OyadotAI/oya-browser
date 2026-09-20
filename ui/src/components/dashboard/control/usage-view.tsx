/**
 * The Usage view: every counter for the current hour.
 */
import { usageCell, usageRows, usageWarns } from './format';
import type { Usage } from './types';

/** Loading until the first poll, then the hour's counters. */
export default function UsageView({ usage: u }: { /** Counters for the hour. */ usage: Usage | undefined }) {
  if (!u) return <p className="text-text-dim text-sm">Loading…</p>;
  return (
    <div className="space-y-4">
      <p className="text-text-dim text-xs">Hour beginning {new Date(u.hour).toLocaleString()}</p>
      <div className="control-table border border-border rounded-xl overflow-x-auto">
        <table className="w-full text-sm">
          <thead className="bg-bg-elevated/60 text-text-dim text-xs">
            <tr>
              <th className="text-left px-3 py-2">Metric</th>
              <th className="text-right px-3 py-2">This hour</th>
            </tr>
          </thead>
          <tbody>
            {usageRows(u).map(([label, value]) => (
              <tr key={label} className="border-t border-border">
                <td className="px-3 py-2 text-xs">{label}</td>
                <td
                  className={`px-3 py-2 text-right font-mono tabular-nums text-xs ${usageWarns(label, value) ? 'text-yellow' : ''}`}
                >
                  {usageCell(label, value, u)}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
