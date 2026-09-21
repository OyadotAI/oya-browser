/** IPC: recording, saving as a playbook, and exporting Playwright code. */
const { saveRecording } = require('../recording/publish.cjs');
const { writePrivateFile } = require('./files.cjs');
const { workspaceCommand } = require('./workspace.cjs');
const { MAX_EXPORT_CHARS } = require('./constants.cjs');

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
    return ctx.recorder.queueRecording(() => ctx.recorder.startRecording());
  },
  'stop-recording': (ctx) => ctx.recorder.queueRecording(() => ctx.recorder.stopRecording()),
  'save-recording': (ctx, _e, name, description) =>
    ctx.recorder.queueRecording(() => saveRecording(ctx, name, description)),
  'export-playwright': exportPlaywright,
};

module.exports = { RECORDING_HANDLERS };
