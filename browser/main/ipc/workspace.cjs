/**
 * IPC: the workflow workspace (drafts, editing, target picking, validation,
 * diagnostics). Command type → handler; any other type is an edit.
 */
const { JSON_INDENT } = require('../app/constants.cjs');
const { writePrivateFileSync } = require('./files.cjs');
const { CONFIRM_BUTTON } = require('./constants.cjs');

/** The question asked before a workflow runs against the real site. */
const VALIDATE_PROMPT = {
  type: 'question',
  title: 'Validate workflow',
  message: 'Run on the real website?',
  detail:
    'Oya opens a fresh tab using your current login. This can submit forms, send messages, upload files, or change data. Steps run exactly as shown in the exported Playwright module.',
  buttons: ['Cancel', 'Run workflow'],
  defaultId: 0,
  cancelId: 0,
};

/** Where the diagnostics report is offered to be saved. */
const SUPPORT_SAVE_OPTIONS = {
  defaultPath: 'oya-diagnostics.json',
  filters: [{ name: 'JSON diagnostics', extensions: ['json'] }],
};

/** The page to pick on, once nothing else is using it. */
function pickableView(ctx) {
  ctx.shield.requireHumanControl();
  if (ctx.workspace.busy() || ctx.recorder.recording) {
    throw new Error('Pause recording and stop validation before picking a target');
  }
  const view = ctx.tabs.getActiveView();
  if (!view) throw new Error('Open a page first');
  return view;
}

/** Picks a target on the page for one step. */
async function pickTarget(ctx, command) {
  const view = pickableView(ctx);
  const workspace = ctx.workspace;
  const draftId = workspace.draft.id;
  const candidates = await require('../../scripts/target-picker.cjs').pickTarget(view);
  if (workspace.draft.id !== draftId) throw new Error('Draft changed during target selection');
  return applyTarget(ctx, command, candidates);
}

/** Saves the picked candidates on the step. */
function applyTarget(ctx, command, candidates) {
  const workspace = ctx.workspace;
  const state = workspace.edit({ type: 'update', id: command.id, patch: { candidates, captureIssue: undefined } });
  ctx.recorder.recordedSteps = structuredClone(workspace.draft.steps);
  return state;
}

/** Runs the draft against the real site, once the person confirms. */
async function validate(ctx, command) {
  const answer = await ctx.electron.dialog.showMessageBox(ctx.shell.window, VALIDATE_PROMPT);
  if (answer.response !== CONFIRM_BUTTON) return ctx.workspace.snapshot();
  return ctx.workspace.start(command);
}

/** Resumes, steps or stops a validation; resuming hands control back to the run. */
async function controlRun(ctx, command) {
  const state = ctx.control.snapshot();
  const resuming = ['resume', 'step'].includes(command.command);
  if (resuming && state.mode === 'human' && state.mine) await ctx.control.change('return');
  return ctx.workspace.control(command.command);
}

/** Saves a diagnostics report where the person chooses; the snapshot says whether it was saved. */
async function saveSupportReport(ctx) {
  const report = ctx.workspace.support();
  const result = await ctx.electron.dialog.showSaveDialog(ctx.shell.window, SUPPORT_SAVE_OPTIONS);
  const supportSaved = !result.canceled && !!result.filePath;
  if (supportSaved) writePrivateFileSync(result.filePath, JSON.stringify(report, null, JSON_INDENT));
  return { ...ctx.workspace.snapshot(), supportSaved };
}

/** An edit to the draft; the recording follows it. */
function editDraft(ctx, command) {
  const workspace = ctx.workspace;
  const state = workspace.edit(command);
  ctx.recorder.adopt(structuredClone(workspace.draft.steps), workspace.draft.secrets);
  return state;
}

/** Command type → handler. */
const WORKSPACE_COMMANDS = {
  get: (ctx) => ctx.workspace.snapshot(),
  'resume-recording': async (ctx) => {
    ctx.shield.requireHumanControl();
    await ctx.recorder.queueRecording(() => ctx.recorder.startRecording(true));
    return ctx.workspace.snapshot();
  },
  pick: pickTarget,
  validate,
  control: controlRun,
  support: saveSupportReport,
};

/** The `workspace` channel. */
async function workspaceCommand(ctx, _event, command = {}) {
  if (!ctx.workspace) throw new Error('Workspace is starting');
  if (Object.hasOwn(WORKSPACE_COMMANDS, command.type)) return WORKSPACE_COMMANDS[command.type](ctx, command);
  return editDraft(ctx, command);
}

module.exports = { workspaceCommand, WORKSPACE_COMMANDS };
