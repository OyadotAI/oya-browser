/**
 * The CDP sessions view: every gateway session the key holds, with a button
 * to end each.
 */
import { X } from 'lucide-react';
import { SESSION_COLUMNS, SESSION_ID_CHARS } from './constants';
import { bytes, duration } from './format';
import { del } from './requests';
import type { Control } from './use-control';
import type { Session } from './types';

/** Header cell classes, left or right aligned. */
const TH = 'text-left px-3 py-2';
/** Right-aligned header cell. */
const TH_RIGHT = 'text-right px-3 py-2';
/** Numeric cell. */
const TD_NUM = 'px-3 py-2 text-right font-mono tabular-nums text-xs';

/** The sessions table, or an explanation of what creates one. */
export default function SessionsView({ ctl }: { /** Control state and actions. */ ctl: Control }) {
  return (
    <div className="control-table border border-border rounded-xl overflow-x-auto">
      <table className="w-full text-sm">
        <thead className="bg-bg-elevated/60 text-text-dim text-xs">
          <tr>
            <th className={TH}>Session</th>
            <th className={TH}>Provider</th>
            <th className={TH}>Profile</th>
            <th className={TH_RIGHT}>Age</th>
            <th className={TH_RIGHT}>Traffic</th>
            <th className={TH}>State</th>
            <th className="px-3 py-2" />
          </tr>
        </thead>
        <tbody>
          {ctl.sessions.map((s) => (
            <SessionRow key={s.id} s={s} ctl={ctl} />
          ))}
          {!ctl.sessions.length && <EmptyRow />}
        </tbody>
      </table>
    </div>
  );
}

/** One session: provider, profile, age, traffic, state and End session. */
function SessionRow({ s, ctl }: { /** The session. */ s: Session; /** Control state and actions. */ ctl: Control }) {
  const end = () => void ctl.act(s.id, () => del(ctl.apiKey, `/gateway/sessions/${s.id}`));
  return (
    <tr className="border-t border-border">
      <td className="px-3 py-2 font-mono text-xs">{s.id.slice(0, SESSION_ID_CHARS)}</td>
      <td className="px-3 py-2 text-xs">{s.provider}</td>
      <td className="px-3 py-2 text-xs">{s.profile || <span className="text-text-dim">—</span>}</td>
      <td className={TD_NUM}>{duration(s.seconds)}</td>
      <td className={TD_NUM}>{bytes(s.bytesUp + s.bytesDown)}</td>
      <td className="px-3 py-2 text-xs">
        <span className={s.connected ? 'text-accent' : 'text-yellow'}>
          {s.connected ? 'attached' : 'held for resume'}
        </span>
        {s.recording && <span className="ml-2 text-text-dim">● rec</span>}
      </td>
      <td className="px-3 py-2 text-right">
        <button
          disabled={!!ctl.busy}
          onClick={end}
          className="text-text-dim hover:text-red disabled:opacity-40"
          title="End session"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </td>
    </tr>
  );
}

/** Shown when the key has no CDP sessions. */
function EmptyRow() {
  return (
    <tr>
      <td colSpan={SESSION_COLUMNS} className="px-3 py-8 text-center text-text-dim text-xs">
        No CDP sessions yet. Connect Playwright or Puppeteer through /connect to start one. REST commands appear in each
        browser’s Activity history.
      </td>
    </tr>
  );
}
