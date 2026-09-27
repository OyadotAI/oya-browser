/**
 * The Playbooks tab's title, count, and its Import and Record buttons.
 */
'use client';

import { CircleDot, Upload } from 'lucide-react';
import { useRef } from 'react';

/** Props for the header. */
interface Props {
  /** How many playbooks there are. */
  count: number;
  /** Recording needs a running browser. */
  hasBrowsers: boolean;
  /** Opens the recording dialog. */
  onRecord: () => void;
  /** Imports a playbook export file. */
  onImport: (file: File) => void;
}

/** Props for the import button. */
interface ImportProps {
  /** Imports the chosen file. */
  onImport: (file: File) => void;
}

/** A button that picks a playbook export file to import. */
function ImportButton({ onImport }: ImportProps) {
  const input = useRef<HTMLInputElement>(null);
  return (
    <>
      <button className="btn-ghost" onClick={() => input.current?.click()} title="Import a playbook exported elsewhere">
        <Upload className="h-4 w-4" /> Import
      </button>
      <input
        ref={input}
        type="file"
        accept=".json,application/json"
        className="hidden"
        aria-label="Playbook export file"
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) onImport(file);
          e.target.value = '';
        }}
      />
    </>
  );
}

/** The title, count and Record button. */
export default function TabHeader({ count, hasBrowsers, onRecord, onImport }: Props) {
  return (
    <div className="flex shrink-0 flex-wrap items-center gap-3 px-4 py-5 lg:px-6">
      <div className="mr-auto">
        <h2 className="text-[22px] font-medium tracking-tight text-text">
          Playbooks <span className="ml-2 text-[14px] text-text-dim">{count}</span>
        </h2>
        <p className="text-[12px] text-text-muted">
          Flows recorded from an ask() run, or from you doing it yourself. Replayed without the LLM; values you enter or
          select are variables you can override.
        </p>
      </div>
      <ImportButton onImport={onImport} />
      <button
        className="btn-primary"
        onClick={onRecord}
        disabled={!hasBrowsers}
        title={
          hasBrowsers ? 'Do the task yourself in a live browser and keep it as a playbook' : 'Start a browser first'
        }
      >
        <CircleDot className="h-4 w-4" /> Record a flow
      </button>
    </div>
  );
}
