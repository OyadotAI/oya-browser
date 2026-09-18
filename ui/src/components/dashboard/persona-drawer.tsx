'use client';

import { useEffect, useState } from 'react';
import { Loader2, Copy, Trash2, Lock, ShieldCheck, ShieldOff } from 'lucide-react';
import Dialog, { Confirm } from '@/components/ui/dialog';
import { api, errorMessage, ago } from '@/lib/api-client';
import { useToast } from './toast';
import type { Persona, BrowserRow } from './types';
import { Preview } from './persona-form';
import { desktopSignInUrl } from './config';

interface Proxy { id: string; label: string; geo: string | null; available?: boolean; assigned: number; maxPersonas: number }

interface Props {
  persona: Persona | null;
  onClose: () => void;
  apiKey: string;
  browsers: BrowserRow[];
  onChanged: () => void;
  onShowBrowsers: (personaId: string) => void;
  now: number;
}

/** Everything about one identity, and the few things about it that may change. */
export default function PersonaDrawer({ persona, onClose, apiKey, browsers, onChanged, onShowBrowsers, now }: Props) {
  const toast = useToast();
  const [name, setName] = useState('');
  const [cap, setCap] = useState('');
  const [geo, setGeo] = useState('');
  const [proxies, setProxies] = useState<Proxy[]>([]);
  const [pin, setPin] = useState('');
  const [mfaType, setMfaType] = useState<'totp' | 'email' | 'sms'>('totp');
  const [mfaValue, setMfaValue] = useState('');
  const [credDomain, setCredDomain] = useState('');
  const [credUser, setCredUser] = useState('');
  const [credPassword, setCredPassword] = useState('');
  const [busy, setBusy] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);

  useEffect(() => {
    if (!persona) return;
    setName(persona.name);
    setCap(persona.maxConcurrent === null ? '' : String(persona.maxConcurrent));
    setGeo(persona.proxy?.geo || '');
    setPin(persona.exit?.id || '');
    setMfaValue('');
    api<{ proxies?: Proxy[] } | Proxy[]>('/proxies', { key: apiKey })
      .then((r) => setProxies(Array.isArray(r) ? r : r.proxies || [])).catch(() => setProxies([]));
  // Refreshing profile counts must not overwrite a form being edited.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [persona?.id, apiKey]);

  if (!persona) return null;
  const p = persona;
  const running = browsers.filter((b) => b.persona === p.id);
  const dirty = name !== p.name || cap !== (p.maxConcurrent === null ? '' : String(p.maxConcurrent)) || geo !== (p.proxy?.geo || '');

  const run = async (what: string, fn: () => Promise<unknown>, done?: string) => {
    setBusy(what);
    try { await fn(); if (done) toast(done, 'success'); onChanged(); }
    catch (err) { toast(errorMessage(err), 'error'); }
    finally { setBusy(null); }
  };

  const save = () => run('save', () => api(`/personas/${p.id}`, { key: apiKey, method: 'PUT', body: {
    name, maxConcurrent: cap === '' ? null : Number(cap), proxy: geo ? { ...(p.proxy || {}), geo } : null,
  } }), 'Saved');

  const savePin = (proxyId: string) => run('pin', () => api(`/personas/${p.id}/proxy`, { key: apiKey, method: 'PUT', body: { proxyId: proxyId || null } }), proxyId ? 'Pinned' : 'Unpinned');

  const setMfa = () => run('mfa', () => api(`/personas/${p.id}/mfa`, { key: apiKey, method: 'PUT',
    body: mfaType === 'totp' ? { type: 'totp', secret: mfaValue } : { type: mfaType, url: mfaValue } }), 'Second factor stored').then(() => setMfaValue(''));
  const clearMfa = () => run('mfa', () => api(`/personas/${p.id}/mfa`, { key: apiKey, method: 'DELETE' }), 'Second factor removed');
  const addCredential = () => run('cred', () => api(`/personas/${p.id}/credentials`, { key: apiKey, method: 'PUT',
    body: { domain: credDomain, username: credUser, password: credPassword } }), 'Sign-in stored')
    .then(() => { setCredDomain(''); setCredUser(''); setCredPassword(''); });
  const removeCredential = (domain: string) => run('cred',
    () => api(`/personas/${p.id}/credentials?domain=${encodeURIComponent(domain)}`, { key: apiKey, method: 'DELETE' }),
    'Sign-in removed');

  const clone = () => run('clone', async () => {
    const c = await api<Persona>(`/personas/${p.id}/clone`, { key: apiKey, method: 'POST', body: {} });
    toast(`Created ${c.name} — same kind of device, new identity`, 'success');
  });

  const remove = () => run('delete', async () => {
    await api(`/personas/${p.id}`, { key: apiKey, method: 'DELETE' });
    setConfirmDelete(false); onClose();
  }, 'Deleted');

  return (
    <>
      <Dialog open={!!persona} onClose={onClose} title={p.name} size="drawer"
        description={<span className="font-mono">{p.id}{p.isDefault ? ' · default profile' : ''}</span>}
        footer={
          <>
            {!p.isDefault && (
              <button className="btn-ghost mr-auto text-red hover:text-red" onClick={() => setConfirmDelete(true)}
                disabled={running.length > 0} title={running.length ? 'Stop its browsers first' : undefined}>
                <Trash2 className="h-3.5 w-3.5" /> Delete
              </button>
            )}
            <button className="btn-ghost" onClick={clone} disabled={busy === 'clone'}><Copy className="h-3.5 w-3.5" /> Clone as new profile</button>
            <button className="btn-primary" onClick={save} disabled={!dirty || busy === 'save'}>{busy === 'save' && <Loader2 className="h-3.5 w-3.5 animate-spin" />}Save</button>
          </>
        }>
        <div className="space-y-5">
          <section>
            <h3 className="label">Saved account sessions</h3>
            <p className="mb-3 break-words text-sm text-text-secondary">{p.login?.sites.length ? p.login.sites.join(' · ') : 'No account sessions saved yet.'}</p>
            <button className="btn-primary" disabled={!!busy} onClick={() => run('pair', async () => { window.location.href = await desktopSignInUrl(apiKey, p.id); })}>Sign in on desktop</button>
            <p className="mt-2 text-xs text-text-muted">This opens the same Oya desktop window using this profile.</p>
          </section>
          {/* Running */}
          <section>
            <div className="flex items-center justify-between">
              <h3 className="label mb-0">Running now</h3>
              <span className={`text-[12px] num ${running.length >= (p.maxConcurrent ?? Infinity) ? 'text-yellow' : 'text-text-muted'}`}>
                {running.length} / {p.maxConcurrent === null ? '∞' : p.maxConcurrent}
              </span>
            </div>
            {running.length ? (
              <div className="mt-1.5 rounded-md border border-border bg-bg text-[12.5px]">
                {running.slice(0, 6).map((b) => (
                  <div key={b.id} className="flex items-center gap-2 border-b border-border/60 px-2 py-1 last:border-0">
                    <span className={`dot dot-${b.health}`} />
                    <span className="truncate text-text">{b.name}</span>
                    <span className="ml-auto truncate font-mono text-[11px] text-text-muted">{b.currentUrl.replace(/^https?:\/\//, '')}</span>
                  </div>
                ))}
                {running.length > 6 && <button className="w-full px-2 py-1 text-left text-[12px] text-text-muted hover:text-text" onClick={() => onShowBrowsers(p.id)}>and {running.length - 6} more — show in fleet</button>}
              </div>
            ) : <p className="mt-1 text-[12.5px] text-text-dim">Idle{p.lastUsedAt ? ` · last used ${ago(p.lastUsedAt, now)} ago` : ''}.</p>}
          </section>

          {/* Editable */}
          <section className="space-y-3">
            <div>
              <label className="label" htmlFor="pd-name">Name</label>
              <input id="pd-name" className="field" value={name} onChange={(e) => setName(e.target.value)} />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="label" htmlFor="pd-cap">Concurrent browsers</label>
                <input id="pd-cap" className="field num" type="number" min={1} value={cap} onChange={(e) => setCap(e.target.value)} placeholder="∞" />
              </div>
              <div>
                <label className="label" htmlFor="pd-geo">Proxy geo hint</label>
                <input id="pd-geo" className="field" value={geo} onChange={(e) => setGeo(e.target.value.toUpperCase())} placeholder="any" maxLength={8} />
              </div>
            </div>
            <div>
              <label className="label" htmlFor="pd-pin">Exit proxy</label>
              <div className="flex gap-2">
                <select id="pd-pin" className="field" value={pin} onChange={(e) => setPin(e.target.value)}>
                  <option value="">Auto — assigned at first connect{p.exit ? ` (currently ${p.exit.label})` : ''}</option>
                  {proxies.map((x) => <option key={x.id} value={x.id}>{x.label}{x.geo ? ` · ${x.geo}` : ''} · {x.assigned}/{x.maxPersonas}{x.available === false ? ' · unhealthy' : ''}</option>)}
                </select>
                <button className="btn-ghost shrink-0" onClick={() => savePin(pin)} disabled={busy === 'pin' || (pin || '') === (p.exit?.id || '')}>Apply</button>
              </div>
              {p.exit && <p className="mt-1 text-[11.5px] text-text-muted">On <span className="text-text">{p.exit.label}</span>{p.exit.geo ? ` (${p.exit.geo})` : ''}{p.exit.healthy ? '' : ' — unhealthy'}. Changing the exit mid-life changes the IP this identity is known by.</p>}
            </div>
          </section>

          {/* MFA */}
          <section>
            <h3 className="label">Second factor</h3>
            {p.mfa?.configured ? (
              <div className="flex items-center gap-2 rounded-md border border-border bg-bg px-3 py-2 text-[12.5px]">
                <ShieldCheck className="h-4 w-4 text-accent" />
                <span className="text-text">{p.mfa.type?.toUpperCase()} configured</span>
                <span className="text-text-dim">· the secret is never shown</span>
                <button className="btn-ghost ml-auto h-7" onClick={clearMfa} disabled={busy === 'mfa'}><ShieldOff className="h-3.5 w-3.5" /> Remove</button>
              </div>
            ) : (
              <div className="grid grid-cols-[120px_1fr_auto] gap-2">
                <select className="field" value={mfaType} onChange={(e) => setMfaType(e.target.value as typeof mfaType)} aria-label="MFA type">
                  <option value="totp">TOTP</option><option value="email">Email code</option><option value="sms">SMS code</option>
                </select>
                <input className="field font-mono" type={mfaType === 'totp' ? 'password' : 'url'} autoComplete="off" value={mfaValue} onChange={(e) => setMfaValue(e.target.value)}
                  placeholder={mfaType === 'totp' ? 'Base32 secret' : 'https://relay.example/latest'} />
                <button className="btn-ghost" onClick={setMfa} disabled={!mfaValue || busy === 'mfa'}>Store</button>
              </div>
            )}
          </section>

          {/* Site sign-ins */}
          <section>
            <h3 className="label">Portal sign-ins</h3>
            <p className="mb-2 text-xs text-text-muted">
              Signing in on the desktop and inheriting the cookies is still the better path. Store a login only for
              portals that drop the session between runs. Passwords are sealed and never shown again.
            </p>
            {p.sites?.credentials?.length ? (
              <div className="mb-2 rounded-md border border-border bg-bg text-[12.5px]">
                {p.sites.credentials.map((c) => (
                  <div key={c.domain} className="flex items-center gap-2 border-b border-border px-3 py-2 last:border-0">
                    <Lock className="h-3.5 w-3.5 text-accent" />
                    <span className="text-text">{c.domain}</span>
                    <span className="text-text-dim">· {c.username}</span>
                    <button className="btn-ghost ml-auto h-7" onClick={() => removeCredential(c.domain)} disabled={busy === 'cred'}>
                      <Trash2 className="h-3.5 w-3.5" /> Remove
                    </button>
                  </div>
                ))}
              </div>
            ) : null}
            <div className="grid grid-cols-[1fr_1fr_1fr_auto] gap-2">
              <input className="field" value={credDomain} onChange={(e) => setCredDomain(e.target.value)} placeholder="portal.example.com" aria-label="Site" />
              <input className="field" autoComplete="off" value={credUser} onChange={(e) => setCredUser(e.target.value)} placeholder="Username" aria-label="Username" />
              <input className="field" type="password" autoComplete="new-password" value={credPassword} onChange={(e) => setCredPassword(e.target.value)} placeholder="Password" aria-label="Password" />
              <button className="btn-ghost" onClick={addCredential} disabled={!credDomain || !credUser || !credPassword || busy === 'cred'}>Store</button>
            </div>
          </section>

          {/* Device — locked */}
          <section>
            <div className="mb-1.5 flex items-center gap-1.5">
              <Lock className="h-3 w-3 text-text-dim" />
              <h3 className="label mb-0">Device — fixed for this profile</h3>
            </div>
            <Preview fp={p.fingerprint} title="Fingerprint" />
            <p className="mt-1.5 text-[11.5px] text-text-muted">
              Clone this profile to create a new device identity with an empty login state. The original device stays consistent across sessions.
            </p>
          </section>
        </div>
      </Dialog>

      <Confirm open={confirmDelete} onClose={() => setConfirmDelete(false)} onConfirm={remove} danger busy={busy === 'delete'}
        title={`Delete ${p.name}?`} confirmLabel="Delete persona"
        body={<>Its cookie jar and any stored second factor go with it. Browsers cannot start as it again.</>} />
    </>
  );
}
