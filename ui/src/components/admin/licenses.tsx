/**
 * Self-hosted licenses: issue one (its key is shown once, here), and see or
 * revoke the ones issued. Its state lives in `use-license-form.ts`.
 */
'use client';

import type { InputHTMLAttributes } from 'react';
import { Section, Table } from './parts';
import { dayOf } from './model';
import { useLicenseForm } from './use-license-form';
import type { AdminState } from './use-admin';
import type { License } from './types';

/** An input with its label. */
function Field({ label, ...input }: { /** What it is. */ label: string } & InputHTMLAttributes<HTMLInputElement>) {
  return (
    <label className="flex flex-col gap-1 text-xs text-text-dim">
      {label}
      <input
        {...input}
        className="h-8 rounded-md border border-border bg-bg px-2 text-sm text-text outline-none focus:border-accent"
      />
    </label>
  );
}

/** The form that issues a license, and the key it issued. */
function IssueForm({ issue }: { /** Issues a license. */ issue: AdminState['issue'] }) {
  const f = useLicenseForm(issue);
  return (
    <>
      <div className="flex flex-wrap items-end gap-2">
        <Field
          label="Licensee"
          value={f.fields.licensee}
          onChange={(e) => f.set({ licensee: e.target.value })}
          placeholder="Company name"
        />
        <Field
          label="Cloud browsers at once"
          type="number"
          min={1}
          value={f.fields.browsers}
          onChange={(e) => f.set({ browsers: e.target.value })}
        />
        <Field
          label="Expires"
          type="date"
          value={f.fields.expires}
          onChange={(e) => f.set({ expires: e.target.value })}
        />
        <button
          onClick={() => void f.submit()}
          className="h-8 rounded-md bg-accent px-3 text-xs font-medium text-white"
        >
          Issue
        </button>
      </div>
      {f.error && <p className="text-xs text-red-400">{f.error}</p>}
      {f.key && (
        <label className="flex flex-col gap-1 text-xs text-text-dim">
          License key, shown once. Send it to the licensee to set as OYA_LICENSE_KEY:
          <textarea
            readOnly
            value={f.key}
            rows={3}
            className="rounded-md border border-border bg-bg p-2 font-mono text-[11px] text-text"
          />
        </label>
      )}
    </>
  );
}

/** A license's row: who, how many, until when, by whom, and Revoke while it stands. */
const row = (l: License, revoke: AdminState['revoke']) => [
  l.licensee,
  l.max_concurrent,
  dayOf(l.expires_at),
  l.created_by || '—',
  l.revoked_at ? (
    `revoked ${dayOf(l.revoked_at)}`
  ) : (
    <button key={l.id} onClick={() => void revoke(l.id)} className="text-red-400 hover:underline">
      Revoke
    </button>
  ),
];

/** Issue a license, and the list of those issued. */
export default function Licenses({ s }: { /** The page's state. */ s: AdminState }) {
  return (
    <Section title="Self-hosted licenses">
      <IssueForm issue={s.issue} />
      <Table
        head={['Licensee', 'Browsers', 'Expires', 'Issued by', '']}
        rows={(s.data?.licenses || []).map((l) => row(l, s.revoke))}
      />
    </Section>
  );
}
