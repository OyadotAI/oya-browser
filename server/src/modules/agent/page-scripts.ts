/**
 * Scripts that act on native form controls from the page's main world, where the
 * analyzer's element ids do not reach: choosing a <select> option and attaching a file.
 */
import { sendCommand } from '../browsers/socket.ts';
import { UPLOAD_TIMEOUT_MS } from './constants.ts';

/** The select script after its interpolated handle and wanted option. */
const SELECT_OPTION_BODY = `
  const selects = [...document.querySelectorAll('select')];
  const byId = handle.domId && document.getElementById(handle.domId);
  const sel = (byId && byId.tagName === 'SELECT' && byId)
    || (handle.name && selects.find((s) => s.name === handle.name))
    || (handle.ariaLabel && selects.find((s) => s.getAttribute('aria-label') === handle.ariaLabel))
    || (handle.text && selects.find((s) => (s.labels && s.labels[0] ? s.labels[0].textContent.trim() : '') === handle.text));
  if (!sel) return { ok: false, error: 'dropdown not found on the page' };
  const opts = [...sel.options];
  const text = (o) => o.text.trim().toLowerCase();
  const only = (list) => (list.length === 1 ? list[0] : null);
  const pick = opts.find((o) => text(o) === want) || opts.find((o) => o.value.trim().toLowerCase() === want)
    || only(opts.filter((o) => want && text(o).startsWith(want))) || only(opts.filter((o) => want && text(o).includes(want)));
  if (!pick) return { ok: false, error: 'no single option matches', options: opts.map((o) => o.text.trim()).slice(0, 60) };
  sel.value = pick.value;
  sel.dispatchEvent(new Event('input', { bubbles: true }));
  sel.dispatchEvent(new Event('change', { bubbles: true }));
  return { ok: true, chosen: pick.text.trim() };
})()`;

/**
 * Choose a native <select> option by its text. Runs in the page's main world, where the
 * analyzer's element ids do not reach, so the select is found by the stable handles analyze
 * recorded. Every interpolated value is a JSON literal.
 */
const SELECT_OPTION_JS = (el, option) => `(() => {
  const handle = ${JSON.stringify({ domId: el.domId, name: el.name, ariaLabel: el.ariaLabel, text: el.text })};
  const want = ${JSON.stringify(String(option ?? ''))}.trim().toLowerCase();${SELECT_OPTION_BODY}`;

/**
 * Pick a <select> option by its text in the page's main world.
 * ponytail: top document only; selects inside iframes or shadow roots are not reached.
 */
export async function selectOptionIn(browserId, el, option) {
  const r = await sendCommand(browserId, 'evaluate_raw', { expression: SELECT_OPTION_JS(el || {}, option) });
  if (!r.ok) return { ok: false, error: r.error || 'select failed' };
  return r.data?.result ?? r.data ?? { ok: false, error: 'select failed' };
}

/** The upload script after its interpolated handle, file metadata and bytes. */
const UPLOAD_FILE_BODY = `
  const inputs = [...document.querySelectorAll('input[type="file"]')];
  if (!inputs.length) return { ok: false, error: 'this page has no file input' };

  const named = handle.domId || handle.name || handle.ariaLabel || handle.text;
  const pool = [...document.querySelectorAll('input,button,label,a,div,span,p,section,form,[role]')];
  const byId = handle.domId && document.getElementById(handle.domId);
  const anchor = byId
    || (handle.name && pool.find((e) => e.getAttribute('name') === handle.name))
    || (handle.ariaLabel && pool.find((e) => e.getAttribute('aria-label') === handle.ariaLabel))
    || (handle.text && pool.find((e) => (e.textContent || '').trim() === handle.text))
    || null;
  if (named && !anchor) return { ok: false, error: 'that element is no longer on the page; analyze again' };

  const isFileInput = (n) => n && n.tagName === 'INPUT' && n.type === 'file';
  const near = (node) => {
    if (!node) return null;
    if (isFileInput(node)) return node;
    if (node.htmlFor) { const t = document.getElementById(node.htmlFor); if (isFileInput(t)) return t; }
    const inside = node.querySelector && node.querySelector('input[type="file"]');
    if (inside) return inside;
    let up = node.parentElement;
    for (let i = 0; i < 4 && up; i++, up = up.parentElement) {
      const found = up.querySelector('input[type="file"]');
      if (found) return found;
    }
    return null;
  };

  const input = near(anchor) || (inputs.length === 1 ? inputs[0] : null);
  if (!input) {
    return { ok: false, error: anchor
      ? 'no file input belongs to that element'
      : 'this page has ' + inputs.length + ' file inputs; pass the element id of the upload button or field you mean' };
  }
  if (input.disabled) return { ok: false, error: 'that file input is disabled' };

  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  const dt = new DataTransfer();
  dt.items.add(new File([bytes], meta.file, { type: meta.type }));
  try { input.files = dt.files; } catch (e) { return { ok: false, error: 'the page would not take the file: ' + e.message }; }
  if (!input.files.length) return { ok: false, error: 'the page cleared the file straight away' };
  input.dispatchEvent(new Event('input', { bubbles: true }));
  input.dispatchEvent(new Event('change', { bubbles: true }));
  return { ok: true, field: input.name || input.id || (input.labels && input.labels[0] && input.labels[0].textContent.trim()) || 'the file input' };
})()`;

/**
 * Put a file into a page's file input. Main world, like select_option, because the
 * analyzer's element ids live in an isolated world and its tag attribute is randomised
 * per session — so the element is found by the stable handles analyze recorded.
 *
 * The input itself is usually not what the agent can see: upload widgets hide the real
 * `<input type=file>` behind a button or a drop zone, and a hard-hidden node never gets
 * an element id at all. So the handle names whatever was visible and the input is found
 * from there.
 *
 * ponytail: a 10MB file rides as a ~13.4MB Runtime.evaluate expression; chunk it only if
 * that measurably hurts. Top document only, so inputs inside iframes or shadow roots are
 * out of reach, and a drop zone with no file input behind it cannot be fed this way.
 */
export const UPLOAD_FILE_JS = (el, f) => `(() => {
  const handle = ${JSON.stringify({ domId: el.domId, name: el.name, ariaLabel: el.ariaLabel, text: el.text })};
  const meta = ${JSON.stringify({ file: f.file, type: f.type })};
  const b64 = ${JSON.stringify(String(f.b64 || ''))};${UPLOAD_FILE_BODY}`;

/** The upload twin of selectOptionIn. `el` is null when the agent named no element. */
export async function uploadFileIn(browserId, el, f) {
  const r = await sendCommand(
    browserId,
    'evaluate_raw',
    { expression: UPLOAD_FILE_JS(el || {}, f || {}) },
    UPLOAD_TIMEOUT_MS,
  );
  if (!r.ok) return { ok: false, error: r.error || 'upload failed' };
  return r.data?.result ?? r.data ?? { ok: false, error: 'upload failed' };
}
