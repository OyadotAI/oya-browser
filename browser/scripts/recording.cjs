const { randomBytes } = require('node:crypto');

/** Deliver steps before navigation destroys the document. The binding and all
 * recorder globals live only in our isolated world, never in the site's world. */
class RecordingChannel {
  constructor({ send, on, evaluate, worldName, analyzer, receive }) {
    Object.assign(this, { send, on, evaluate, worldName, analyzer, receive });
    this.binding = 'r' + randomBytes(12).toString('hex');
  }

  async start() {
    const sink = `window.__acRecordSink = (data) => window[${JSON.stringify(this.binding)}](JSON.stringify(data));`;
    this.off = this.on('Runtime.bindingCalled', (event) => {
      if (event.name !== this.binding) return;
      try { this.receive(JSON.parse(event.payload)); } catch {}
    });
    try {
      await this.send('Runtime.addBinding', { name: this.binding, executionContextName: this.worldName });
      // The analyzer needs a body. Arm every subsequent main document before
      // user interaction, without waiting for the next server status poll.
      const source = `(() => {
        if (window.top !== window) return;
        const arm = () => { ${sink}\n${this.analyzer}\nwindow.__acRecordStart(); };
        if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', arm, { once: true });
        else arm();
      })();`;
      const { identifier } = await this.send('Page.addScriptToEvaluateOnNewDocument', { source, worldName: this.worldName });
      this.script = identifier;
      await this.evaluate(`${sink}\nwindow.__acRecordStart()`);
    } catch (err) {
      await this.stop().catch(() => {});
      throw err;
    }
  }

  async stop() {
    try {
      // Keep the listener attached until the last field value is delivered.
      await this.evaluate('window.__acRecordStop?.(); window.__acRecordSink = undefined;');
    } finally {
      if (this.script) await this.send('Page.removeScriptToEvaluateOnNewDocument', { identifier: this.script }).catch(() => {});
      await this.send('Runtime.removeBinding', { name: this.binding }).catch(() => {});
      this.off?.();
      this.script = null;
    }
  }
}

module.exports = { RecordingChannel };
