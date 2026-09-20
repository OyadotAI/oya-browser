/**
 * The key's rate limits: per-minute allowance, burst, and what is left.
 */
import { limitIsLow, metricLabel, num } from './format';
import type { Limit } from './types';

/** Header cell class, right-aligned. */
const TH_RIGHT = 'text-right px-3 py-2';
/** Numeric cell class. */
const TD_NUM = 'px-3 py-2 text-right font-mono tabular-nums text-xs';

/** A table of every limit, with nearly spent ones highlighted. */
export default function LimitsTable({ limits }: { /** Rate limits by name. */ limits: Record<string, Limit> }) {
  return (
    <div>
      <h3 className="text-xs uppercase tracking-wider text-text-dim mb-2">Your remaining allowance</h3>
      <div className="control-table border border-border rounded-xl overflow-x-auto">
        <table className="w-full text-sm">
          <thead className="bg-bg-elevated/60 text-text-dim text-xs">
            <tr>
              <th className="text-left px-3 py-2">Limit</th>
              <th className={TH_RIGHT}>Per minute</th>
              <th className={TH_RIGHT}>Burst</th>
              <th className={TH_RIGHT}>Remaining</th>
            </tr>
          </thead>
          <tbody>
            {Object.entries(limits).map(([name, l]) => (
              <LimitRow key={name} name={name} limit={l} />
            ))}
          </tbody>
        </table>
      </div>
      <p className="text-text-dim text-xs mt-2">
        Everything here is scoped to the API key you are connected with — its browsers, sessions, providers, usage and
        audit trail.
      </p>
    </div>
  );
}

/** One limit; a disabled one reads off and infinite. */
function LimitRow({ name, limit: l }: { /** Limit name. */ name: string; /** The limit. */ limit: Limit }) {
  return (
    <tr className="border-t border-border">
      <td className="px-3 py-2 font-mono text-xs">{metricLabel(name)}</td>
      <td className={TD_NUM}>{l.disabled ? 'off' : num(l.limit)}</td>
      <td className={TD_NUM}>{num(l.burst)}</td>
      <td className={`${TD_NUM} ${limitIsLow(l) ? 'text-yellow' : ''}`}>{l.disabled ? '∞' : num(l.remaining)}</td>
    </tr>
  );
}
