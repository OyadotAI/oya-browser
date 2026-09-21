/**
 * "Move logins" in the profile drawer: export this profile's sessions to a
 * file, import them from one, or copy them from another profile.
 */
'use client';

import { Download, Upload, Copy } from 'lucide-react';
import type { Persona } from '../types';
import { useLogins } from './use-logins';

/** Props for the section. */
interface Props {
  /** The caller's API key. */
  apiKey: string;
  /** The profile whose logins move. */
  persona: Persona;
  /** Asks the tab to reload after logins came in. */
  onChanged: () => void;
}

/** Export to a file, import from one, copy from another profile. */
export function LoginsSection({ apiKey, persona, onChanged }: Props) {
  const l = useLogins(apiKey, persona, onChanged);
  return (
    <section>
      <h3 className="label">Move logins</h3>
      <p className="mb-2 text-xs text-text-muted">
        A profile&apos;s cookies are its sessions. Take them to a script or another tool, or bring them in, without
        signing in to each site again.
      </p>
      <div className="flex flex-wrap items-center gap-2">
        <button className="btn-ghost" onClick={l.exportJar} disabled={!!l.busy}>
          <Download className="h-3.5 w-3.5" /> Export
        </button>
        <label className={`btn-ghost cursor-pointer ${l.busy ? 'pointer-events-none opacity-50' : ''}`}>
          <Upload className="h-3.5 w-3.5" /> Import
          <input
            type="file"
            accept="application/json,.json"
            className="sr-only"
            aria-label="Import a cookie file"
            onChange={(e) => e.target.files?.[0] && l.importFile(e.target.files[0])}
          />
        </label>
        <select
          className="field w-auto"
          value={l.from}
          onChange={(e) => l.setFrom(e.target.value)}
          aria-label="Copy logins from another profile"
        >
          <option value="">Copy from profile…</option>
          {l.others.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
        <button className="btn-ghost" onClick={l.copyFrom} disabled={!l.from || !!l.busy}>
          <Copy className="h-3.5 w-3.5" /> Copy
        </button>
      </div>
      {l.note && (
        <p className="mt-2 text-xs text-text-secondary" role="status">
          {l.note}
        </p>
      )}
    </section>
  );
}
