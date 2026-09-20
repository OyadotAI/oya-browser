/**
 * One settings editing session: the dialog, its section tabs, and the open
 * panel. State and saving live in useSettingsEditor.
 */
'use client';

import { useState } from 'react';
import Dialog from '@/components/ui/dialog';
import type { Section } from './constants';
import EditorFooter from './editor-footer';
import { ErrorNote, Loading } from './heading';
import { PANELS } from './panels';
import SectionTabs from './section-tabs';
import { useSettingsEditor } from './use-settings-editor';

/** What the dashboard passes the settings dialog. */
export interface SettingsProps {
  /** Whether the dialog is showing. */
  open: boolean;
  /** Closes it. */
  onClose: () => void;
  /** The key whose settings are edited. */
  apiKey: string;
  /** Restarts onboarding; offers a "Run setup again" link when given. */
  onRerunSetup?: () => void;
  /** The section to open on. */
  initialSection?: Section;
}

/** The dialog for one session. */
export default function SettingsEditor({ onClose, apiKey, onRerunSetup, initialSection }: SettingsProps) {
  const [section, setSection] = useState<Section>(initialSection || 'model');
  const editor = useSettingsEditor(apiKey, onClose);
  const { config, error, saving } = editor;
  const rerun = onRerunSetup && (() => (editor.close(), onRerunSetup()));
  return (
    <Dialog
      open
      onClose={editor.close}
      title="Settings"
      description="Your defaults. Every browser, ready to work."
      size="lg"
      footer={<EditorFooter editor={editor} />}
    >
      <div className="-mx-5 -my-4 grid min-h-[390px] min-w-0 md:grid-cols-[156px_minmax(0,1fr)]">
        <SectionTabs section={section} onSelect={setSection} onRerun={rerun} saving={saving} />
        <div className="min-w-0 px-5 py-6 sm:px-7">
          {error && (
            <ErrorNote>
              {error}
              {!config && (
                <button className="ml-3 underline" onClick={editor.retry}>
                  Retry
                </button>
              )}
            </ErrorNote>
          )}
          {!config ? (
            <Loading>Loading your preferences…</Loading>
          ) : (
            <fieldset
              disabled={saving}
              role="tabpanel"
              id={`settings-panel-${section}`}
              aria-labelledby={`settings-tab-${section}`}
              className="min-w-0"
            >
              {PANELS[section]({ editor, config, apiKey })}
            </fieldset>
          )}
        </div>
      </div>
    </Dialog>
  );
}
