/**
 * The playbook editor's state: the definition as text (loaded when it opens),
 * whether it parses, whether it changed, and Save, Save as and Discard. A save's
 * refusal stays in the editor in the server's words.
 */
import { useEffect } from 'react';
import { errorMessage } from '@/lib/api-client';
import { usePatchState, type Patch } from '@/lib/hooks/use-patch-state';
import { getDefinition, saveDefinition, saveDefinitionAs, type Definition } from './editor-api';
import type { PlaybookBody } from '../types';
import { parse, toText, type Parsed } from './definition';

/** The editor's state. */
interface EditorState {
  /** The text as last saved or loaded; null until loaded. */
  saved: string | null;
  /** The text being edited. */
  text: string;
  /** Why the definition could not be loaded. */
  loadError: string | null;
  /** A save is in flight. */
  busy: boolean;
  /** Why the last save was refused. */
  saveError: string | null;
  /** The Playwright code for the steps as last saved; null until a save here. */
  code: string | null;
}

/** Nothing loaded yet. */
const START: EditorState = { saved: null, text: '', loadError: null, busy: false, saveError: null, code: null };

/** Loads the definition into the text. */
const loadInto = (apiKey: string, name: string, patch: Patch<EditorState>) =>
  getDefinition(apiKey, name).then(
    (d) => patch({ saved: toText(d), text: toText(d) }),
    (e) => patch({ loadError: errorMessage(e) }),
  );

/** The text, its parse, the change state, and the save actions. */
export function usePlaybookEditor(apiKey: string, name: string, onSaved: () => void) {
  const [state, patch] = usePatchState<EditorState>(START);
  useEffect(() => void loadInto(apiKey, name, patch), [apiKey, name, patch]);
  const parsed = parse(state.text);
  const dirty = state.saved !== null && state.text !== state.saved;
  const setText = (text: string) => patch({ text });
  return { ...state, setText, parsed, dirty, ...actions({ apiKey, name, parsed, state, patch, onSaved }) };
}

/** What the save actions need. */
interface Ctx {
  /** The caller's API key. */
  apiKey: string;
  /** The playbook. */
  name: string;
  /** The text's parse. */
  parsed: Parsed;
  /** The state. */
  state: EditorState;
  /** Merges into it. */
  patch: Patch<EditorState>;
  /** Runs after a save. */
  onSaved: () => void;
}

/** Save, Save as and Discard. */
function actions(c: Ctx) {
  const def = c.parsed.definition as Definition;
  return {
    save: () => def && !c.state.busy && attempt(c, () => saveDefinition(c.apiKey, c.name, def)),
    saveAs: (as: string) => def && attempt(c, () => saveDefinitionAs(c.apiKey, as, def)),
    discard: () => c.state.saved !== null && c.patch({ text: c.state.saved }),
  };
}

/** Sends a save, keeping the server's refusal in the editor. */
async function attempt(c: Ctx, send: () => Promise<PlaybookBody>) {
  c.patch({ busy: true, saveError: null });
  const done = (body: PlaybookBody) => (c.patch((s) => ({ saved: s.text, code: body?.code ?? null })), c.onSaved());
  await send().then(done, (err) => c.patch({ saveError: errorMessage(err) }));
  c.patch({ busy: false });
}

/** What the hook gives the editor. */
export type PlaybookEditorState = ReturnType<typeof usePlaybookEditor>;
