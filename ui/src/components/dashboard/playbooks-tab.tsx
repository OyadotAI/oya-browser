/**
 * The Playbooks tab (facade): flows recorded from ask() runs, or from a person
 * doing the task in a live browser. The pieces live in ./playbooks/.
 */
'use client';

import PlaybookDialogs from './playbooks/tab-dialogs-set';
import PlaybookTable from './playbooks/playbook-table';
import TabHeader from './playbooks/tab-header';
import { usePlaybooksTab } from './playbooks/use-playbooks-tab';
import type { BrowserRow, Persona } from './types';

/** Props for the tab. */
interface Props {
  /** The caller's API key. */
  apiKey: string;
  /** Running browsers, to record and run on. */
  browsers: BrowserRow[];
  /** Profiles, to run on a browser with the right identity. */
  personas: Persona[];
  /** The clock ages are measured from. */
  now: number;
}

/**
 * Flows recorded from ask() runs. Each replays without the LLM; a replay that
 * breaks can heal itself into a draft, which waits here for review.
 */
export default function PlaybooksTab({ apiKey, browsers, personas, now }: Props) {
  const tab = usePlaybooksTab(apiKey);
  return (
    <div className="workspace-list flex h-full min-h-0 min-w-0 flex-col">
      <TabHeader
        count={(tab.playbooks || []).length}
        hasBrowsers={browsers.length > 0}
        onRecord={() => tab.dialogs.setRecording(true)}
      />
      {tab.loadError && (
        <div
          role="alert"
          className="mx-4 mb-3 rounded-lg border border-red/30 bg-red/10 px-3 py-2 text-sm text-red lg:mx-6"
        >
          Could not load playbooks: {tab.loadError}
        </div>
      )}
      <PlaybookTable playbooks={tab.playbooks} now={now} on={tab.on} />
      <PlaybookDialogs tab={tab} apiKey={apiKey} browsers={browsers} personas={personas} />
    </div>
  );
}
