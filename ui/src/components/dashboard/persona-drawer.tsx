/**
 * The profile drawer: everything about one identity, and the few things about
 * it that may change. Its sections and actions live in `personas/`.
 */
'use client';

import { Loader2, Copy, Trash2, Lock } from 'lucide-react';
import Dialog, { Confirm } from '@/components/ui/dialog';
import { Preview } from './persona-form';
import { clone, remove, save } from './personas/drawer-actions';
import { RunningSection, SessionsSection } from './personas/drawer-overview';
import { MfaSection, SignInsSection } from './personas/drawer-secrets';
import SettingsSection from './personas/drawer-settings';
import { isDirty, runningAs } from './personas/model';
import { usePersonaDrawer, type DrawerCtx, type DrawerProps } from './personas/use-persona-drawer';

/** Delete (not for the default profile, nor while its browsers run), clone, and save. */
function Footer(ctx: DrawerCtx) {
  const { d, p, props } = ctx;
  const running = runningAs(props.browsers, p.id);
  return (
    <>
      {!p.isDefault && (
        <button
          className="btn-ghost mr-auto text-red hover:text-red"
          onClick={() => d.setConfirmDelete(true)}
          disabled={running.length > 0}
          title={running.length ? 'Stop its browsers first' : undefined}
        >
          <Trash2 className="h-3.5 w-3.5" /> Delete
        </button>
      )}
      <button className="btn-ghost" onClick={() => clone(ctx)} disabled={d.busy === 'clone'}>
        <Copy className="h-3.5 w-3.5" /> Clone as new profile
      </button>
      <button className="btn-primary" onClick={() => save(ctx)} disabled={!isDirty(d.fields, p) || d.busy === 'save'}>
        {d.busy === 'save' && <Loader2 className="h-3.5 w-3.5 animate-spin" />}Save
      </button>
    </>
  );
}

/** The device, locked: rotating it means cloning to a new profile. */
function DeviceSection({ p }: DrawerCtx) {
  return (
    <section>
      <div className="mb-1.5 flex items-center gap-1.5">
        <Lock className="h-3 w-3 text-text-dim" />
        <h3 className="label mb-0">Device — fixed for this profile</h3>
      </div>
      <Preview fp={p.fingerprint} title="Fingerprint" />
      <p className="mt-1.5 text-[11.5px] text-text-muted">
        Clone this profile to create a new device identity with an empty login state. The original device stays
        consistent across sessions.
      </p>
    </section>
  );
}

/** Everything about one identity, and the few things about it that may change. */
export default function PersonaDrawer(props: DrawerProps) {
  const d = usePersonaDrawer(props);
  if (!props.persona) return null;
  const ctx: DrawerCtx = { d, p: props.persona, props };
  const p = ctx.p;
  return (
    <>
      <Dialog
        open={!!p}
        onClose={props.onClose}
        title={p.name}
        size="drawer"
        description={
          <span className="font-mono">
            {p.id}
            {p.isDefault ? ' · default profile' : ''}
          </span>
        }
        footer={<Footer {...ctx} />}
      >
        <div className="space-y-5">
          <SessionsSection {...ctx} />
          <RunningSection {...ctx} />
          <SettingsSection {...ctx} />
          <MfaSection {...ctx} />
          <SignInsSection {...ctx} />
          <DeviceSection {...ctx} />
        </div>
      </Dialog>
      <Confirm
        open={d.confirmDelete}
        onClose={() => d.setConfirmDelete(false)}
        onConfirm={() => remove(ctx)}
        danger
        busy={d.busy === 'delete'}
        title={`Delete ${p.name}?`}
        confirmLabel="Delete persona"
        body={<>Its cookie jar and any stored second factor go with it. Browsers cannot start as it again.</>}
      />
    </>
  );
}
