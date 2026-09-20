/**
 * The "Start a browser" dialog: which identity, how many, and — only if the
 * key allows it — where. Logic lives in start/use-start-browser.ts.
 */
'use client';

import { Loader2 } from 'lucide-react';
import Dialog from '@/components/ui/dialog';
import type { Persona } from './types';
import { providerLabel } from './types';
import { NameCountFields, ProfileField, ProviderField } from './start/fields';
import { unavailableReason } from './start/start-run';
import { useStartBrowser, type ProviderOption } from './start/use-start-browser';

/** What the dashboard hands the dialog. */
interface Props {
  /** Shows the dialog. */
  open: boolean;
  /** Closes it. */
  onClose: () => void;
  /** The key browsers start under. */
  apiKey: string;
  /** The key's personas. */
  personas: Persona[];
  /** The key's default provider. */
  defaultProvider: string;
  /** Every provider and whether it is ready. */
  providers: ProviderOption[];
  /** At least one browser started. */
  onStarted: () => void;
}

/** Which identity, how many, and — only if the key allows it — where. */
export default function StartBrowser({
  open,
  onClose,
  apiKey,
  personas,
  defaultProvider,
  providers,
  onStarted,
}: Props) {
  const s = useStartBrowser({ apiKey, defaultProvider, providers, onStarted, onClose });
  const { form } = s;
  const configured = providers.filter((p) => p.configured);
  const footer = <StartFooter busy={s.busy} ready={s.ready} count={form.count} onClose={onClose} onStart={s.start} />;
  return (
    <Dialog
      open={open}
      onClose={() => {
        if (!s.busy) onClose();
      }}
      title="Start a browser"
      size="sm"
      description={`On ${providerLabel(form.provider || defaultProvider)} — change the default in Settings.`}
      footer={footer}
    >
      <fieldset className="space-y-4" disabled={s.busy}>
        {(!s.ready || s.error) && (
          <p role="alert" className="rounded-md border border-yellow/30 bg-yellow/10 p-3 text-sm text-text-secondary">
            {s.error || unavailableReason(s.selectedProvider)}
          </p>
        )}
        <ProfileField value={form.persona} onChange={form.setPersona} personas={personas} />
        <NameCountFields name={form.name} onName={form.setName} count={form.count} onCount={form.setCount} />
        {configured.some((p) => p.id !== defaultProvider) && (
          <ProviderField
            value={form.provider}
            onChange={(v) => {
              form.setProvider(v);
              s.setError('');
            }}
            defaultProvider={defaultProvider}
            configured={configured}
          />
        )}
      </fieldset>
    </Dialog>
  );
}

/** The footer's state and actions. */
interface FooterProps {
  /** A start is running. */
  busy: boolean;
  /** The provider can start browsers. */
  ready: boolean;
  /** How many will start. */
  count: number;
  /** Cancels. */
  onClose: () => void;
  /** Starts. */
  onStart: () => void;
}

/** Cancel, and a Start button that says how many. */
function StartFooter({ busy, ready, count, onClose, onStart }: FooterProps) {
  return (
    <>
      <button className="btn-ghost" onClick={onClose} disabled={busy}>
        Cancel
      </button>
      <button className="btn-primary" onClick={onStart} disabled={busy || !ready}>
        {busy && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
        {count > 1 ? `Start ${count}` : 'Start'}
      </button>
    </>
  );
}
