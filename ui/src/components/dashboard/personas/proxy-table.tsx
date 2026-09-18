/**
 * The proxies table: each exit with its country, last exit IP, load and
 * health, and a two-click remove for the ones this key owns.
 */
'use client';

import { Loader2, Trash2 } from 'lucide-react';
import { PROXY_TABLE_COLUMNS } from './constants';
import { remove } from './proxy-actions';
import type { PartProps, ProxyPartProps } from './use-proxies';

/** Remove, which asks "Remove?" first and spins while it runs. */
function RemoveButton({ s, p }: ProxyPartProps) {
  const confirming = s.confirming === p.id;
  return (
    <button
      className={confirming ? 'btn-ghost h-7 text-[12px] text-red' : 'btn-ghost h-7 w-7 p-0'}
      onClick={() => remove(s, p)}
      disabled={s.busy === p.id}
      aria-label={`Remove ${p.label}`}
    >
      {s.busy === p.id ? (
        <Loader2 className="h-3.5 w-3.5 animate-spin" />
      ) : confirming ? (
        'Remove?'
      ) : (
        <Trash2 className="h-3.5 w-3.5" />
      )}
    </button>
  );
}

/** One proxy. */
function Row({ s, p }: ProxyPartProps) {
  return (
    <tr className="border-b border-border/60 last:border-0">
      <td className="px-3 py-2">
        <div className="font-medium text-text">{p.label}</div>
        <div className="text-[11px] text-text-dim">
          {p.kind}
          {p.shared ? ' · shared' : ''}
        </div>
      </td>
      <td className="px-2 py-2 text-text-secondary">{p.geo || 'any'}</td>
      <td className="px-2 py-2 font-mono text-[12px] text-text-secondary">
        {p.exitIp || <span className="text-text-dim">not checked</span>}
      </td>
      <td className="px-2 py-2 text-right num text-text-secondary">
        {p.assigned} / {p.maxPersonas}
      </td>
      <td className="px-2 py-2">
        {p.available ? (
          <span className="text-accent">ok</span>
        ) : (
          <span className="text-yellow">{p.healthy ? 'cooling down' : 'failing'}</span>
        )}
      </td>
      <td className="px-2 py-2 text-right">{!p.shared && <RemoveButton s={s} p={p} />}</td>
    </tr>
  );
}

/** Every proxy, or a note that profiles connect directly until one is added. */
export default function ProxyTable({ s }: PartProps) {
  return (
    <div className="overflow-x-auto rounded-xl border border-border">
      <table className="w-full text-[13px]" aria-label="Proxies">
        <thead className="bg-bg-elevated">
          <tr className="border-b border-border text-left text-[11px] font-medium uppercase tracking-[0.1em] text-text-muted">
            <th className="px-3 py-2">Proxy</th>
            <th className="px-2 py-2">Country</th>
            <th className="px-2 py-2">Exit IP</th>
            <th className="px-2 py-2 text-right">Profiles</th>
            <th className="px-2 py-2">Health</th>
            <th className="w-10 px-2 py-2">
              <span className="sr-only">Remove</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {s.rows.map((p) => (
            <Row key={p.id} s={s} p={p} />
          ))}
          {s.rows.length === 0 && (
            <tr>
              <td colSpan={PROXY_TABLE_COLUMNS} className="px-3 py-6 text-center text-text-muted">
                No proxies yet. Profiles connect directly until you add one.
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
