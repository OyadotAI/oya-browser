'use client';

import { useCallback, useEffect, useState, type ReactNode } from 'react';
import { Check, Copy, Loader2, RefreshCw, RotateCcw, Trash2 } from 'lucide-react';
import { api, ago, errorMessage } from '@/lib/api-client';
import { useToast } from './toast';

type Hook = { id: string; url: string; types: string[]; enabled: boolean };
type Delivery = { id: string; state: string; attempts: number; at: number; type: string | null };
type Config = { hook: Hook | null; events: string[]; deliveries: Delivery[] };

const VERIFY = `const [t, v1] = req.headers['oya-signature'].split(',').map(p => p.split('=')[1]);
const expected = crypto.createHmac('sha256', SECRET).update(\`\${t}.\${rawBody}\`).digest('hex');
if (expected !== v1 || Date.now() / 1000 - t > 300) return res.sendStatus(400);`;

/**
 * One endpoint per project, Stripe-style: a URL, the events it wants, and a signing
 * secret shown only when it is minted. Saves itself, like the Slack panel — the
 * endpoint is live the moment it is saved.
 */
export default function WebhookSection({ apiKey, Row }: { apiKey: string; Row: (props: { id?: string; label: string; hint?: string; children: ReactNode }) => ReactNode }) {
  const toast = useToast();
  const [config, setConfig] = useState<Config | null>(null);
  const [url, setUrl] = useState('');
  const [types, setTypes] = useState<string[]>([]);
  const [secret, setSecret] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');

  const load = useCallback(async () => {
    try {
      const next = await api<Config>('/control/webhook', { key: apiKey });
      setConfig(next);
      if (next.hook) { setUrl(next.hook.url); setTypes(next.hook.types); }
    } catch (err) { setError(errorMessage(err)); }
  }, [apiKey]);
  useEffect(() => { void load(); }, [load]);

  const save = async (roll = false) => {
    setBusy(true); setError('');
    try {
      const saved = await api<Hook & { secret?: string }>('/control/webhook', { key: apiKey, method: 'PUT', body: { url: url.trim(), types, roll } });
      if (saved.secret) setSecret(saved.secret);
      toast(roll ? 'Signing secret rotated' : 'Webhook saved', 'success');
      await load();
    } catch (err) { setError(errorMessage(err)); }
    finally { setBusy(false); }
  };

  const disable = async () => {
    setBusy(true); setError('');
    try { await api('/control/webhook', { key: apiKey, method: 'DELETE' }); setSecret(''); toast('Webhook disabled', 'success'); await load(); }
    catch (err) { setError(errorMessage(err)); }
    finally { setBusy(false); }
  };

  const replay = async (id: string) => {
    try { await api(`/control/deliveries/${encodeURIComponent(id)}/replay`, { key: apiKey, method: 'POST' }); toast('Delivery queued', 'success'); await load(); }
    catch (err) { setError(errorMessage(err)); }
  };

  if (!config) return error
    ? <div role="alert" className="rounded-lg border border-red/25 bg-red/5 p-3 text-[13px] text-red">{error}</div>
    : <div className="flex min-h-[270px] items-center justify-center gap-2 text-sm text-text-muted" role="status"><Loader2 className="h-4 w-4 animate-spin" />Loading your webhook…</div>;

  const toggle = (type: string) => setTypes(t => t.includes(type) ? t.filter(x => x !== type) : [...t, type]);
  const active = config.hook?.enabled;
  return (
    <>
      <div className="mb-7">
        <p className="mb-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent">Webhooks</p>
        <h3 className="text-[22px] font-semibold tracking-[-0.035em]">Events, pushed to you.</h3>
        <p className="mt-1.5 text-[13px] leading-6 text-text-muted">One HTTPS endpoint receives the events you choose, signed so you can verify they came from Oya. Failed deliveries retry with backoff for a day.</p>
      </div>
      {error && <div role="alert" className="mb-5 rounded-lg border border-red/25 bg-red/5 p-3 text-[13px] text-red">{error}</div>}

      <div className="space-y-5">
        <Row id="webhook-url" label="Endpoint URL" hint={active ? 'Receiving events.' : config.hook ? 'Disabled · save to turn back on.' : 'Must be HTTPS.'}>
          <input id="webhook-url" className="settings-input font-mono text-[12px]" type="url" placeholder="https://example.com/oya/webhook" value={url} onChange={e => setUrl(e.target.value)} />
        </Row>
        <Row label="Events" hint="Nothing selected sends every event.">
          <div role="group" aria-label="Webhook events" className="grid grid-cols-1 gap-1.5 sm:grid-cols-2">
            {config.events.map(type => (
              <label key={type} className="flex items-center gap-2 font-mono text-[11.5px] text-text-secondary">
                <input type="checkbox" checked={types.includes(type)} onChange={() => toggle(type)} />{type}
              </label>
            ))}
          </div>
        </Row>
      </div>

      <div className="mt-5 flex flex-wrap gap-2">
        <button type="button" className="btn-primary h-9 px-4" disabled={busy || !url.trim()} onClick={() => void save()}>
          {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Check className="h-3.5 w-3.5" />}Save
        </button>
        {config.hook && <button type="button" className="btn-ghost h-9" disabled={busy || !url.trim()} onClick={() => void save(true)}><RotateCcw className="h-3.5 w-3.5" />Rotate secret</button>}
        {active && <button type="button" className="btn-ghost h-9" disabled={busy} onClick={() => void disable()}><Trash2 className="h-3.5 w-3.5" />Disable</button>}
      </div>

      {secret && (
        <div className="mt-5 rounded-lg border border-yellow/25 bg-yellow/5 p-3">
          <p className="text-[12px] leading-5 text-text-secondary">Signing secret — copy it now, it won’t be shown again.</p>
          <div className="mt-2 flex items-center gap-2">
            <code className="min-w-0 flex-1 truncate rounded-md border border-border bg-bg-sunken px-2 py-1.5 font-mono text-[11px] text-text">{secret}</code>
            <button type="button" className="btn-ghost h-8 shrink-0" onClick={() => void navigator.clipboard?.writeText(secret).then(() => toast('Secret copied', 'success'))}><Copy className="h-3.5 w-3.5" />Copy</button>
          </div>
        </div>
      )}

      <details className="mt-5 border-t border-border pt-4 text-[12px] text-text-muted">
        <summary className="cursor-pointer font-medium hover:text-text">Verifying the signature</summary>
        <p className="mt-2 leading-5">Each POST carries <code className="font-mono">Oya-Event-Id</code> and <code className="font-mono">Oya-Signature: t=…,v1=…</code>, an HMAC-SHA256 of <code className="font-mono">t.body</code>. Respond 2xx to acknowledge; dedupe on the event id.</p>
        <pre className="mt-2 overflow-x-auto rounded-md border border-border bg-bg-sunken p-2 font-mono text-[11px] text-text">{VERIFY}</pre>
      </details>

      {config.hook && (
        <div className="mt-5 border-t border-border pt-4">
          <div className="mb-2 flex items-center justify-between">
            <h4 className="text-[13px] font-medium">Recent deliveries</h4>
            <button type="button" className="btn-ghost h-8" onClick={() => void load()} aria-label="Refresh deliveries"><RefreshCw className="h-3.5 w-3.5" /></button>
          </div>
          {config.deliveries.length ? (
            <ul className="divide-y divide-border text-[12px]">
              {config.deliveries.map(d => (
                <li key={d.id} className="flex items-center gap-3 py-2">
                  <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${d.state === 'delivered' ? 'bg-accent' : d.state === 'pending' ? 'bg-yellow' : 'bg-red'}`} />
                  <span className="min-w-0 flex-1 truncate font-mono">{d.type ?? 'event'}</span>
                  <span className="text-text-muted">{d.state}{d.attempts > 1 ? ` · ${d.attempts} tries` : ''} · {ago(new Date(d.at).toISOString())}</span>
                  <button type="button" className="btn-ghost h-7" onClick={() => void replay(d.id)}>Replay</button>
                </li>
              ))}
            </ul>
          ) : <p className="text-[12px] text-text-muted">No events sent yet.</p>}
        </div>
      )}
    </>
  );
}
