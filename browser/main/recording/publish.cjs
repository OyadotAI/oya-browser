/**
 * Hand the recording to the server, which saves it as a playbook and returns
 * its Playwright code; the workspace marks the draft published.
 */
const { canCallServer, postToBrowserApi } = require('../connection/server-api.cjs');
const { PLAYBOOK_SCHEMA_VERSION } = require('./constants.cjs');
const { SAVE_TIMEOUT_MS } = require('../connection/constants.cjs');

/** The playbook the server is sent. Capture timestamps stay here. */
function playbookBody(ctx, name, description) {
  const variables = ctx.workspace?.draft.variables || {};
  return { schemaVersion: PLAYBOOK_SCHEMA_VERSION, variables, name, prompt: description, ...recorded(ctx.recorder) };
}

/** The recorded steps without their capture timestamps, and the secret names. */
function recorded({ recordedSteps, recordedSecrets }) {
  return { steps: recordedSteps.map(({ t, ...step }) => step), secrets: [...recordedSecrets] };
}

/** Marks the draft published, if it is still the revision that was sent. */
function markPublished(workspace, sent) {
  if (workspace?.draft.id !== sent.id || workspace.draft.revision !== sent.revision) return;
  Object.assign(workspace.draft, { publishedAt: Date.now(), publishedRevision: sent.revision });
  workspace.persist();
}

/** The server's answer; a success marks the draft published, and a refusal is always an error. */
async function readPlaybookAnswer(ctx, res, sent) {
  const body = await res.json().catch(() => ({}));
  if (!res.ok) return { error: body.error || `Server returned ${res.status}` };
  markPublished(ctx.workspace, sent);
  return body;
}

/** Posts the playbook; answers the server's body, or `{ error }`. */
async function postPlaybook(ctx, name, description) {
  const sent = { id: ctx.workspace?.draft.id, revision: ctx.workspace?.draft.revision };
  try {
    const res = await postToBrowserApi(ctx, 'playbooks', playbookBody(ctx, name, description), SAVE_TIMEOUT_MS);
    return await readPlaybookAnswer(ctx, res, sent);
  } catch (err) {
    if (err.name === 'TimeoutError') return { error: 'The server took too long to save. Try again.' };
    return { error: err.message };
  }
}

/** Stops any recording and publishes what was recorded. */
async function saveRecording(ctx, name, description) {
  if (!canCallServer(ctx)) return { error: 'Not connected to server' };
  if (ctx.recorder.recording) await ctx.recorder.stopRecording();
  if (!ctx.recorder.recordedSteps.length) return { error: 'Nothing recorded yet' };
  return postPlaybook(ctx, name, description);
}

module.exports = { saveRecording };
