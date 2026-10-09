/** Intentional isolated-world picker UI; only the visible highlight enters the page DOM. */
/** Install the local hover overlay and descriptor without changing any page-owned API. */
export const PICKER_START = `(() => {
  globalThis.__oyaPicker?.stop();
  const box = document.createElement('div');
  box.setAttribute('data-oya-picker', '');
  box.setAttribute('aria-hidden', 'true');
  box.style.cssText = 'position:fixed;pointer-events:none;z-index:2147483647;border:1px solid rgb(70,180,160);background:rgba(70,180,160,.3);display:none;box-sizing:border-box';
  document.documentElement.appendChild(box);
  const find = (x, y) => {
    let n = document.elementFromPoint(x, y);
    while (n?.shadowRoot) { const next = n.shadowRoot.elementFromPoint(x, y); if (!next || next === n) break; n = next; }
    return n?.closest('button,a,input,textarea,select,[role],[contenteditable]') || n;
  };
  globalThis.__oyaPicker = {
    hover(x, y) {
      const n = find(x, y); if (!n) { box.style.display = 'none'; return; }
      const r = n.getBoundingClientRect();
      Object.assign(box.style, {display:'block',left:r.left+'px',top:r.top+'px',width:r.width+'px',height:r.height+'px'});
    },
    pick(x, y) {
      const n = find(x, y); if (!n) return {};
      if (n.matches('iframe,frame')) return {unsupported:true};
      return { tag: n.tagName.toLowerCase(), type: n.tagName === 'INPUT' ? 'input' : n.tagName.toLowerCase(), text: (n.labels?.[0]?.textContent || (n.matches('input,textarea,[contenteditable]') ? '' : n.textContent) || '').trim().slice(0, 160), domId: n.id, name: n.getAttribute('name'), placeholder: n.getAttribute('placeholder'), ariaLabel: n.getAttribute('aria-label'), testId: n.getAttribute('data-testid'), role: n.getAttribute('role') || (n.tagName === 'BUTTON' ? 'button' : n.tagName === 'A' ? 'link' : undefined) };
    },
    stop() { box.remove(); delete globalThis.__oyaPicker; }
  };
})();`;
/** Cleanup is idempotent even when navigation replaced the original document. */
export const PICKER_STOP = 'globalThis.__oyaPicker?.stop();';
