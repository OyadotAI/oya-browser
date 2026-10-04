/**
 * IPC: a workflow as a JSON file. Saved as Oya's own draft (to open here again)
 * or as a Chrome DevTools Recorder recording; opened from either. What is saved
 * comes from the workspace, never from the shell, and a draft holds secret
 * names only, never their values.
 */
import fs from 'node:fs';
import type { SaveDialogOptions } from 'electron';
import type { AppServices } from '../app/services.ts';
import type { Payload } from '../../shared/ipc.ts';
import { isChromeRecording, fromChromeRecording, toChromeRecording, type Draft } from '../../workflow/index.ts';
import { JSON_INDENT } from '../app/constants.ts';
import { writePrivateFile } from './files.ts';
import { ShellDialogs } from './shell-dialogs.ts';
import { MAX_IMPORT_BYTES, SAFE_FILE_NAME } from './constants.ts';

/** The services workflow files use. */
type Deps = Pick<AppServices, 'workspace' | 'electron' | 'shell'>;

/** Applies an edit to the draft (the import), answering the new studio state. */
export type Edit = (command: Payload) => Payload;

/** The fields of a draft that make up the workflow itself, without its history or runs. */
const WORKFLOW_FIELDS = ['schemaVersion', 'name', 'description', 'steps', 'variables', 'secrets'];

/** A draft reduced to the workflow. */
const workflowOf = (draft: Draft): Record<string, unknown> =>
  Object.fromEntries(WORKFLOW_FIELDS.map((key) => [key, draft[key]]));

/** A file's contents, and the suffix its default name gets. */
interface ExportFile {
  /** What is written, before JSON. */
  body: unknown;
  /** Added to the workflow's name. */
  suffix: string;
}

/** Export format → the file's contents and its default name suffix. */
const FORMATS: Readonly<Record<string, (draft: Draft) => ExportFile>> = {
  oya: (draft) => ({ body: workflowOf(draft), suffix: '' }),
  chrome: (draft) => ({ body: toChromeRecording(draft), suffix: '.recording' }),
};

/** The save dialog for a workflow file, named after the workflow when its name is safe. */
function saveOptions(draft: Draft, suffix: string): SaveDialogOptions {
  const name = SAFE_FILE_NAME.test(String(draft.name)) ? String(draft.name) : 'workflow';
  return {
    title: 'Save workflow',
    defaultPath: `${name}${suffix}.json`,
    filters: [{ name: 'JSON', extensions: ['json'] }],
  };
}

/** The chosen file's JSON, refusing one too large to be a workflow. */
async function readWorkflowFile(file: string): Promise<unknown> {
  const { size } = await fs.promises.stat(file);
  if (size > MAX_IMPORT_BYTES) throw new Error('This file is too large to be a workflow.');
  try {
    return JSON.parse(await fs.promises.readFile(file, 'utf8'));
  } catch {
    throw new Error('This file is not a workflow: it is not valid JSON.');
  }
}

/** Saves the workflow to a file and opens one as a new draft. */
export class WorkflowFiles {
  /** The main-process services. */
  private readonly deps: Deps;

  /** `deps` is the main-process services (see src/main/main.ts). */
  constructor(deps: Deps) {
    this.deps = deps;
  }

  /** The workspace, which the workspace channel has checked exists. */
  private get studio(): NonNullable<Deps['workspace']> {
    const workspace = this.deps.workspace;
    if (!workspace) throw new Error('Workspace is starting');
    return workspace;
  }

  /** Saves the workflow as JSON where the person chooses; the snapshot says whether it was saved. */
  async exportJson(command: Payload): Promise<Payload> {
    const format =
      typeof command.format === 'string' && Object.hasOwn(FORMATS, command.format) ? command.format : 'oya';
    const draft = this.studio.draft;
    const { body, suffix } = FORMATS[format](draft);
    const result = await new ShellDialogs(this.deps).save(saveOptions(draft, suffix));
    if (!result.canceled) await writePrivateFile(result.filePath, JSON.stringify(body, null, JSON_INDENT));
    return { ...this.studio.snapshot(), exported: !result.canceled };
  }

  /** Opens a workflow file the person chooses, from Oya or Chrome's Recorder, as a new draft. */
  async importJson(edit: Edit): Promise<Payload> {
    const options = { properties: ['openFile' as const], filters: [{ name: 'JSON', extensions: ['json'] }] };
    const result = await new ShellDialogs(this.deps).open(options);
    if (result.canceled || !result.filePaths?.[0]) return this.studio.snapshot();
    const json = await readWorkflowFile(result.filePaths[0]);
    const draft = isChromeRecording(json) ? fromChromeRecording(json) : workflowOf((json as Draft) || {});
    return edit({ type: 'import', draft });
  }
}
