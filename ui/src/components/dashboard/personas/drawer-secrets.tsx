/**
 * The profile drawer's stored secrets: second factors and portal sign-ins.
 * Both list what is stored (never the secret) and offer a form to add more.
 */
'use client';

import type { ReactNode } from 'react';
import { Lock, ShieldCheck, ShieldOff, Trash2 } from 'lucide-react';
import { mfaReady } from './mfa';
import MfaFields from './mfa-fields';
import { addCredential, clearFactor, removeCredential, storeFactor } from './drawer-actions';
import type { DrawerCtx } from './use-persona-drawer';

/** One stored factor: its type, where it applies, and a remove button. */
function FactorRow({
  type,
  where,
  onRemove,
  busy,
}: {
  /** The factor's type. */
  type?: string;
  /** Where it applies. */
  where: ReactNode;
  /** Removes it. */
  onRemove: () => void;
  /** A factor change is running. */
  busy: boolean;
}) {
  return (
    <div className="flex items-center gap-2 border-b border-border px-3 py-2 last:border-0">
      <ShieldCheck className="h-4 w-4 text-accent" />
      <span className="text-text">{type?.toUpperCase()}</span>
      <span className="text-text-dim">{where}</span>
      <button className="btn-ghost ml-auto h-7" onClick={onRemove} disabled={busy}>
        <ShieldOff className="h-3.5 w-3.5" /> Remove
      </button>
    </div>
  );
}

/** The default factor and the per-site ones, then the form for another. */
export function MfaSection(ctx: DrawerCtx) {
  const { d, p } = ctx;
  const busy = d.busy === 'mfa';
  return (
    <section>
      <h3 className="label">Second factor</h3>
      {/* A persona can hold one default factor and one per site, so the form stays open either way. */}
      {p.mfa?.configured || p.sites?.mfa?.length ? (
        <div className="mb-2 rounded-md border border-border bg-bg text-[12.5px]">
          {p.mfa?.configured && !p.mfa.domain && (
            <FactorRow
              type={p.mfa.type}
              where="· every site · the secret is never shown"
              onRemove={() => clearFactor(ctx)}
              busy={busy}
            />
          )}
          {p.sites?.mfa?.map((f) => (
            <FactorRow
              key={f.domain}
              type={f.type}
              where={<>· {f.domain}</>}
              onRemove={() => clearFactor(ctx, f.domain)}
              busy={busy}
            />
          ))}
        </div>
      ) : null}
      <div className="space-y-2">
        <MfaFields value={d.mfa} onChange={d.setMfa} />
        <button className="btn-ghost" onClick={() => storeFactor(ctx)} disabled={!mfaReady(d.mfa) || busy}>
          Store factor
        </button>
      </div>
    </section>
  );
}

/** The stored sign-ins, each removable. */
function CredentialList(ctx: DrawerCtx) {
  const { d, p } = ctx;
  if (!p.sites?.credentials?.length) return null;
  return (
    <div className="mb-2 rounded-md border border-border bg-bg text-[12.5px]">
      {p.sites.credentials.map((c) => (
        <div key={c.domain} className="flex items-center gap-2 border-b border-border px-3 py-2 last:border-0">
          <Lock className="h-3.5 w-3.5 text-accent" />
          <span className="text-text">{c.domain}</span>
          <span className="text-text-dim">· {c.username}</span>
          <button
            className="btn-ghost ml-auto h-7"
            onClick={() => removeCredential(ctx, c.domain)}
            disabled={d.busy === 'cred'}
          >
            <Trash2 className="h-3.5 w-3.5" /> Remove
          </button>
        </div>
      ))}
    </div>
  );
}

/** Portal sign-ins: for portals that drop the session between runs. */
export function SignInsSection(ctx: DrawerCtx) {
  const { cred, setCred, busy } = ctx.d;
  return (
    <section>
      <h3 className="label">Portal sign-ins</h3>
      <p className="mb-2 text-xs text-text-muted">
        Signing in on the desktop and inheriting the cookies is still the better path. Store a login only for portals
        that drop the session between runs. Passwords are sealed and never shown again.
      </p>
      <CredentialList {...ctx} />
      <div className="grid grid-cols-[1fr_1fr_1fr_auto] gap-2">
        <input
          className="field"
          value={cred.domain}
          onChange={(e) => setCred({ ...cred, domain: e.target.value })}
          placeholder="portal.example.com"
          aria-label="Site"
        />
        <input
          className="field"
          autoComplete="off"
          value={cred.username}
          onChange={(e) => setCred({ ...cred, username: e.target.value })}
          placeholder="Username"
          aria-label="Username"
        />
        <input
          className="field"
          type="password"
          autoComplete="new-password"
          value={cred.password}
          onChange={(e) => setCred({ ...cred, password: e.target.value })}
          placeholder="Password"
          aria-label="Password"
        />
        <button
          className="btn-ghost"
          onClick={() => addCredential(ctx)}
          disabled={!cred.domain || !cred.username || !cred.password || busy === 'cred'}
        >
          Store sign-in
        </button>
      </div>
    </section>
  );
}
