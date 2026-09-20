/**
 * The Audit view: a timeline of workspace changes and administrative actions.
 */
import { ACTOR_ID_CHARS, AUDIT_COLUMNS, SHORT_ID_CHARS } from './constants';
import { auditTarget, outcomeClass } from './format';
import type { AuditEvent } from './types';

/** Header cell class. */
const TH = 'text-left px-3 py-2';
/** Column headings, in order. */
const HEADINGS = ['When', 'Action', 'Actor', 'Target', 'From', 'Outcome'];

/** The audit table, or a note that it needs an admin key. */
export default function AuditView({ audit }: { /** Audit trail. */ audit: AuditEvent[] }) {
  return (
    <div className="control-table border border-border rounded-xl overflow-x-auto">
      <table className="w-full text-sm">
        <thead className="bg-bg-elevated/60 text-text-dim text-xs">
          <tr>
            {HEADINGS.map((h) => (
              <th key={h} className={TH}>
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {audit.map((e, i) => (
            <AuditRow key={i} e={e} />
          ))}
          {!audit.length && (
            <tr>
              <td colSpan={AUDIT_COLUMNS} className="px-3 py-8 text-center text-text-dim text-xs">
                No audit events (admin key required).
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}

/** One audit event, its outcome coloured. */
function AuditRow({ e }: { /** The audit event. */ e: AuditEvent }) {
  return (
    <tr className="border-t border-border">
      <td className="px-3 py-2 text-xs text-text-dim whitespace-nowrap">{new Date(e.ts).toLocaleTimeString()}</td>
      <td className="px-3 py-2 font-mono text-xs">{e.action}</td>
      <td className="px-3 py-2 font-mono text-xs text-text-dim">{e.actor?.slice(0, ACTOR_ID_CHARS) || '—'}</td>
      <td className="px-3 py-2 text-xs text-text-dim truncate max-w-[16rem]">{auditTarget(e, SHORT_ID_CHARS)}</td>
      <td className="px-3 py-2 text-xs text-text-dim">{e.ip || '—'}</td>
      <td className={`px-3 py-2 text-xs ${outcomeClass(e.outcome)}`}>{e.outcome}</td>
    </tr>
  );
}
