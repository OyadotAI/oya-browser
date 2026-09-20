/**
 * Event delivery: add or disable webhooks, and replay deliveries that have
 * not arrived.
 */
import { DELIVERIES_SHOWN, DURABLE_BUTTON, DURABLE_FIELD } from './constants';
import type { Durable } from './use-durable';
import type { Overview } from './types';

/** Webhooks section props. */
interface Props {
  /** State and actions. */
  d: Durable;
  /** The project's webhooks. */
  webhooks: NonNullable<Overview['webhooks']>;
  /** Their deliveries. */
  deliveries: Overview['deliveries'];
}

/** The add form, each webhook, and undelivered deliveries to replay. */
export default function DurableWebhooks({ d, webhooks, deliveries }: Props) {
  const undelivered = deliveries?.filter((x) => x.state !== 'delivered').slice(0, DELIVERIES_SHOWN);
  return (
    <section className="space-y-3">
      <h3 className="text-sm font-medium">Event delivery</h3>
      <WebhookForm d={d} />
      {webhooks.map((h) => (
        <div key={h.id} className="flex justify-between gap-3 text-xs">
          <span className="truncate">{h.url}</span>
          <button
            disabled={d.busy || !h.enabled}
            className={DURABLE_BUTTON}
            onClick={() => void d.act(`/webhooks/${h.id}`, 'DELETE')}
          >
            {h.enabled ? 'Disable' : 'Disabled'}
          </button>
        </div>
      ))}
      {undelivered?.map((x) => (
        <div key={x.id} className="flex justify-between text-xs">
          <span>
            {x.state} · {x.attempts} attempts
          </span>
          <button
            disabled={d.busy}
            className={DURABLE_BUTTON}
            onClick={() => void d.act(`/deliveries/${encodeURIComponent(x.id)}/replay`, 'POST', {})}
          >
            Replay
          </button>
        </div>
      ))}
    </section>
  );
}

/** A URL field and Add webhook. */
function WebhookForm({ d }: { /** Project operations state and actions. */ d: Durable }) {
  return (
    <form
      className="flex gap-2"
      onSubmit={(e) => {
        e.preventDefault();
        void d.act('/webhooks', 'POST', { url: d.form.url });
      }}
    >
      <input
        aria-label="Webhook URL"
        type="url"
        required
        className={DURABLE_FIELD}
        placeholder="https://your-service.example/events"
        value={d.form.url}
        onChange={(e) => d.setForm({ url: e.target.value })}
      />
      <button disabled={d.busy} className={`${DURABLE_BUTTON} shrink-0`}>
        Add webhook
      </button>
    </form>
  );
}
