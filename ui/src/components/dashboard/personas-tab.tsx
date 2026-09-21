/**
 * The Profiles tab: every identity this key owns, with the dialogs to create
 * one, manage proxies, and open one in the drawer.
 */
'use client';

import { useState } from 'react';
import { Globe, Plus, Users } from 'lucide-react';
import type { Persona, BrowserRow } from './types';
import PersonaForm from './persona-form';
import PersonaDrawer from './persona-drawer';
import ProxiesDialog from './proxies-dialog';
import PersonaRow from './personas/persona-row';

/** What the tab needs from the dashboard. */
interface Props {
  /** Key the profiles belong to. */
  apiKey: string;
  /** Every connected browser. */
  browsers: BrowserRow[];
  /** Every profile. */
  personas: Persona[];
  /** Reloads the profiles. */
  refresh: () => void;
  /** The profile whose drawer is open. */
  openId: string | null;
  /** Opens a profile's drawer, or closes it with null. */
  onOpen: (id: string | null) => void;
  /** Shows the fleet filtered to a profile. */
  onShowBrowsers: (personaId: string) => void;
  /** The clock the relative times are measured against. */
  now: number;
}

/** The column headings. */
function Head() {
  return (
    <thead className="sticky top-0 z-10 bg-bg-elevated">
      <tr className="border-b border-border text-left text-[11px] font-medium uppercase tracking-[0.1em] text-text-muted">
        <th className="w-[22%] px-4 py-3">Profile</th>
        <th className="w-[18%] px-3 py-3">Saved sites</th>
        <th className="w-[110px] px-2 py-3 text-right">Running</th>
        <th className="px-3 py-3">Device</th>
        <th className="w-[140px] px-2 py-3">Exit</th>
        <th className="w-[70px] px-2 py-3">MFA</th>
        <th className="w-[90px] px-2 py-3 text-right">Last used</th>
        <th className="w-[90px] px-2 py-3 text-right">Created</th>
      </tr>
    </thead>
  );
}

/** Shown in place of rows before the first profile exists. */
function Empty({ onCreate }: { /** Opens the new-profile form. */ onCreate: () => void }) {
  return (
    <div className="flex flex-col items-center justify-center gap-3 py-34 text-center">
      <div className="flex h-12 w-12 items-center justify-center rounded-xl border border-border bg-bg-card">
        <Users className="h-5 w-5 text-text-muted" />
      </div>
      <p className="text-[15px] font-medium text-text">No profiles yet</p>
      <button className="btn-primary" onClick={onCreate}>
        Create one
      </button>
    </div>
  );
}

/** The heading with the profile count, and the Proxies and New profile buttons. */
function Toolbar(props: {
  /** Profiles this key owns. */
  count: number;
  /** Opens the proxies dialog. */
  onProxies: () => void;
  /** Opens the new-profile form. */
  onCreate: () => void;
}) {
  const { count, onProxies, onCreate } = props;
  return (
    <div className="flex shrink-0 flex-wrap items-center gap-3 px-4 py-5 lg:px-6">
      <div className="mr-auto">
        <h2 className="text-[22px] font-medium tracking-tight text-text">
          Profiles <span className="ml-2 text-[14px] text-text-dim">{count}</span>
        </h2>
        <p className="text-[12px] text-text-muted">
          Saved account sessions, a consistent device, and an optional second factor.
        </p>
      </div>
      <button className="btn-ghost h-9" onClick={onProxies}>
        <Globe className="h-3.5 w-3.5" /> Proxies
      </button>
      <button className="btn-primary h-9" onClick={onCreate}>
        <Plus className="h-3.5 w-3.5" /> New profile
      </button>
    </div>
  );
}

/**
 * Every identity this key owns. One identity = one device, stable for its
 * life; rotation means choosing a different row here, never editing one.
 */
export default function PersonasTab({
  apiKey,
  browsers,
  personas,
  refresh,
  openId,
  onOpen,
  onShowBrowsers,
  now,
}: Props) {
  const [creating, setCreating] = useState(false);
  const [proxies, setProxies] = useState(false);
  const open = personas.find((p) => p.id === openId) || null;

  return (
    <div className="workspace-list flex h-full min-h-0 min-w-0 flex-col">
      <Toolbar count={personas.length} onProxies={() => setProxies(true)} onCreate={() => setCreating(true)} />
      <div className="data-scroll mx-4 mb-4 min-h-0 flex-initial overflow-auto rounded-xl border border-border bg-bg-card/25 lg:mx-6 lg:mb-6">
        <table className="data-table profile-data w-full table-fixed border-collapse text-[13px]" aria-label="Profiles">
          <Head />
          <tbody>
            {personas.map((p) => (
              <PersonaRow key={p.id} p={p} browsers={browsers} selected={openId === p.id} onOpen={onOpen} now={now} />
            ))}
          </tbody>
        </table>
        {personas.length === 0 && <Empty onCreate={() => setCreating(true)} />}
      </div>
      <ProxiesDialog open={proxies} onClose={() => setProxies(false)} apiKey={apiKey} onChanged={refresh} />
      <PersonaForm open={creating} onClose={() => setCreating(false)} apiKey={apiKey} onCreated={() => refresh()} />
      <PersonaDrawer
        persona={open}
        onClose={() => onOpen(null)}
        apiKey={apiKey}
        browsers={browsers}
        onChanged={refresh}
        onShowBrowsers={onShowBrowsers}
        now={now}
      />
    </div>
  );
}
