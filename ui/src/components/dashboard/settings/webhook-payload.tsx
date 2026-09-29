/**
 * What a webhook delivery's body looks like: the envelope every event shares
 * and the `detail` fields of each event type, folded away under the form.
 * Keep it in step with the server's emit sites (docs/control-plane.md lists the same).
 */
'use client';

/** The body of one delivery, as an example. */
const ENVELOPE = `{
  "id": 4812,                  // event id, also in the Oya-Event-Id header; dedupe on it
  "project": "prj_3f9a…",
  "type": "run.failed",
  "sessionId": "brw_…",        // the browser it is about, or null
  "at": 1759100000000,         // when it happened, in milliseconds
  "detail": { "runId": "run_…", "owner": "…", "error": "Timed out" }
}`;

/** Each event type and the fields its `detail` carries. */
export const EVENT_DETAILS: [type: string, detail: string][] = [
  ['session.ready', '{}'],
  ['session.stopped', '{ reason? }'],
  ['session.disconnected', '{ reason? }'],
  ['session.failed', '{ reason }'],
  ['run.needs_attention', '{ runId, owner, reason, message }, or from Ask: { reason, message, source: "chat" }'],
  ['run.failed', '{ runId, owner, error }'],
  ['budget.threshold', '{ threshold, estimatedUsd }'],
  ['persona.created', '{ personaId, name }'],
  ['persona.updated', '{ personaId, fields: string[] }'],
  ['persona.deleted', '{ personaId }'],
  ['recording.ready', '{}  (sessionId is the recorded browser)'],
  ['login.completed', '{ personaId, domain, method }'],
  ['login.failed', '{ personaId, domain, method }'],
  ['mfa.completed', '{ personaId, domain, method }'],
  ['credential.created', '{ id, role }'],
  ['credential.revoked', '{ id }'],
  ['webhook.test', '{ message }  (only from Send test event)'],
];

/** The envelope and the per-event table. */
export function PayloadHelp() {
  return (
    <details className="mt-3 text-[12px] text-text-muted">
      <summary className="cursor-pointer font-medium hover:text-text">What each event looks like</summary>
      <p className="mt-2 leading-5">
        A JSON POST. Every event shares this envelope; only <code className="font-mono">detail</code> changes by type.
      </p>
      <pre className="mt-2 overflow-x-auto rounded-md border border-border bg-bg-sunken p-2 font-mono text-[11px] text-text">
        {ENVELOPE}
      </pre>
      <table className="mt-3 w-full font-mono text-[11px]">
        <tbody>
          {EVENT_DETAILS.map(([type, detail]) => (
            <tr key={type} className="border-t border-border">
              <td className="py-1.5 pr-3 align-top text-text-secondary">{type}</td>
              <td className="py-1.5 text-text">{detail}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </details>
  );
}
