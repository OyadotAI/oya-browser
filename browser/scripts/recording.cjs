const { randomBytes } = require('node:crypto');

/** Capture in a dedicated isolated world. In Electron, createIsolatedWorld can
 * return a different context from the same-named new-document script's world;
 * keep the actual recorder contexts instead of using the analyzer's evaluator. */
class RecordingChannel {
  constructor({ send, on, worldName, analyzer, receive, disableRuntimeOnStop = false }) {
    Object.assign(this, { send, on, analyzer, receive, disableRuntimeOnStop });
    this.binding = 'r' + randomBytes(12).toString('hex');
    this.worldName = worldName + '-' + this.binding;
    this.contexts = new Set();
    this.contextFrames = new Map();
    this.delivery = Promise.resolve();
    this.listeners = [];
  }

  async evaluate(contextId, expression) {
    const result = await this.send('Runtime.evaluate', { contextId, expression, returnByValue: true, awaitPromise: true });
    if (result.exceptionDetails) throw new Error(result.exceptionDetails.exception?.description || 'Recorder evaluation failed');
    return result.result?.value;
  }

  async start() {
    const { frameTree } = await this.send('Page.getFrameTree');
    this.frameId = frameTree.frame.id;
    let acknowledge;
    const ready = new Promise(resolve => { acknowledge = resolve; });
    this.listeners.push(
      this.on('Runtime.executionContextCreated', ({ context }) => {
        if (context.name === this.worldName) { this.contexts.add(context.id); this.contextFrames.set(context.id, context.auxData?.frameId); }
      }),
      this.on('Runtime.executionContextDestroyed', ({ executionContextId }) => this.contexts.delete(executionContextId)),
      this.on('Runtime.executionContextsCleared', () => this.contexts.clear()),
      this.on('Runtime.bindingCalled', (event) => {
        if (event.name !== this.binding) return;
        let data;
        try { data = JSON.parse(event.payload); } catch { return; }
        if (data.ready === this.binding) acknowledge();
        else this.deliver(data, event.executionContextId);
      }),
    );
    try {
      // Electron 35 does not deliver bindingCalled until Runtime is enabled.
      // Only desktop recording owns this subscription; the CDP driver already
      // keeps Runtime enabled for the lifetime of its connection.
      await this.send('Runtime.enable');
      this.runtimeStarted = true;
      await this.send('Runtime.addBinding', { name: this.binding, executionContextName: this.worldName });
      const source = `(() => {
        window.__acRecordCancelled = false;
        window.__acRecordSink = data => window[${JSON.stringify(this.binding)}](JSON.stringify(data));
        const arm = () => {
          if (window.__acRecordCancelled) return;
          ${this.analyzer}
          window.__acRecordStart();
        };
        if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', arm, { once: true });
        else arm();
      })();`;
      const { identifier } = await this.send('Page.addScriptToEvaluateOnNewDocument', { source, worldName: this.worldName });
      this.script = identifier;
      const { executionContextId } = await this.send('Page.createIsolatedWorld', { frameId: this.frameId, worldName: this.worldName });
      this.contexts.add(executionContextId); this.contextFrames.set(executionContextId, this.frameId);
      await this.evaluate(executionContextId, source);
      const armChildren = async tree => {
        for (const child of tree.childFrames || []) {
          try {
            const world = await this.send('Page.createIsolatedWorld', { frameId: child.frame.id, worldName: this.worldName });
            this.contexts.add(world.executionContextId); this.contextFrames.set(world.executionContextId, child.frame.id);
            await this.evaluate(world.executionContextId, source);
          } catch {
            this.receive({ steps: [{ action: 'unsupported_frame', captureIssue: 'An embedded frame could not be recorded. Review this part of the workflow before validation.' }] });
          }
          await armChildren(child);
        }
      };
      await armChildren(frameTree);
      await this.evaluate(executionContextId, `window[${JSON.stringify(this.binding)}](${JSON.stringify(JSON.stringify({ ready: this.binding }))})`);
      let timeout;
      try {
        await Promise.race([ready, new Promise((_, reject) => {
          timeout = setTimeout(() => reject(new Error('Page recording did not connect. Restart this browser and try again.')), 3000);
        })]);
      } finally { clearTimeout(timeout); }
    } catch (err) {
      await this.stop().catch(() => {});
      throw err;
    }
  }

  async framePath(frameId) {
    if (!frameId || frameId === this.frameId) return [];
    const { frameTree } = await this.send('Page.getFrameTree');
    const find = (tree, chain = []) => tree.frame.id === frameId ? chain : (tree.childFrames || []).map(child => find(child, [...chain, child.frame.id])).find(Boolean);
    const chain = find(frameTree); if (!chain) throw new Error('Frame detached');
    const selectors = [];
    for (const id of chain) {
      const owner = await this.send('DOM.getFrameOwner', { frameId: id });
      const { node } = await this.send('DOM.describeNode', { backendNodeId: owner.backendNodeId });
      const attributes = {}; for (let i = 0; i < (node.attributes || []).length; i += 2) attributes[node.attributes[i]] = node.attributes[i + 1];
      const key = ['data-testid', 'id', 'name', 'src'].find(key => attributes[key]);
      if (!key) throw new Error('Frame has no stable selector');
      selectors.push(`${node.localName || 'iframe'}[${key}=${JSON.stringify(attributes[key])}]`);
    }
    return selectors;
  }
  deliver(data, contextId) {
    const frameId = this.contextFrames.get(contextId);
    this.delivery = this.delivery.then(async () => {
      if (!data.steps?.length) return this.receive(data);
      let frames = [], issue;
      try { frames = await this.framePath(frameId); } catch { issue = 'The frame target could not be identified. Set its frame selector before validation.'; }
      this.receive({ ...data, steps: data.steps.map(step => ({ ...step, frames, ...(issue ? { captureIssue: issue } : {}) })) });
    });
    return this.delivery;
  }
  async visit(expression) {
    for (const id of this.contexts) {
      try {
        const out = await this.evaluate(id, expression);
        if (out) await this.deliver(out, id);
      } catch (err) {
        // Navigation can destroy a context while a status/stop is in flight.
        if (!/context|Cannot find/i.test(err.message || '')) throw err;
        this.contexts.delete(id);
      }
    }
  }

  drain(final = false) { return this.visit(`window.__acRecordDrain?.(${!!final})`); }
  clear() { return this.visit('window.__acRecordClear?.(); undefined'); }

  async stop() {
    try {
      if (this.script) await this.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: this.script });
      await this.visit(`window.__acRecordCancelled = true;
        window.__acRecordStop?.();
        window.__acRecordSink = undefined;
        window.__acRecordDrain?.(true);`);
    } finally {
      await this.send('Runtime.removeBinding', { name: this.binding }).catch(() => {});
      for (const off of this.listeners) off();
      this.listeners = [];
      await this.delivery;
      this.contexts.clear(); this.contextFrames.clear();
      this.script = null;
      if (this.runtimeStarted && this.disableRuntimeOnStop) await this.send('Runtime.disable').catch(() => {});
      this.runtimeStarted = false;
    }
  }
}

module.exports = { RecordingChannel };
