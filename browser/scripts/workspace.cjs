const { randomUUID } = require('node:crypto');
const { normalizeDraft, normalizeStep, generate, issues } = require('./workflow.cjs');
const { redact } = require('./diagnostics.cjs');
class Workspace {
  constructor({ store, runStore, notify, runner }) {
    this.store = store; this.runStore = runStore; this.notify = notify; this.runner = runner;
    this.draft = normalizeDraft(); this.history = []; this.future = []; this.run = null; this.storageError = null;
    const recent = store.list().find(d => !d.error);
    if (recent) this.draft = store.load(recent.id);
    for (const item of runStore?.list() || []) {
      if (item.error) continue;
      const record = runStore.load(item.id);
      if (['starting', 'running', 'paused', 'stopping'].includes(record.run?.status)) { record.run.status = 'interrupted'; record.run.error = 'Oya closed during validation. Check the website before retrying; nothing was automatically resubmitted.'; runStore.save(record); }
      if (!this.run && record.run?.draftId === this.draft.id) this.run = record.run;
    }
    this.pruneRuns();
  }
  support() { return redact({ schemaVersion: 1, run: this.run && { ...this.run, draft: undefined, code: undefined, repairs: this.run.repairs.map(r => ({ stepId: r.stepId, draftId: r.draftId })) } }); }
  snapshot() {
    let artifact = { code: '', mapping: {} }; const problems = issues(this.draft);
    try { if (!problems.length) artifact = generate(this.draft); } catch (e) { problems.push({ message: e.message }); }
    return { draft: this.draft, ...artifact, issues: problems, library: this.store.list(), storageError: this.storageError, canUndo: !!this.history.length, canRedo: !!this.future.length, runHistory: (this.runStore?.list() || []).filter(item => !item.error), run: this.run, support: this.support() };
  }
  publish() { const state = this.snapshot(); this.notify(state); return state; }
  persist() {
    this.draft.updatedAt = Date.now();
    try { this.store.save(this.draft); this.storageError = null; } catch (e) { this.storageError = e.message; }
    return this.publish();
  }
  pruneRuns() {
    let total = 0;
    for (const [index, item] of (this.runStore?.list() || []).entries()) {
      if (item.error) continue;
      const size = require('node:fs').statSync(this.runStore.file(item.id)).size;
      total += size;
      if (index >= 30 || Date.now() - item.updatedAt > 7 * 86400000 || total > 250 * 1024 * 1024) this.runStore.remove(item.id);
    }
  }
  saveRun() {
    if (!this.runStore || !this.run) return;
    try { this.runStore.save({ id: this.run.id, name: this.draft.name + ' · ' + this.run.status, run: this.run, updatedAt: Date.now() }); } catch (e) { this.storageError = e.message; }
  }
  busy() { return this.run && ['starting', 'running', 'paused', 'stopping'].includes(this.run.status); }
  edit(command) {
    if (this.busy()) throw new Error('Stop validation before editing its draft.');
    if (this.draft.phase === 'recording') throw new Error('Pause recording before editing steps.');
    if (command.type === 'undo' || command.type === 'redo') {
      const from = command.type === 'undo' ? this.history : this.future, to = command.type === 'undo' ? this.future : this.history;
      if (from.length) { to.push(this.draft); this.draft = from.pop(); } return this.persist();
    }
    const before = structuredClone(this.draft), draft = structuredClone(this.draft);
    const index = draft.steps.findIndex(s => s.id === command.id);
    switch (command.type) {
      case 'open-run': this.run = this.runStore.load(command.id).run; return this.publish();
      case 'new': this.draft = normalizeDraft(); this.history = []; this.future = []; this.run = null; return this.persist();
      case 'open': this.draft = this.store.load(command.id); this.history = []; this.future = []; this.run = null; return this.publish();
      case 'metadata': Object.assign(draft, { name: String(command.name ?? draft.name).slice(0, 64), description: String(command.description ?? draft.description).slice(0, 2000) }); break;
      case 'rename-variable': {
        if (!/^[A-Za-z_]\w{0,63}$/.test(command.nextName) || ['__proto__', 'constructor', 'prototype'].includes(command.nextName) || draft.variables[command.nextName]) throw new Error('Choose a unique variable name using letters, digits, and underscores');
        draft.variables[command.nextName] = draft.variables[command.name]; delete draft.variables[command.name];
        draft.secrets = draft.secrets.map(name => name === command.name ? command.nextName : name);
        draft.steps = JSON.parse(JSON.stringify(draft.steps).split('{{' + command.name + '}}').join('{{' + command.nextName + '}}')); break;
      }
      case 'variables': draft.variables = command.variables; draft.secrets = Object.keys(command.variables).filter(k => command.variables[k].secret); break;
      case 'add': draft.steps.splice(index < 0 ? draft.steps.length : index + 1, 0, normalizeStep(command.step)); break;
      case 'update': if (index < 0) throw new Error('Step no longer exists'); draft.steps[index] = normalizeStep({ ...draft.steps[index], ...command.patch, id: command.id }); break;
      case 'delete': if (index >= 0) draft.steps.splice(index, 1); break;
      case 'duplicate': if (index >= 0) draft.steps.splice(index + 1, 0, { ...structuredClone(draft.steps[index]), id: randomUUID() }); break;
      case 'move': if (index >= 0) { const [step] = draft.steps.splice(index, 1); draft.steps.splice(Math.max(0, Math.min(draft.steps.length, index + Number(command.delta))), 0, step); } break;
      default: throw new Error('Unknown editor command');
    }
    delete draft.publishedAt;
    this.draft = normalizeDraft({ ...draft, revision: draft.revision + 1 });
    this.history.push(before); if (this.history.length > 100) this.history.shift(); this.future = [];
    return this.persist();
  }
  capture(steps, secrets, recording) {
    this.draft = normalizeDraft({ ...this.draft, steps, secrets: [...secrets], phase: recording ? 'recording' : 'paused', revision: this.draft.revision + 1 });
    return this.persist();
  }
  async start(options) {
    if (this.busy()) throw new Error('A validation is already running');
    if (this.draft.phase === 'recording') throw new Error('Pause recording before validation');
    if (!this.draft.steps.some(s => s.enabled)) throw new Error('Add a step before validation');
    generate(this.draft);
    this.session = null; this.pendingControl = null;
    this.run = { id: randomUUID(), draftId: this.draft.id, revision: this.draft.revision, draft: structuredClone(this.draft), code: generate(this.draft).code, status: 'starting', startedAt: Date.now(), events: [], repairs: [] };
    this.saveRun(); this.pruneRuns(); this.publish();
    try { this.session = await this.runner(structuredClone(this.draft), options, message => this.receive(message)); if (this.pendingControl) { this.session.control(this.pendingControl); this.pendingControl = null; } if (this.run.status === 'starting') this.run.status = 'running'; }
    catch (e) { this.run.status = 'failed'; this.run.error = redact(e.message); this.saveRun(); }
    return this.publish();
  }
  receive(message) {
    if (!this.run) return;
    if (message.type === 'event') {
      this.run.events.push(message.event); if (this.run.events.length > 2000) this.run.events.shift();
      if (message.event.status === 'paused') this.run.status = 'paused';
      if (message.event.status === 'running') this.run.status = 'running';
    } else if (message.type === 'repair') {
      const repaired = normalizeDraft({ ...message.draft, id: randomUUID(), name: this.draft.name.slice(0, 45) + '-repair', repairedFrom: this.draft.id, phase: 'paused' });
      try { this.store.save(repaired); } catch (e) { this.storageError = e.message; this.run.events.push({ kind: 'attention', message: 'Repair could not be saved because secure storage is unavailable.', at: Date.now() }); }
      this.run.repairs.push({ stepId: message.stepId, original: message.original, replacement: message.replacement, draftId: repaired.id });
    } else if (message.type === 'finished') {
      Object.assign(this.run, message, { finishedAt: Date.now() });
    }
    if (message.type === 'finished') {
      clearTimeout(this.notifyTimer); clearTimeout(this.runSaveTimer); this.notifyTimer = this.runSaveTimer = null;
      this.saveRun(); this.publish();
    } else {
      if (!this.notifyTimer) this.notifyTimer = setTimeout(() => { this.notifyTimer = null; this.publish(); }, 100);
      if (!this.runSaveTimer) this.runSaveTimer = setTimeout(() => { this.runSaveTimer = null; this.saveRun(); }, 1000);
    }
  }
  control(command) {
    if (!this.busy()) throw new Error('No active validation');
    if (!['pause', 'resume', 'step', 'stop'].includes(command)) throw new Error('Unknown run control');
    if (this.session) this.session.control(command); else this.pendingControl = command;
    if (command === 'stop') this.run.status = 'stopping';
    return this.publish();
  }
}
module.exports = { Workspace };
