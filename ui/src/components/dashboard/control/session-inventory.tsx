/**
 * The durable session inventory: every session in the project, filterable
 * by state, with the control, recovery and stop actions each state allows.
 */
import { DURABLE_BUTTON, DURABLE_FIELD, ENDED_STATES, SESSION_STATES, SHORT_ID_CHARS } from './constants';
import type { Durable } from './use-durable';
import type { DurableSession } from './types';

/** A control hand-over: the action sent and its button's label. */
interface ControlStep {
  /** Action posted to /sessions/:id/control. */
  action: string;
  /** Button text. */
  label: string;
}

/** Who holds a ready session → the control action that hands it over. */
const CONTROL_ACTIONS: Record<string, ControlStep> = {
  agent: { action: 'acquire', label: 'Take control' },
  human: { action: 'release', label: 'Release' },
};
/** Any other mode: the agent is paused, so resume it. */
const RESUME = { action: 'resume', label: 'Resume agent' };

/** The control action for a session's mode. */
export const controlAction = (mode: string) => (Object.hasOwn(CONTROL_ACTIONS, mode) ? CONTROL_ACTIONS[mode] : RESUME);

/** Column headings, in order. */
const HEADINGS = ['Session', 'Provider', 'State', 'Control', 'Actions'];

/** The filter and the table. */
export default function SessionInventory({
  d,
  sessions: all,
}: {
  /** Project operations state and actions. */ d: Durable;
  /** Sessions to list. */ sessions: DurableSession[];
}) {
  const filter = d.form.filter;
  const sessions = all.filter((s) => !filter || s.state === filter);
  return (
    <section>
      <div className="mb-3 flex items-center justify-between gap-3">
        <h3 className="text-sm font-medium">Session inventory</h3>
        <select
          aria-label="Filter sessions by state"
          className={`${DURABLE_FIELD} max-w-48`}
          value={filter}
          onChange={(e) => d.setForm({ filter: e.target.value })}
        >
          <option value="">All states</option>
          {SESSION_STATES.map((s) => (
            <option key={s}>{s}</option>
          ))}
        </select>
      </div>
      <div className="overflow-x-auto rounded border border-border">
        <table className="w-full text-left text-xs">
          <thead className="bg-bg-elevated text-text-dim">
            <tr>
              {HEADINGS.map((h) => (
                <th key={h} className="px-3 py-2 font-normal">
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {sessions.map((s) => (
              <SessionRow key={s.id} s={s} d={d} />
            ))}
          </tbody>
        </table>
        {!sessions.length && <p className="p-5 text-sm text-text-dim">No sessions match this view.</p>}
      </div>
    </section>
  );
}

/** One session: id, provider and guarantees, state, controller, actions. */
function SessionRow({
  s,
  d,
}: {
  /** The session. */ s: DurableSession;
  /** Project operations state and actions. */ d: Durable;
}) {
  return (
    <tr className="border-t border-border">
      <td className="max-w-48 truncate px-3 py-3 font-mono" title={s.id}>
        {s.id.slice(0, SHORT_ID_CHARS)}
      </td>
      <td className="px-3 py-3">
        {s.provider}
        <span className="mt-1 block text-[10px] text-text-dim">
          {s.managed ? 'Managed provisioning' : 'Legacy · limited guarantees'}
        </span>
      </td>
      <td className="px-3 py-3" title={s.cleanupError}>
        {s.state.replaceAll('_', ' ')}
      </td>
      <td className="px-3 py-3">{s.control.mode}</td>
      <td className="px-3 py-3">
        <SessionActions s={s} d={d} />
      </td>
    </tr>
  );
}

/** Hand over control when ready, replace an ended managed session, or stop a live one. */
function SessionActions({
  s,
  d,
}: {
  /** The session. */ s: DurableSession;
  /** Project operations state and actions. */ d: Durable;
}) {
  const ended = ENDED_STATES.includes(s.state);
  const control = controlAction(s.control.mode);
  const button = (label: string, path: string, body: unknown, extra = '') => (
    <button disabled={d.busy} className={`${DURABLE_BUTTON}${extra}`} onClick={() => void d.act(path, 'POST', body)}>
      {label}
    </button>
  );
  return (
    <div className="flex flex-wrap gap-2">
      {s.state === 'ready' && button(control.label, `/sessions/${s.id}/control`, { action: control.action })}
      {ended && s.managed && button('Replace from profile', `/sessions/${s.id}/recover`, { replace: true })}
      {!ended && (
        <>
          {button('Stop', `/sessions/${s.id}/stop`, {})}
          {button('Force stop', `/sessions/${s.id}/stop`, { force: true }, ' text-red')}
        </>
      )}
    </div>
  );
}
