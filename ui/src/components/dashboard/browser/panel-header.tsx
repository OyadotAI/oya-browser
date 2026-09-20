/**
 * The panel's header: health, name, id, provider and persona, then Connect,
 * Stop and Close.
 */
import { Check, Copy, Plug, Square, X } from 'lucide-react';
import Kbd from '@/components/ui/kbd';
import { shortId } from '@/lib/api-client';
import type { BrowserDetail } from '../types';
import { HEALTH_LABEL, providerLabel } from '../types';

/** The browser and the header's actions. */
export interface PanelHeaderProps {
  /** The browser shown. */
  browserId: string;
  /** Latest detail, or null before the first poll. */
  d: BrowserDetail | null;
  /** Whether the id was just copied. */
  copied: boolean;
  /** Copies the id. */
  copyId: () => void;
  /** Opens a persona in its drawer. */
  onOpenPersona: (id: string) => void;
  /** Opens the connect dialog. */
  onConnect: (id: string) => void;
  /** Stops the given browsers. */
  onStop: (ids: string[]) => void;
  /** Closes the panel. */
  onClose: () => void;
}

/** Id (click to copy), provider, and the persona link. */
function Meta({ browserId, d, copied, copyId, onOpenPersona }: PanelHeaderProps) {
  return (
    <div className="mt-0.5 flex flex-wrap items-center gap-x-3 gap-y-0.5 text-[12px] text-text-muted">
      <button className="font-mono hover:text-text" onClick={copyId} title="Copy id">
        {shortId(browserId)}{' '}
        {copied ? <Check className="inline h-3 w-3 text-accent" /> : <Copy className="inline h-3 w-3" />}
      </button>
      {d && <span>{providerLabel(d.provider)}</span>}
      {d?.persona && (
        <button className="hover:text-text" onClick={() => onOpenPersona(d.persona!)} title="Open persona">
          {d.personaName || shortId(d.persona)} ↗
        </button>
      )}
    </div>
  );
}

/** Who the browser is, and what can be done to it from here. */
export default function PanelHeader(p: PanelHeaderProps) {
  const { d, browserId } = p;
  return (
    <div className="flex items-start gap-3 border-b border-border px-4 py-3">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          {d && <span className={`dot dot-${d.health}`} title={HEALTH_LABEL[d.health]} />}
          <h2 className="truncate text-[15px] font-semibold text-text">{d?.name || '…'}</h2>
        </div>
        <Meta {...p} />
      </div>
      <button
        className="btn-ghost h-7"
        onClick={() => p.onConnect(browserId)}
        title="Code, Playwright, MCP — for this browser"
      >
        <Plug className="h-3 w-3" /> Connect
      </button>
      <button
        className="btn-danger h-7"
        onClick={() => p.onStop([browserId])}
        title={d?.provider === 'oya-cloud' ? 'Destroys the sandbox' : 'Stops this browser'}
      >
        <Square className="h-3 w-3" /> Stop <Kbd>X</Kbd>
      </button>
      <button className="btn-icon" onClick={p.onClose} aria-label="Close panel">
        <X className="h-4 w-4" />
      </button>
    </div>
  );
}
