/**
 * The switcher's forms: create, join, add or restore a key, rename, delete.
 * What each one says and asks for is looked up by form.
 */
'use client';

import type { ReactNode } from 'react';
import { Loader2 } from 'lucide-react';
import { show } from './actions';
import { FIELD_CLASS, NAME_MAX_LENGTH } from './constants';
import { submit } from './submit';
import type { FormProps, PickerState, SubmitForm } from './types';
import { isRestoring, submitLabel } from './view';

/** The line above each form's fields. */
const INTRO: Record<SubmitForm, (ui: PickerState) => ReactNode> = {
  new: () => 'A home for your browsers, API key, and team.',
  rename: () => 'Choose a name that’s easy to find.',
  join: () => 'Paste the invitation code shared by your team.',
  import: (ui) =>
    isRestoring(ui) ? (
      <>
        Paste the API key <strong className="font-medium text-text">{ui.target?.name}</strong> was created with. Its
        browsers, profiles and settings are kept.
      </>
    ) : (
      'Connect an existing project or restore access with its original API key.'
    ),
  delete: (ui) => (
    <>
      Delete <strong className="font-medium text-text">{ui.target?.name}</strong>? Its running browsers are stopped, and
      every API key, invite and member loses access. This can’t be undone.
    </>
  ),
};

/** The name field, for creating, adding (optional) and renaming. */
function NameField({ c, form }: FormProps) {
  if (!(form === 'new' || (form === 'import' && !isRestoring(c.ui)) || form === 'rename')) return null;
  return (
    <label className="block space-y-2">
      <span className="text-xs font-medium text-text">
        Project name {form === 'import' && <span className="font-normal text-text-dim">(optional)</span>}
      </span>
      <input
        autoFocus
        maxLength={NAME_MAX_LENGTH}
        required={form === 'rename'}
        className={FIELD_CLASS}
        placeholder="e.g. Checkout agents"
        value={c.ui.name}
        onChange={(e) => c.patch({ name: e.target.value })}
      />
    </label>
  );
}

/** The invitation code or API key field. */
function SecretField({ c, form }: FormProps) {
  if (form !== 'join' && form !== 'import') return null;
  const join = form === 'join';
  return (
    <label className="block space-y-2">
      <span className="text-xs font-medium text-text">{join ? 'Invitation code' : 'API key'}</span>
      <input
        autoFocus={join || isRestoring(c.ui)}
        required
        type={join ? 'text' : 'password'}
        className={`${FIELD_CLASS} font-mono`}
        placeholder={join ? 'Paste invitation code' : 'Paste your API key'}
        value={c.ui.secret}
        onChange={(e) => c.patch({ secret: e.target.value })}
      />
    </label>
  );
}

/** Cancel goes back to the target's options, or to the list; submit runs the form. */
function Buttons({ c, form }: FormProps) {
  const { busy, target } = c.ui,
    back = target && !isRestoring(c.ui);
  return (
    <div className="flex gap-2 pt-1">
      <button
        type="button"
        disabled={busy}
        onClick={() => show(c, back ? 'manage' : null, back ? target : null)}
        className="h-10 rounded-lg border border-border px-4 text-sm text-text-muted hover:bg-text/5"
      >
        Cancel
      </button>
      <button
        disabled={busy}
        className={`flex h-10 flex-1 items-center justify-center gap-2 rounded-lg px-3 text-sm font-medium disabled:opacity-50 ${form === 'delete' ? 'bg-red-500/12 text-red-400 hover:bg-red-500/20' : 'bg-accent text-accent-foreground hover:bg-accent-hover'}`}
      >
        {busy && <Loader2 className="h-4 w-4 animate-spin" />}
        {submitLabel(c.ui)}
      </button>
    </div>
  );
}

/** The open form. */
export default function ProjectForm({ c, form }: FormProps) {
  return (
    <form
      className="space-y-4 p-5"
      onSubmit={(e) => {
        e.preventDefault();
        void submit(c);
      }}
    >
      <p className="text-xs leading-relaxed text-text-muted">{INTRO[form](c.ui)}</p>
      <NameField c={c} form={form} />
      <SecretField c={c} form={form} />
      <Buttons c={c} form={form} />
    </form>
  );
}
