/**
 * A device at a glance: the fingerprint fields a site sees, or a spinner while
 * the server works one out.
 */
'use client';

import type { ReactNode } from 'react';
import { Loader2 } from 'lucide-react';
import { platformLabel } from '../types';
import type { Fingerprint } from './model';

/** A label and its value. */
interface RowProps {
  /** What the value is. */
  k: string;
  /** The value; a string also becomes its tooltip, since long ones are truncated. */
  v: ReactNode;
}

/** One label and value line. */
function Row({ k, v }: RowProps) {
  return (
    <div className="flex justify-between gap-3 border-b border-border/60 py-1.5 last:border-0">
      <span className="text-text-muted">{k}</span>
      <span className="truncate text-right text-text" title={typeof v === 'string' ? v : undefined}>
        {v}
      </span>
    </div>
  );
}

/** What the card shows. */
interface PreviewProps {
  /** The device, or null while it is being worked out. */
  fp: Fingerprint | null;
  /** The card's heading. */
  title?: string;
}

/** The fingerprint's rows. */
function Fields({ fp }: { /** The device. */ fp: Fingerprint }) {
  return (
    <>
      <Row k="Platform" v={platformLabel(fp.platform)} />
      <Row k="Timezone" v={fp.timezone} />
      <Row k="Locale" v={fp.locale} />
      <Row k="Screen" v={fp.screen} />
      <Row k="GPU" v={<span className="font-mono text-[11px]">{fp.webgl}</span>} />
      <Row k="Cores · RAM" v={`${fp.hardwareConcurrency} · ${fp.deviceMemory} GB`} />
    </>
  );
}

/** The device card; `fp` is null while the preview is on its way. */
export default function Preview({ fp, title = 'This device' }: PreviewProps) {
  return (
    <div className="rounded-lg border border-border bg-bg p-3 text-[12.5px]">
      <div className="mb-2 flex items-center justify-between">
        <span className="label mb-0">{title}</span>
        {!fp && <Loader2 className="h-3 w-3 animate-spin text-text-dim" />}
      </div>
      {fp ? <Fields fp={fp} /> : <div className="py-6 text-center text-text-dim">Preview…</div>}
    </div>
  );
}
