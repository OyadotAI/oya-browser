/** IPC: recording, discarding, saving as a playbook, and exporting Playwright code. */
const { saveRecording } = require('../recording/publish.cjs');
const { writePrivateFile } = require('./files.cjs');
const { workspaceCommand } = require('./workspace.cjs');
const { MAX_EXPORT_CHARS, CONFIRM_BUTTON } = require('./constants.cjs');

/** The question asked before recorded steps are thrown away. */
const DISCARD_PROMPT = {
  type: 'question',
  title: 'Discard recording?',
  message: 'Discard these recorded steps?',
  detail: 'This cannot be undone. Save your playbook first if you want to keep it.',
  buttons: ['Keep recording', 'Discard'],
  defaultId: 0,
  cancelId: 0,
};

/** The save dialog for an export, named after the playbook when its name is safe. */
function playwrightSaveOptions(payload) {
  const name = typeof payload.name === 'string' && /^[\w-]{1,64}$/.test(payload.name) ? payload.name : 'playbook';
  const filters = [{ name: 'JavaScript', extensions: ['mjs'] }];
  return { title: 'Save Playwright script', defaultPath: name + '.mjs', filters };
}

/** Saves Playwright code the shell generated where the person chooses. */
async function exportPlaywright(ctx, _e, payload) {
  if (typeof payload?.code !== 'string' || payload.code.length > MAX_EXPORT_CHARS)
    throw new Error('Invalid Playwright export');
  const result = await ctx.electron.dialog.showSaveDialog(ctx.shell.window, playwrightSaveOptions(payload));
  if (result.canceled) return { canceled: true };
  await writePrivateFile(result.filePath, payload.code);
  return { saved: true };
}

/** Channel → handler. */
const RECORDING_HANDLERS = {
  workspace: workspaceCommand,
  'start-recording': (ctx) => {
    ctx.shield.requireHumanControl();
    return ctx.recorder.queueRecording((previous) => ctx.recorder.startRecording(previous));
  },
  'stop-recording': (ctx) => ctx.recorder.queueRecording(() => ctx.recorder.stopRecording()),
  'clear-recording': (ctx) => ctx.recorder.queueRecording(() => ctx.recorder.clear()),
  'save-recording': (ctx, _e, name, description) =>
    ctx.recorder.queueRecording(() => saveRecording(ctx, name, description)),
  'export-playwright': exportPlaywright,
  'confirm-discard-recording': async (ctx) => {
    const result = await ctx.electron.dialog.showMessageBox(ctx.shell.window, DISCARD_PROMPT);
    return result.response === CONFIRM_BUTTON;
  },
};

module.exports = { RECORDING_HANDLERS };
