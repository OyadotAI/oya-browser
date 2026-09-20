/**
 * Durable activity: the project's most recent events, newest first.
 */
import { DURABLE_EVENTS_SHOWN, SHORT_ID_CHARS } from './constants';
import type { Overview } from './types';

/** The last events, or a note that there are none. */
export default function DurableActivity({
  events,
}: {
  /** Durable events, oldest first. */ events: Overview['events'];
}) {
  return (
    <section>
      <h3 className="mb-3 text-sm font-medium">Durable activity</h3>
      <div className="divide-y divide-border border-y border-border">
        {events
          .slice(-DURABLE_EVENTS_SHOWN)
          .reverse()
          .map((e) => (
            <div key={e.id} className="flex justify-between gap-4 py-2 text-xs">
              <span>
                {e.type}
                <span className="ml-3 font-mono text-text-dim">{e.sessionId?.slice(0, SHORT_ID_CHARS)}</span>
              </span>
              <time className="shrink-0 text-text-dim">{new Date(e.at).toLocaleString()}</time>
            </div>
          ))}
        {!events.length && <p className="py-3 text-xs text-text-dim">No events yet.</p>}
      </div>
    </section>
  );
}
