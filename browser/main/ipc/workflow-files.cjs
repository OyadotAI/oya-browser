/**
 * IPC: a workflow as a JSON file. Saved as Oya's own draft (to open here again)
 * or as a Chrome DevTools Recorder recording; opened from either. What is saved
 * comes from the workspace, never from the shell, and a draft holds secret
 * names only, never their values.
 */
const fs = require('fs');
const { JSON_INDENT } = require('../app/constants.cjs');
const { writePrivateFile } = require('./files.cjs');
const { MAX_IMPORT_BYTES } = require('./constants.cjs');
const { isChromeRecording, fromChromeRecording, toChromeRecording } = require('../../scripts/workflow.cjs');

/** The fields of a draft that make up the workflow itself, without its history or runs. */
const WORKFLOW_FIELDS = ['schemaVersion', 'name', 'description', 'steps', 'variables', 'secrets'];

/** A draft reduced to the workflow. */
const workflowOf = (draft) => Object.fromEntries(WORKFLOW_FIELDS.map((key) => [key, draft[key]]));

/** Export format → the file's contents and its default name suffix. */
const FORMATS = {
  oya: (draft) => ({ body: workflowOf(draft), suffix: '' }),
  chrome: (draft) => ({ body: toChromeRecording(draft), suffix: '.recording' }),
};

/** The save dialog for a workflow file, named after the workflow when its name is safe. */
function saveOptions(draft, suffix) {
  const name = /^[\w-]{1,64}$/.test(draft.name) ? draft.name : 'workflow';
  return {
    title: 'Save workflow',
    defaultPath: `${name}${suffix}.json`,
    filters: [{ name: 'JSON', extensions: ['json'] }],
  };
}

/** Saves the workflow as JSON where the person chooses; the snapshot says whether it was saved. */
async function exportJson(ctx, command) {
  const format = Object.hasOwn(FORMATS, command.format) ? command.format : 'oya';
  const draft = ctx.workspace.draft;
  const { body, suffix } = FORMATS[format](draft);
  const result = await ctx.electron.dialog.showSaveDialog(ctx.shell.window, saveOptions(draft, suffix));
  if (!result.canceled) await writePrivateFile(result.filePath, JSON.stringify(body, null, JSON_INDENT));
  return { ...ctx.workspace.snapshot(), exported: !result.canceled };
}

/** The chosen file's JSON, refusing one too large to be a workflow. */
async function readWorkflowFile(file) {
  const { size } = await fs.promises.stat(file);
  if (size > MAX_IMPORT_BYTES) throw new Error('This file is too large to be a workflow.');
  try {
    return JSON.parse(await fs.promises.readFile(file, 'utf8'));
  } catch {
    throw new Error('This file is not a workflow: it is not valid JSON.');
  }
}

/** Opens a workflow file the person chooses, from Oya or Chrome's Recorder, as a new draft. */
async function importJson(ctx, _command, edit) {
  const options = { properties: ['openFile'], filters: [{ name: 'JSON', extensions: ['json'] }] };
  const result = await ctx.electron.dialog.showOpenDialog(ctx.shell.window, options);
  if (result.canceled || !result.filePaths?.[0]) return ctx.workspace.snapshot();
  const json = await readWorkflowFile(result.filePaths[0]);
  const draft = isChromeRecording(json) ? fromChromeRecording(json) : workflowOf(json || {});
  return edit(ctx, { type: 'import', draft });
}

module.exports = { exportJson, importJson };
