/**
 * Moving a profile's logins: export its cookie jar to a file, import one, or
 * copy another profile's. The jar is the profile's sessions, so a login made
 * once can follow the person to wherever it is needed.
 */
import { useEffect, useState } from 'react';
import type { Persona } from '../types';
import {
  cookiesIn,
  downloadJar,
  exportedNote,
  fetchJar,
  fetchOtherProfiles,
  mergedNote,
  putJar,
  type Cookie,
} from './logins-api';
import { useBusy } from './use-busy';

/** The other profiles on this key, for "copy from". */
function useOtherProfiles(apiKey: string, id: string) {
  const [others, setOthers] = useState<Persona[]>([]);
  useEffect(() => {
    let live = true;
    void fetchOtherProfiles(apiKey, id).then((list) => live && setOthers(list));
    return () => void (live = false);
  }, [apiKey, id]);
  return others;
}

/** What the section's actions work with. */
interface LoginsCtx {
  /** The caller's API key. */
  apiKey: string;
  /** The profile whose logins move. */
  persona: Persona;
  /** Shows the last outcome, in words. */
  setNote: (note: string) => void;
  /** Asks the tab to reload after logins came in. */
  onChanged: () => void;
}

/** Merges cookies into this profile and says what happened. */
async function bringIn({ apiKey, persona, setNote, onChanged }: LoginsCtx, cookies: Cookie[]) {
  setNote(mergedNote(await putJar(apiKey, persona.id, cookies)));
  onChanged();
}

/** Saves this profile's cookies as a file and says how many. */
async function exportJar({ apiKey, persona, setNote }: LoginsCtx) {
  const cookies = await fetchJar(apiKey, persona.id);
  downloadJar(persona.name, cookies);
  setNote(exportedNote(cookies.length));
}

/** The three actions, each run as the section's one busy action. */
function loginActions(ctx: LoginsCtx, from: string, run: ReturnType<typeof useBusy>['run']) {
  return {
    exportJar: () => run('export', () => exportJar(ctx)),
    importFile: (file: File) => run('import', async () => bringIn(ctx, cookiesIn(await file.text()))),
    copyFrom: () => run('copy', async () => bringIn(ctx, await fetchJar(ctx.apiKey, from))),
  };
}

/** The section's state and its three actions; `note` is the last outcome, in words. */
export function useLogins(apiKey: string, persona: Persona, onChanged: () => void) {
  const { busy, run } = useBusy();
  const [note, setNote] = useState('');
  const [from, setFrom] = useState('');
  const others = useOtherProfiles(apiKey, persona.id);
  const actions = loginActions({ apiKey, persona, setNote, onChanged }, from, run);
  return { busy, note, from, setFrom, others, ...actions };
}
