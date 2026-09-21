/**
 * The webhook panel's parts: the endpoint form, its actions, the one-time
 * secret, the verification help, and recent deliveries.
 */
'use client';

import { Check, Copy, Loader2, RefreshCw, RotateCcw, Trash2 } from 'lucide-react';
import { ago } from '@/lib/api-client';
import { useToast } from '../toast';
import type { RowComponent } from './fields';
import type { WebhookState } from './use-webhook';
import type { Delivery, WebhookConfig } from './webhook-types';

/** How a receiver checks the signature, in Node. */
const VERIFY = `const [t, v1] = req.headers['oya-signature'].split(',').map(p => p.split('=')[1]);
const expected = crypto.createHmac('sha256', SECRET).update(\`\${t}.\${rawBody}\`).digest('hex');
if (expected !== v1 || Date.now() / 1000 - t > 300) return res.sendStatus(400);`;

/** Dot colour per delivery state; anything else (failed) is red. */
const DELIVERY_DOT: Record<string, string> = { delivered: 'bg-accent', pending: 'bg-yellow' };

/** Panel state plus the loaded config, which every part below needs. */
interface Props {
  /** Panel state and actions. */
  webhook: WebhookState;
  /** The loaded config. */
  config: WebhookConfig;
}

/** The endpoint URL and the event checkboxes. */
export function WebhookFields({
  webhook,
  config,
  Row,
}: Props & { /** The dialog's row component. */ Row: RowComponent }) {
  const hint = config.hook?.enabled
    ? 'Receiving events.'
    : config.hook
      ? 'Disabled · save to turn back on.'
      : 'Must be HTTPS.';
  return (
    <div className="space-y-5">
      <Row id="webhook-url" label="Endpoint URL" hint={hint}>
        <input
          id="webhook-url"
          className="settings-input font-mono text-[12px]"
          type="url"
          placeholder="https://example.com/oya/webhook"
          value={webhook.url}
          onChange={(e) => webhook.setUrl(e.target.value)}
        />
      </Row>
      <Row label="Events" hint="Nothing selected sends every event.">
        <div role="group" aria-label="Webhook events" className="grid grid-cols-1 gap-1.5 sm:grid-cols-2">
          {config.events.map((type) => (
            <label key={type} className="flex items-center gap-2 font-mono text-[11.5px] text-text-secondary">
              <input type="checkbox" checked={webhook.types.includes(type)} onChange={() => webhook.toggle(type)} />
              {type}
            </label>
          ))}
        </div>
      </Row>
    </div>
  );
}

/** Save, and once an endpoint exists, Rotate secret; Disable while it is active. */
export function WebhookActions({ webhook, config }: Props) {
  const { busy, url, save, disable } = webhook;
  return (
    <div className="mt-5 flex flex-wrap gap-2">
      <button type="button" className="btn-primary h-9 px-4" disabled={busy || !url.trim()} onClick={() => void save()}>
        {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Check className="h-3.5 w-3.5" />}Save
      </button>
      {config.hook && (
        <button type="button" className="btn-ghost h-9" disabled={busy || !url.trim()} onClick={() => void save(true)}>
          <RotateCcw className="h-3.5 w-3.5" />
          Rotate secret
        </button>
      )}
      {config.hook?.enabled && (
        <button type="button" className="btn-ghost h-9" disabled={busy} onClick={() => void disable()}>
          <Trash2 className="h-3.5 w-3.5" />
          Disable
        </button>
      )}
    </div>
  );
}

/** A freshly minted signing secret: shown once, with a copy button. */
export function SigningSecret({ secret }: { /** The secret. */ secret: string }) {
  const toast = useToast();
  return (
    <div className="mt-5 rounded-lg border border-yellow/25 bg-yellow/5 p-3">
      <p className="text-[12px] leading-5 text-text-secondary">Signing secret, copy it now, it won’t be shown again.</p>
      <div className="mt-2 flex items-center gap-2">
        <code className="min-w-0 flex-1 truncate rounded-md border border-border bg-bg-sunken px-2 py-1.5 font-mono text-[11px] text-text">
          {secret}
        </code>
        <button
          type="button"
          className="btn-ghost h-8 shrink-0"
          onClick={() => void navigator.clipboard?.writeText(secret).then(() => toast('Secret copied', 'success'))}
        >
          <Copy className="h-3.5 w-3.5" />
          Copy
        </button>
      </div>
    </div>
  );
}

/** How to verify a delivery's signature, folded away. */
export function VerifyHelp() {
  return (
    <details className="mt-5 border-t border-border pt-4 text-[12px] text-text-muted">
      <summary className="cursor-pointer font-medium hover:text-text">Verifying the signature</summary>
      <p className="mt-2 leading-5">
        Each POST carries <code className="font-mono">Oya-Event-Id</code> and{' '}
        <code className="font-mono">Oya-Signature: t=…,v1=…</code>, an HMAC-SHA256 of{' '}
        <code className="font-mono">t.body</code>. Respond 2xx to acknowledge; dedupe on the event id.
      </p>
      <pre className="mt-2 overflow-x-auto rounded-md border border-border bg-bg-sunken p-2 font-mono text-[11px] text-text">
        {VERIFY}
      </pre>
    </details>
  );
}

/** One delivery: state, type, tries, age, and Replay. */
function DeliveryRow({
  d,
  onReplay,
}: {
  /** The delivery. */ d: Delivery;
  /** Queues it again. */ onReplay: () => void;
}) {
  const dot = Object.hasOwn(DELIVERY_DOT, d.state) ? DELIVERY_DOT[d.state] : 'bg-red';
  return (
    <li className="flex items-center gap-3 py-2">
      <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${dot}`} />
      <span className="min-w-0 flex-1 truncate font-mono">{d.type ?? 'event'}</span>
      <span className="text-text-muted">
        {d.state}
        {d.attempts > 1 ? ` · ${d.attempts} tries` : ''} · {ago(new Date(d.at).toISOString())}
      </span>
      <button type="button" className="btn-ghost h-7" onClick={onReplay}>
        Replay
      </button>
    </li>
  );
}

/** Recent deliveries, with a refresh. */
export function Deliveries({ webhook, config }: Props) {
  return (
    <div className="mt-5 border-t border-border pt-4">
      <div className="mb-2 flex items-center justify-between">
        <h4 className="text-[13px] font-medium">Recent deliveries</h4>
        <button
          type="button"
          className="btn-ghost h-8"
          onClick={() => void webhook.load()}
          aria-label="Refresh deliveries"
        >
          <RefreshCw className="h-3.5 w-3.5" />
        </button>
      </div>
      {config.deliveries.length ? (
        <ul className="divide-y divide-border text-[12px]">
          {config.deliveries.map((d) => (
            <DeliveryRow key={d.id} d={d} onReplay={() => void webhook.replay(d.id)} />
          ))}
        </ul>
      ) : (
        <p className="text-[12px] text-text-muted">No events sent yet.</p>
      )}
    </div>
  );
}
