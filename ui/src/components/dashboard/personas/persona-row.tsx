/**
 * One row of the profiles table: name, saved sites, running against its cap,
 * device, exit, second factor, and when it was last used and created.
 */
'use client';

import { ShieldCheck } from 'lucide-react';
import { ago } from '@/lib/api-client';
import type { BrowserRow, Persona } from '../types';
import { platformLabel } from '../types';
import { rowStats } from './model';

/** What a row shows and does. */
interface RowProps {
  /** The profile. */
  p: Persona;
  /** Every connected browser, to count the ones running as it. */
  browsers: BrowserRow[];
  /** Whether its drawer is open. */
  selected: boolean;
  /** Opens its drawer. */
  onOpen: (id: string) => void;
  /** The clock the relative times are measured against. */
  now: number;
}

/** The exit it uses: a pinned proxy, an auto geo hint, or a direct connection. */
function Exit({ p }: Pick<RowProps, 'p'>) {
  if (p.exit)
    return (
      <span title={p.exit.label}>
        {p.exit.label}
        {p.exit.geo ? ` · ${p.exit.geo}` : ''}
      </span>
    );
  if (p.proxy?.geo) return <span className="text-text-muted">{p.proxy.geo} (auto)</span>;
  return <span className="text-text-dim">direct</span>;
}

/** The name cell: a button that opens the drawer, the default badge, and the id. */
function NameCell({ p, onOpen }: Pick<RowProps, 'p' | 'onOpen'>) {
  return (
    <td className="px-2 py-3 pl-4">
      <div className="flex min-w-0 items-center gap-2">
        <button
          className="truncate text-left font-medium text-text hover:text-accent"
          title={p.name}
          onClick={(e) => {
            e.stopPropagation();
            onOpen(p.id);
          }}
        >
          {p.name}
        </button>
        {p.isDefault && (
          <span className="shrink-0 rounded border border-border px-1 text-[10px] uppercase tracking-wider text-text-muted">
            default
          </span>
        )}
      </div>
      <div className="mt-1 truncate font-mono text-[11px] text-text-dim">{p.id}</div>
    </td>
  );
}

/** One profile; clicking anywhere on it opens its drawer. */
export default function PersonaRow({ p, browsers, selected, onOpen, now }: RowProps) {
  const { cap, at, running } = rowStats(p, browsers);
  return (
    <tr
      className={`cursor-pointer border-b border-border/60 hover:bg-text/[0.035] ${selected ? 'bg-accent/[0.08]' : ''}`}
      onClick={() => onOpen(p.id)}
    >
      <NameCell p={p} onOpen={onOpen} />
      <td className="max-w-[240px] px-2 py-3 text-text-secondary">
        <span className="block truncate" title={p.login?.sites.join(', ')}>
          {p.login?.sites.length ? p.login.sites.join(', ') : 'No accounts saved'}
        </span>
      </td>
      <td className="px-2 py-3 text-right num">
        <span className={at ? 'text-yellow' : running ? 'text-accent' : 'text-text-muted'}>{running}</span>
        <span className="text-text-dim"> / {cap === Infinity ? '∞' : cap}</span>
      </td>
      <td className="truncate px-3 py-3 text-text-secondary">
        <span className="block truncate">
          {platformLabel(p.fingerprint.platform)} · {p.fingerprint.screen}
        </span>
        <span className="mt-1 block truncate text-[11px] text-text-dim">{p.fingerprint.timezone}</span>
      </td>
      <td className="truncate px-3 py-3 text-text-secondary">
        <Exit p={p} />
      </td>
      <td className="px-2 py-3">
        {p.mfa?.configured ? (
          <span className="inline-flex items-center gap-1 text-[12px] text-accent">
            <ShieldCheck className="h-3.5 w-3.5" />
            {p.mfa.type}
          </span>
        ) : (
          <span className="text-text-dim">—</span>
        )}
      </td>
      <td className="px-2 py-3 text-right num text-text-muted">{p.lastUsedAt ? ago(p.lastUsedAt, now) : '—'}</td>
      <td className="px-2 py-3 text-right num text-text-muted">{ago(p.createdAt, now)}</td>
    </tr>
  );
}
