/**
 * The new-profile dialog, and the facade for the second-factor and device
 * preview pieces the profile drawer shares with it.
 */
'use client';

import { Loader2 } from 'lucide-react';
import Dialog from '@/components/ui/dialog';
import type { Persona } from './types';
import { DeviceFields, LimitFields } from './personas/form-fields';
import { usePersonaForm } from './personas/use-persona-form';
import MfaFields from './personas/mfa-fields';
import Preview from './personas/device-preview';

export { MfaFields, Preview };
export { newMfa, mfaBody, mfaReady, type MfaDraft } from './personas/mfa';

/** What the dialog needs from the tab that opens it. */
interface Props {
  /** Whether it is showing. */
  open: boolean;
  /** Closes it. */
  onClose: () => void;
  /** Key the profile is created under. */
  apiKey: string;
  /** Hears about the new profile. */
  onCreated: (p: Persona) => void;
}

/**
 * A persona is one device. This is the only moment its device is chosen,
 * afterwards those fields are locked, because a device that changes under an
 * existing cookie jar is the tell the whole model exists to avoid.
 */
export default function PersonaForm({ open, onClose, apiKey, onCreated }: Props) {
  const { draft, set, opts, preview, busy, create } = usePersonaForm({ open, apiKey, onCreated, onClose });
  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="New profile"
      size="lg"
      description="One identity: a fingerprint, a cookie jar and a proxy, bound together and stable for its life."
      footer={
        <>
          <button className="btn-ghost" onClick={onClose}>
            Cancel
          </button>
          <button className="btn-primary" onClick={create} disabled={busy}>
            {busy && <Loader2 className="h-3.5 w-3.5 animate-spin" />}Create profile
          </button>
        </>
      }
    >
      <div className="grid gap-6 md:grid-cols-[1fr_260px]">
        <div className="space-y-4">
          <div>
            <label className="label" htmlFor="pf-name">
              Name
            </label>
            <input
              id="pf-name"
              className="field"
              value={draft.name}
              onChange={(e) => set({ name: e.target.value })}
              placeholder="e.g. acme-ops"
              autoFocus
            />
          </div>
          <DeviceFields draft={draft} set={set} opts={opts} />
          <LimitFields draft={draft} set={set} />
          <fieldset>
            <legend className="label">
              Second factor <span className="normal-case tracking-normal text-text-dim">(optional)</span>
            </legend>
            <MfaFields value={draft.mfa} onChange={(mfa) => set({ mfa })} none />
          </fieldset>
        </div>
        <Preview fp={preview} />
      </div>
    </Dialog>
  );
}
