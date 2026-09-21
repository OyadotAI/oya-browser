/**
 * The settings dialog's footer: the unsaved-changes note, Cancel and Save.
 */
'use client';

import { Check, Loader2 } from 'lucide-react';
import type { SettingsEditorState } from './use-settings-editor';

/** Status on the left, actions on the right. */
export default function EditorFooter({ editor }: { /** The editor state. */ editor: SettingsEditorState }) {
  const { form, saving, canSave, close, save } = editor;
  return (
    <>
      <div className="mr-auto flex items-center gap-2 text-[12px] text-text-muted">
        <span className={`h-1.5 w-1.5 rounded-full ${form.dirty ? 'bg-yellow' : 'bg-text-dim/50'}`} />
        <span className="hidden sm:inline">{form.dirty ? 'Unsaved changes' : 'Changes apply to this API key'}</span>
      </div>
      <button className="btn-ghost h-9" onClick={close} disabled={saving}>
        Cancel
      </button>
      <button className="btn-primary h-9 min-w-[100px]" onClick={save} disabled={!canSave}>
        {saving ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Check className="h-3.5 w-3.5" />}
        Save
      </button>
    </>
  );
}
