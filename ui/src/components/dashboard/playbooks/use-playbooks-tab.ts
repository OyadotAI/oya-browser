/**
 * The Playbooks tab's state: the list, its edits, and which dialog is open.
 */
import { useState } from 'react';
import { usePlaybookActions } from './use-playbook-actions';
import { usePlaybookList } from './use-playbook-list';
import type { RowHandlers } from './playbook-row';
import type { PlaybookBody, PlaybookInfo } from './types';

/** Which of the code, run and record dialogs is open. */
function useOpenDialogs() {
  const [code, setCode] = useState<PlaybookBody | null>(null);
  const [running, setRunning] = useState<PlaybookInfo | null>(null);
  const [recording, setRecording] = useState(false);
  return { code, setCode, running, setRunning, recording, setRecording };
}

/** Everything the tab shows and does. */
export function usePlaybooksTab(apiKey: string) {
  const list = usePlaybookList(apiKey);
  const actions = usePlaybookActions(apiKey, list.refresh);
  const dialogs = useOpenDialogs();
  return { ...list, actions, dialogs, on: rowHandlers(actions, dialogs) };
}

/** Row buttons get the dialogs' openers and the actions. */
type Actions = ReturnType<typeof usePlaybookActions>;

/** What each row button opens or does. */
function rowHandlers(actions: Actions, dialogs: ReturnType<typeof useOpenDialogs>): RowHandlers {
  const { setRenaming: onRename, setRemoving: onRemove, promote: onPromote } = actions;
  return { onCode: dialogs.setCode, onRun: dialogs.setRunning, onRename, onRemove, onPromote };
}

/** The tab's state. */
export type PlaybooksTabState = ReturnType<typeof usePlaybooksTab>;
