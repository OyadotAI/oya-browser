/**
 * The Playbooks tab's title, count and Record button.
 */
'use client';

import { CircleDot } from 'lucide-react';

/** Props for the header. */
interface Props {
  /** How many playbooks there are. */
  count: number;
  /** Recording needs a running browser. */
  hasBrowsers: boolean;
  /** Opens the recording dialog. */
  onRecord: () => void;
}

/** The title, count and Record button. */
export default function TabHeader({ count, hasBrowsers, onRecord }: Props) {
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
