/**
 * Target picker: lets a person click an element in the page (Chrome's inspect
 * overlay) and turns it into locator candidates for a workflow step.
 */
const { candidates } = require('./workflow.cjs');
const { PICKER } = require('./constants.cjs');

/**
 * Runs on the picked node in the page: climbs to the nearest interactive
 * element and describes it. Its text is what the page runs, so it keeps its
 * exact shape.
 */
const DESCRIBE_ELEMENT = `function() {
          const n = this.closest('button,a,input,textarea,select,[role],[contenteditable]') || this;
          if (n.ownerDocument.defaultView !== n.ownerDocument.defaultView.top) return { unsupported: true };
          return { tag: n.tagName.toLowerCase(), type: n.tagName === 'INPUT' ? 'input' : n.tagName.toLowerCase(), text: (n.labels?.[0]?.textContent || (n.matches('input,textarea,[contenteditable]') ? '' : n.textContent) || '').trim().slice(0, 160), domId: n.id, name: n.getAttribute('name'), placeholder: n.getAttribute('placeholder'), ariaLabel: n.getAttribute('aria-label'), testId: n.getAttribute('data-testid'), role: n.getAttribute('role') || (n.tagName === 'BUTTON' ? 'button' : n.tagName === 'A' ? 'link' : undefined) };
        }`;

/** The inspect overlay's highlight: the brand teal, translucent inside, solid border. */
const HIGHLIGHT = {
  showInfo: true,
  contentColor: { ...PICKER.HIGHLIGHT, a: PICKER.CONTENT_ALPHA },
  borderColor: { ...PICKER.HIGHLIGHT, a: 1 },
};

/** Describes the picked node in the page, releasing the remote object afterwards. */
async function describePicked(dbg, backendNodeId) {
  const { object } = await dbg.sendCommand('DOM.resolveNode', { backendNodeId });
  const call = { objectId: object.objectId, returnByValue: true, functionDeclaration: DESCRIBE_ELEMENT };
  const { result, exceptionDetails } = await dbg.sendCommand('Runtime.callFunctionOn', call);
  await dbg.sendCommand('Runtime.releaseObject', { objectId: object.objectId });
  if (exceptionDetails || result.value?.unsupported) {
    throw new Error('This picker supports top-level targets. Enter the frame selector for embedded targets.');
  }
  return result.value;
}

/** Locator candidates for a described element; an element with none is refused. */
function choicesFor(element) {
  const choices = candidates(element);
  if (!choices.length) throw new Error('This element has no stable target. Add a test ID or enter a CSS selector.');
  return choices;
}

/** One pick in progress: settles once, on a choice, a cancel, a closed page or the timeout. */
class TargetPick {
  /** Picks on `view` through its debugger `dbg`, settling through `resolve`/`reject`. */
  constructor(dbg, view, resolve, reject) {
    Object.assign(this, { dbg, view, resolve, reject, settled: false });
    this.listener = (_event, method, params) => this.onMessage(method, params);
    this.destroyed = () => this.finish(new Error('The page closed while picking a target'));
  }

  /** Starts listening and turns on the inspect overlay. */
  start() {
    this.timer = setTimeout(() => this.finish(new Error('Target selection timed out')), PICKER.TIMEOUT_MS);
    this.dbg.on('message', this.listener);
    this.view.webContents.once('destroyed', this.destroyed);
    const inspect = { mode: 'searchForNode', highlightConfig: HIGHLIGHT };
    this.dbg.sendCommand('Overlay.setInspectMode', inspect).catch((error) => this.finish(error));
  }

  /** Settles the pick once: stops listening, turns the overlay off, then answers. */
  async finish(error, value) {
    if (this.settled) return;
    this.settled = true;
    clearTimeout(this.timer);
    this.dbg.off('message', this.listener);
    this.view.webContents.off('destroyed', this.destroyed);
    await this.dbg.sendCommand('Overlay.setInspectMode', { mode: 'none' }).catch(() => {});
    if (error) this.reject(error);
    else this.resolve(value);
  }

  /** A debugger event: a cancel ends the pick, a picked node is described and answered. */
  async onMessage(method, params) {
    if (method === 'Overlay.inspectModeCanceled') return this.finish(new Error('Target selection canceled'));
    if (method !== 'Overlay.inspectNodeRequested') return;
    try {
      this.finish(null, choicesFor(await describePicked(this.dbg, params.backendNodeId)));
    } catch (error) {
      this.finish(error);
    }
  }
}

/** Lets the person pick an element on `view`'s page; resolves to its locator candidates. */
async function pickTarget(view) {
  const dbg = view.webContents.debugger;
  if (!dbg.isAttached()) dbg.attach('1.3');
  await dbg.sendCommand('DOM.enable');
  await dbg.sendCommand('Overlay.enable');
  return new Promise((resolve, reject) => new TargetPick(dbg, view, resolve, reject).start());
}

module.exports = { pickTarget };
