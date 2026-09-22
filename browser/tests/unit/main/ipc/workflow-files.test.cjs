/**
 * Unit tests for main/ipc/workflow-files.cjs: a workflow saved as JSON (Oya's
 * own, or Chrome Recorder's) and opened again from either.
 */
const { describe, it, beforeEach, afterEach } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { exportJson, importJson } = require('../../../../main/ipc/workflow-files.cjs');
const { Workspace } = require('../../../../scripts/workspace.cjs');
const { MemoryStore } = require('../../support/stores.cjs');

/** A workspace holding one workflow, and a ctx whose dialogs answer `file`. */
function setup(file) {
  const workspace = new Workspace({ store: new MemoryStore({}), runStore: new MemoryStore({}), notify: () => {} });
  workspace.capture([{ id: 'a', action: 'navigate', url: 'https://x.test/' }], ['password'], false);
  workspace.edit({ type: 'metadata', name: 'login' });
  const dialog = {
    showSaveDialog: async (_w, options) => ((dialog.saveOptions = options), { canceled: !file, filePath: file }),
    showOpenDialog: async () => ({ canceled: !file, filePaths: [file] }),
  };
  return { workspace, electron: { dialog }, shell: {} };
}

/** How the IPC layer answers an edit: straight to the workspace. */
const edit = (ctx, command) => ctx.workspace.edit(command);

describe('workflow files', () => {
  let dir;
  beforeEach(() => (dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oya-wf-'))));
  afterEach(() => fs.rmSync(dir, { recursive: true, force: true }));

  it('saves the workflow itself, its steps and secret names, without its history', async () => {
    const file = path.join(dir, 'login.json');
    const ctx = setup(file);
    const answer = await exportJson(ctx, { format: 'oya' });
    const saved = JSON.parse(fs.readFileSync(file, 'utf8'));
    assert.equal(answer.exported, true);
    assert.equal(ctx.electron.dialog.saveOptions.defaultPath, 'login.json');
    assert.deepEqual(Object.keys(saved).sort(), [
      'description',
      'name',
      'schemaVersion',
      'secrets',
      'steps',
      'variables',
    ]);
    assert.deepEqual(saved.secrets, ['password']);
  });

  it('saves the workflow in Chrome Recorder’s shape when asked', async () => {
    const file = path.join(dir, 'login.recording.json');
    await exportJson(setup(file), { format: 'chrome' });
    const saved = JSON.parse(fs.readFileSync(file, 'utf8'));
    assert.deepEqual(saved, { title: 'login', steps: [{ type: 'navigate', url: 'https://x.test/' }] });
  });

  it('opens a Chrome recording as a new draft', async () => {
    const file = path.join(dir, 'chrome.json');
    fs.writeFileSync(
      file,
      JSON.stringify({ title: 'From Chrome', steps: [{ type: 'navigate', url: 'https://y.test/' }] }),
    );
    const ctx = setup(file);
    const before = ctx.workspace.draft.id;
    const answer = await importJson(ctx, {}, edit);
    assert.notEqual(answer.draft.id, before);
    assert.deepEqual([answer.draft.name, answer.draft.steps[0].url], ['From Chrome', 'https://y.test/']);
  });

  it('opens an Oya workflow file as a new draft of its own', async () => {
    const file = path.join(dir, 'login.json');
    const saving = setup(file);
    await exportJson(saving, { format: 'oya' });
    const answer = await importJson(setup(file), {}, edit);
    assert.deepEqual([answer.draft.name, answer.draft.steps.length, answer.draft.publishedAt], ['login', 1, undefined]);
  });

  it('refuses a file that is not JSON, and leaves the draft alone', async () => {
    const file = path.join(dir, 'bad.json');
    fs.writeFileSync(file, 'not json');
    const ctx = setup(file);
    await assert.rejects(importJson(ctx, {}, edit), /not valid JSON/);
    assert.equal(ctx.workspace.draft.name, 'login');
  });

  it('changes nothing when the person cancels', async () => {
    const ctx = setup(null);
    assert.equal((await exportJson(ctx, { format: 'oya' })).exported, false);
    assert.equal((await importJson(ctx, {}, edit)).draft.name, 'login');
  });
});
