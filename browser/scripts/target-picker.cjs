const { candidates } = require('./workflow.cjs');
async function pickTarget(view) {
  const dbg = view.webContents.debugger;
  if (!dbg.isAttached()) dbg.attach('1.3');
  await dbg.sendCommand('DOM.enable'); await dbg.sendCommand('Overlay.enable');
  return new Promise((resolve, reject) => {
    let settled = false;
    const finish = async (error, value) => {
      if (settled) return; settled = true; clearTimeout(timer); dbg.off('message', listener); view.webContents.off('destroyed', destroyed);
      await dbg.sendCommand('Overlay.setInspectMode', { mode: 'none' }).catch(() => {});
      if (error) reject(error); else resolve(value);
    };
    const destroyed = () => finish(new Error('The page closed while picking a target'));
    const listener = async (_event, method, params) => {
      if (method === 'Overlay.inspectModeCanceled') return finish(new Error('Target selection canceled'));
      if (method !== 'Overlay.inspectNodeRequested') return;
      try {
        const { object } = await dbg.sendCommand('DOM.resolveNode', { backendNodeId: params.backendNodeId });
        const { result, exceptionDetails } = await dbg.sendCommand('Runtime.callFunctionOn', { objectId: object.objectId, returnByValue: true, functionDeclaration: `function() {
          const n = this.closest('button,a,input,textarea,select,[role],[contenteditable]') || this;
          if (n.ownerDocument.defaultView !== n.ownerDocument.defaultView.top) return { unsupported: true };
          return { tag: n.tagName.toLowerCase(), type: n.tagName === 'INPUT' ? 'input' : n.tagName.toLowerCase(), text: (n.labels?.[0]?.textContent || (n.matches('input,textarea,[contenteditable]') ? '' : n.textContent) || '').trim().slice(0, 160), domId: n.id, name: n.getAttribute('name'), placeholder: n.getAttribute('placeholder'), ariaLabel: n.getAttribute('aria-label'), testId: n.getAttribute('data-testid'), role: n.getAttribute('role') || (n.tagName === 'BUTTON' ? 'button' : n.tagName === 'A' ? 'link' : undefined) };
        }` });
        await dbg.sendCommand('Runtime.releaseObject', { objectId: object.objectId });
        if (exceptionDetails || result.value?.unsupported) throw new Error('This picker supports top-level targets. Enter the frame selector for embedded targets.');
        const choices = candidates(result.value);
        if (!choices.length) throw new Error('This element has no stable target. Add a test ID or enter a CSS selector.');
        finish(null, choices);
      } catch (error) { finish(error); }
    };
    const timer = setTimeout(() => finish(new Error('Target selection timed out')), 60000);
    dbg.on('message', listener); view.webContents.once('destroyed', destroyed);
    dbg.sendCommand('Overlay.setInspectMode', { mode: 'searchForNode', highlightConfig: { showInfo: true, contentColor: { r: 70, g: 180, b: 160, a: 0.3 }, borderColor: { r: 70, g: 180, b: 160, a: 1 } } }).catch(error => finish(error));
  });
}
module.exports = { pickTarget };
