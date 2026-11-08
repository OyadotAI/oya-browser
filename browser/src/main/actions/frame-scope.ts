/**
 * Cross-origin iframes in the agent's page read and in its actions. The analyzer reads a
 * same-origin iframe through its document, but another site's iframe (the Azure portal's
 * blades, embedded consoles) is a separate document in another process: the read showed
 * one "[iframe: src]" row with nothing inside, and the agent could neither read nor click
 * anything there. Each such frame is now read in its own isolated world (the tab preload
 * reaches every frame, so the page never sees it), its elements get ids from a range of
 * their own, and an action on one of those ids runs in the frame that owns it.
 */
import type { WebFrameMain } from 'electron';
import type { NativePage as PageView } from '../native/index.ts';
import type { Analysis, Block, PageElement } from '../../page/render/types.ts';
import type { Mouse } from '../input/mouse.ts';
import type { Keyboard } from '../input/keyboard.ts';
import type { PageDriver } from './driver.ts';
import type { CommandParams, ScriptResult } from './types.ts';
import {
  FRAME_ID_STRIDE,
  FRAME_GONE_ERROR,
  FRAME_OUTSIDE_ERROR,
  FRAME_UNSUPPORTED_ERROR,
  COVERED_TARGET_ERROR,
} from './constants.ts';

/** One view's frames by slot, so an id keeps meaning the same frame between reads. */
interface Slots {
  /** The frame each slot names. */
  bySlot: Map<number, WebFrameMain>;
  /** The slot each frame has. */
  byFrame: WeakMap<WebFrameMain, number>;
  /** The next free slot; 0 is the page itself. */
  next: number;
  /** The address each slot's frame had when it was last read: a frame that moved on has other elements. */
  readAt: Map<number, string>;
}

/** Each view's slots; a closed view takes its slots with it. */
const views = new WeakMap<PageView, Slots>();

/** The view's slots, made on first use. */
function slotsOf(view: PageView): Slots {
  let slots = views.get(view);
  if (!slots) views.set(view, (slots = { bySlot: new Map(), byFrame: new WeakMap(), next: 1, readAt: new Map() }));
  return slots;
}

/** The frame's slot in its view, the same one every read. */
export function slotOf(view: PageView, frame: WebFrameMain): number {
  const slots = slotsOf(view);
  let slot = slots.byFrame.get(frame);
  if (slot === undefined) {
    slot = slots.next++;
    slots.byFrame.set(frame, slot);
    slots.bySlot.set(slot, frame);
  }
  return slot;
}

/** A URL's origin, or '' for one that does not parse. */
const originOf = (url: string): string => URL.parse(url)?.origin ?? '';

/**
 * CAPTCHA challenge frames, never read: a read marks elements, and a challenge watches for
 * exactly that and for clicks no person made. The CAPTCHA tools handle these frames.
 */
const CHALLENGE =
  /^https:\/\/([^/]+\.)?(recaptcha\.net|hcaptcha\.com|challenges\.cloudflare\.com)\/|^https:\/\/(www\.)?google\.com\/recaptcha\//;

/** The page's direct child frames on another web origin than the page; same-origin ones the analyzer reads itself. */
export function crossOriginChildren(view: PageView): WebFrameMain[] {
  const main = view.webContents?.mainFrame;
  if (!main?.frames) return []; // a view being torn down, or one with no frame tree
  const page = originOf(main.url);
  return main.frames.filter(
    (f) => !f.detached && /^https?:/.test(f.url) && originOf(f.url) !== page && !CHALLENGE.test(f.url),
  );
}

/** What an iframe row resolves to: its one live frame, a reason none can be chosen, or null when it is not a cross-origin frame. */
type Match = WebFrameMain | string | null;

/**
 * The live frame an iframe row shows. Stock Electron cannot ask the renderer which frame
 * an <iframe> element owns (only Oya's engine can), so a row is matched only when it is
 * the one row and the one frame from its origin: anything less certain stays a row with
 * a reason, and its frame is not read, so an action can never land in the wrong frame.
 */
function matchOf(row: Block, rows: Block[], frames: WebFrameMain[]): Match {
  const origin = originOf(String(row.target ?? ''));
  const live = frames.filter((f) => originOf(f.url) === origin);
  if (!live.length) return null;
  const shown = rows.filter((r) => originOf(String(r.target ?? '')) === origin);
  return live.length === 1 && shown.length === 1 ? live[0] : 'several frames from this site, not read';
}

/** A read item that may carry an element id. */
interface WithId {
  /** Its element id, in its own frame's numbering. */
  id?: unknown;
}

/** Whether an id stays inside one frame's range: one past the stride would land in the next frame's. */
const fits = (item: WithId) => item.id === undefined || Number(item.id) < FRAME_ID_STRIDE;

/** The item with its id moved into the range starting at `offset`. */
const moved = <T extends WithId>(item: T, offset: number): T =>
  ({ ...item, id: item.id === undefined ? undefined : offset + Number(item.id) }) as T;

/** The analysis with each frame's ids moved into its slot's range, rows marked as inside an iframe, overflowing ids left out. */
function shifted(child: Analysis, offset: number): Required<Pick<Analysis, 'blocks' | 'elements'>> {
  const blocks = (child.blocks ?? []).filter(fits).map((b) => ({ ...moved(b, offset), region: 'iframe' }));
  const elements = (child.elements ?? []).filter(fits).map((e) => moved(e as PageElement, offset));
  return { blocks, elements };
}

/**
 * Puts the frame's rows right after its iframe row, and its elements after the page's. The
 * row stays as the frame's heading, so the agent can tell another site's text from the page's.
 */
function splice(data: Analysis, row: Block, child: ReturnType<typeof shifted>): void {
  const blocks = data.blocks ?? [];
  blocks.splice(blocks.indexOf(row) + 1, 0, ...child.blocks);
  data.elements = [...(data.elements ?? []), ...child.elements];
}

/** A matched frame's own read, or why there is none. */
async function readOf(driver: PageDriver, view: PageView, match: Match, script: string) {
  if (!match || typeof match === 'string') return match;
  const read = await driver.deps.worldEval<ScriptResult<Analysis>>(view, script, match).catch((e: Error) => e);
  if (read instanceof Error) return `not readable: ${read.message}`;
  return read?.data?.blocks ? { frame: match, data: read.data } : 'not readable: no answer';
}

/** Puts one frame's read under its row, or says on the row why it could not be read. */
function mergeOne(view: PageView, data: Analysis, row: Block, read: Awaited<ReturnType<typeof readOf>>): void {
  if (read === null) return;
  if (typeof read === 'string') return void (row.target = `${row.target} (${read})`);
  const slot = slotOf(view, read.frame as WebFrameMain);
  slotsOf(view).readAt.set(slot, (read.frame as WebFrameMain).url);
  const child = shifted(read.data, slot * FRAME_ID_STRIDE);
  // A frame row is on screen only where its frame is: the frame judged it by its own viewport.
  for (const b of child.blocks) b.screen = !!(b.screen && row.screen);
  splice(data, row, child);
}

/**
 * Numbers the merged rows in reading order and counts them and the elements, as the analyzer does for its own:
 * cutting a long page keeps rows by those numbers, and frame rows keeping their frame's
 * numbers sat among the page's and were kept or dropped as the wrong rows.
 */
function renumber(data: Analysis, before: number): void {
  const blocks = data.blocks ?? [];
  blocks.forEach((b, i) => (b.index = i));
  data.totalBlocks = (data.totalBlocks ?? before) + blocks.length - before;
  const elements = data.elements ?? [];
  if (data.facts) data.facts.elements = `${elements.length} total, ${elements.filter((e) => e.visible).length} visible`;
}

/**
 * Reads every iframe row's frame at once, not in turn, so a silent frame waits out its
 * deadline without holding the others, then merges each read under its row.
 */
async function readAll(driver: PageDriver, view: PageView, data: Analysis, rows: Block[], ctx: ReadContext) {
  const reads = await Promise.all(rows.map((row) => readOf(driver, view, matchOf(row, rows, ctx.frames), ctx.script)));
  rows.forEach((row, i) => mergeOne(view, data, row, reads[i]));
}

/** What a frame read needs besides the rows: the live frames to match and the analyze script. */
interface ReadContext {
  /** The page's cross-origin child frames. */
  frames: WebFrameMain[];
  /** The analyze script. */
  script: string;
}

/** The page's read with every cross-origin iframe it can match read in place; `script` is the analyze script. */
export async function withFrames(driver: PageDriver, view: PageView, raw: unknown, script: string): Promise<unknown> {
  const data = (raw as ScriptResult<Analysis> | null)?.data;
  const frames = crossOriginChildren(view);
  if (!data?.blocks || !frames.length) return raw;
  const rows = data.blocks.filter((b) => b.kind === 'iframe');
  const before = data.blocks.length;
  await readAll(driver, view, data, rows, { frames, script });
  if (data.blocks.length !== before) renumber(data, before);
  return raw;
}

/** Commands that name no element and type into whatever has focus, which may be inside a frame. */
const KEY_COMMANDS = new Set(['press_key', 'keyboard_type']);

/**
 * The commands that may name an element inside a frame: each one's input the frame driver
 * runs inside the frame. Any other (a double-click, a drag) would send native input to the
 * page at the frame's own coordinates, where a hostile frame chooses what is underneath.
 */
export const FRAME_COMMANDS = new Set(['click', 'type', 'hover', 'select', ...KEY_COMMANDS]);

/** Why `action` cannot run on `target`, with its code; null when it can. */
export function frameRefusal(target: FrameTarget | string, action: string): [string, string] | null {
  if (typeof target === 'string') return [target, 'element_not_found'];
  return FRAME_COMMANDS.has(action) ? null : [FRAME_UNSUPPORTED_ERROR, 'action_unsupported'];
}

/** The element-id selector the server sends (controls.ts byId). */
const ID_SELECTOR = /^\[data-ac-id="(\d+)"\]$/;

/** A subframe element an action names: its frame and its selector inside that frame. */
export interface FrameTarget {
  /** The frame that owns the element. */
  frame: WebFrameMain;
  /** The element's selector in that frame's own numbering. */
  selector: string;
}

/** The frame an action's element lives in; null for the page's own elements, an error message for a frame that is gone. */
export function frameTarget(view: PageView, selector: unknown): FrameTarget | string | null {
  const id = typeof selector === 'string' ? Number(ID_SELECTOR.exec(selector)?.[1]) : NaN;
  if (!(id >= FRAME_ID_STRIDE)) return null;
  const slot = Math.floor(id / FRAME_ID_STRIDE);
  const frame = slotsOf(view).bySlot.get(slot);
  // The same frame object survives a navigation inside it: its address says whether it still shows what was read.
  if (!frame || frame.detached || frame.url !== slotsOf(view).readAt.get(slot)) return FRAME_GONE_ERROR;
  return { frame, selector: `[data-ac-id="${id % FRAME_ID_STRIDE}"]` };
}

/** In the page, the origin of the <iframe> that has focus (through open shadow roots), or ''. */
const FOCUSED_ORIGIN_JS = `(() => {
  let at = document.activeElement;
  while (at && at.shadowRoot && at.shadowRoot.activeElement) at = at.shadowRoot.activeElement;
  if (!at || !at.matches('iframe,frame')) return '';
  try { return new URL(at.src, location.href).origin; } catch { return ''; }
})()`;

/** The cross-origin frame that has focus, when exactly one live frame comes from its origin. */
async function focusedFrame(driver: PageDriver, view: PageView): Promise<WebFrameMain | null> {
  const origin = await driver.deps.worldEval<string>(view, FOCUSED_ORIGIN_JS).catch(() => '');
  const live = origin ? crossOriginChildren(view).filter((f) => originOf(f.url) === origin) : [];
  return live.length === 1 ? live[0] : null;
}

/**
 * Where a command runs: the frame its element id names, or for a key command with no element
 * the frame that has focus (typing in a frame field and then pressing Enter would otherwise
 * send the key to the page, which never reaches the frame, and report success); null for the page.
 */
export async function targetOf(driver: PageDriver, view: PageView, action: string, params?: CommandParams) {
  const named = frameTarget(view, params?.selector);
  if (named || params?.selector || !KEY_COMMANDS.has(action)) return named;
  const frame = await focusedFrame(driver, view);
  return frame ? { frame, selector: '' } : null;
}

/** Where a subframe's content box sits in the page's viewport, and its size. */
interface Box {
  /** Whether the frame's <iframe> was found, once, in the page. */
  ok: boolean;
  /** Left edge of its content, in page viewport pixels. */
  x: number;
  /** Top edge of its content. */
  y: number;
  /** Content width. */
  w: number;
  /** Content height. */
  h: number;
}

/** In the page, the one <iframe> from `origin` (light and open shadow DOM), scrolled into view. */
const IFRAME_OF = (origin: string) => `const O = ${JSON.stringify(origin)};
  const roots = [document], found = [];
  for (let i = 0; i < roots.length; i++) for (const n of roots[i].querySelectorAll('*')) {
    if (n.shadowRoot) roots.push(n.shadowRoot);
    if (n.matches('iframe,frame')) { try { if (new URL(n.src, location.href).origin === O) found.push(n); } catch {} }
  }
  const f = found.length === 1 ? found[0] : null;`;

/** The frame's content box, read in the page after scrolling its <iframe> into view. */
const BOX_JS = (origin: string) => `(() => { ${IFRAME_OF(origin)}
  if (!f) return { ok: false };
  f.scrollIntoView({ block: 'nearest', inline: 'nearest', behavior: 'instant' });
  const r = f.getBoundingClientRect(), s = getComputedStyle(f);
  return { ok: true, x: r.left + f.clientLeft + parseFloat(s.paddingLeft), y: r.top + f.clientTop + parseFloat(s.paddingTop), w: f.clientWidth, h: f.clientHeight };
})()`;

/** Whether the page point (x, y) lands on the frame's <iframe> itself, not an overlay drawn over it. */
const HITS_JS = (origin: string, x: number, y: number) => `(() => { ${IFRAME_OF(origin)}
  let at = document.elementFromPoint(${x}, ${y});
  while (at && at.shadowRoot && at !== f) { const inner = at.shadowRoot.elementFromPoint(${x}, ${y}); if (!inner || inner === at) break; at = inner; }
  return !!f && at === f;
})()`;

/** A frame point checked against the frame's box, so a frame cannot steer input outside itself; answers it in page coordinates. */
function toPage(box: Box, x: unknown, y: unknown): [number, number] {
  const [fx, fy] = [Number(x), Number(y)]; // the point came from the frame: never let it be anything but a number
  if (![fx, fy].every(Number.isFinite) || fx < 0 || fy < 0 || fx > box.w || fy > box.h)
    throw new Error(FRAME_OUTSIDE_ERROR);
  return [box.x + fx, box.y + fy];
}

/** A function source called with its arguments JSON-encoded, so no value is ever spliced into code. */
const call = (fn: string, ...args: unknown[]) => `(${fn})(${args.map((a) => JSON.stringify(a)).join(', ')})`;

/** In the frame, the pointer and click sequence on the element at frame point (x, y), focusing it as a press would. */
const CLICK_AT = `(x, y) => {
  const el = document.elementFromPoint(x, y);
  if (!el) return false;
  const o = { bubbles: true, cancelable: true, composed: true, clientX: x, clientY: y, view: window, button: 0 };
  const p = { ...o, pointerType: 'mouse', isPrimary: true };
  el.dispatchEvent(new PointerEvent('pointerdown', p)); el.dispatchEvent(new MouseEvent('mousedown', o));
  if (typeof el.focus === 'function') el.focus({ preventScroll: true });
  el.dispatchEvent(new PointerEvent('pointerup', p)); el.dispatchEvent(new MouseEvent('mouseup', o));
  el.click();
  return true;
}`;

/** In the frame, the pointer moving onto the element at frame point (x, y), as a hover. */
const HOVER_AT = `(x, y) => {
  const el = document.elementFromPoint(x, y);
  const o = { bubbles: true, composed: true, clientX: x, clientY: y, view: window };
  for (const type of ['pointerover', 'mouseover', 'pointermove', 'mousemove'])
    el?.dispatchEvent(new (type.startsWith('pointer') ? PointerEvent : MouseEvent)(type, o));
  return !!el;
}`;

/** A click at a frame point: the <iframe> is focused from the page, then the element inside is pressed, unless an overlay covers it. */
function frameClick(driver: PageDriver, view: PageView, box: Box, frame: WebFrameMain) {
  const origin = originOf(frame.url);
  return async (_v: PageView, x: number, y: number): Promise<void> => {
    const [px, py] = toPage(box, x, y);
    if (!(await driver.deps.worldEval<boolean>(view, HITS_JS(origin, px, py)))) throw new Error(COVERED_TARGET_ERROR);
    await driver.deps.worldEval(view, `(() => { ${IFRAME_OF(origin)} f?.focus(); })()`);
    if (!(await driver.deps.worldEval<boolean>(view, call(CLICK_AT, x, y), frame)))
      throw new Error(COVERED_TARGET_ERROR);
  };
}

/**
 * The mouse inside a cross-process frame. Stock Electron sends native input to the page's
 * own widget and never routes it into another process's frame: a native click there gave
 * the frame focus but landed at the page's coordinates inside it, on whatever sat there.
 * So the frame's element is pressed from inside the frame, at the point the read found it.
 */
function frameMouse(driver: PageDriver, view: PageView, box: Box, frame: WebFrameMain): Mouse {
  const inFrame = (code: string) => driver.deps.worldEval(view, code, frame);
  const move = async (_v: PageView, x: number, y: number) =>
    void (toPage(box, x, y), await inFrame(call(HOVER_AT, x, y)));
  const scroll = async (_v: PageView, x: number, y: number, dx: number, dy: number) =>
    void (toPage(box, x, y), await inFrame(call(`(x, y) => window.scrollBy(x, y)`, Number(dx) || 0, Number(dy) || 0)));
  const click = frameClick(driver, view, box, frame);
  return Object.assign(Object.create(driver.mouse), { click, move, scroll }) as Mouse;
}

/**
 * In the frame, one key pressed on whatever has focus, doing what the key does: untrusted
 * key events alone move no focus and press no button, so Tab moves to the next control,
 * Enter and Space press a focused button or link, and editing keys edit. Answers false for
 * a key it cannot honour, so the agent is told rather than handed a silent success.
 */
const PRESS = `(k) => {
  const el = document.activeElement || document.body;
  const o = { key: k, bubbles: true, cancelable: true, composed: true };
  if (!el.dispatchEvent(new KeyboardEvent('keydown', o))) return true;
  const text = el.isContentEditable || el.matches('textarea, input:not([type=button],[type=submit],[type=checkbox],[type=radio],[type=reset])');
  const pressable = el.matches('button, a[href], summary, [role=button], input[type=button], input[type=submit], input[type=checkbox], input[type=radio]');
  let done = true;
  if (k === 'Backspace') document.execCommand('delete');
  else if (k === 'Delete') document.execCommand('forwardDelete');
  else if (k === 'Enter' && text && el.form && el.tagName === 'INPUT') el.form.requestSubmit();
  else if (k === 'Enter' && text) document.execCommand('insertLineBreak');
  else if ((k === 'Enter' || k === ' ') && pressable) el.click();
  else if (k === 'Tab') {
    const all = [...document.querySelectorAll('a[href], button, input, select, textarea, [tabindex]')]
      .filter((n) => !n.disabled && n.tabIndex >= 0 && n.getClientRects().length);
    all[(all.indexOf(el) + 1) % all.length]?.focus();
  } else if (k.length === 1 && text) document.execCommand('insertText', false, k);
  else done = k === 'Escape';
  el.dispatchEvent(new KeyboardEvent('keyup', o));
  return done;
}`;

/** A key press inside the frame, refused when the frame cannot honour that key. */
function framePress(inFrame: (code: string) => Promise<unknown>) {
  return async (_v: PageView, key: string): Promise<void> => {
    if (!(await inFrame(call(PRESS, key))))
      throw new Error(`${key} cannot be pressed inside another site’s frame yet.`);
  };
}

/** The keyboard inside a cross-process frame, for the same reason as frameMouse: native keys never reach it. */
function frameKeyboard(driver: PageDriver, view: PageView, frame: WebFrameMain): Keyboard {
  const inFrame = (code: string) => driver.deps.worldEval(view, code, frame);
  const type = async (_v: PageView, text: string) =>
    void (await inFrame(call(`(t) => document.execCommand('insertText', false, t)`, String(text))));
  const press = framePress(inFrame);
  const clear = async () => void (await inFrame(`document.execCommand('selectAll'); document.execCommand('delete')`));
  return Object.assign(Object.create(driver.keyboard), { type, press, clear }) as Keyboard;
}

/** The driver's dependencies with every script run in `frame`; the analyzer installs itself there on first use. */
function frameDeps(driver: PageDriver, frame: WebFrameMain): PageDriver['deps'] {
  const worldEval = <T>(v: PageView, expression: string) => driver.deps.worldEval<T>(v, expression, frame);
  return { ...driver.deps, worldEval, injectScripts: async () => {} };
}

/**
 * The driver for one command on a subframe element: its scripts, clicks and keys all run
 * inside that frame, after its box in the page is found and checked for overlays. Those
 * events are dispatched from the frame's isolated world, so the page sees them as untrusted
 * (isTrusted false); the page's own elements keep trusted native input. Ownership and Human
 * Mode were checked before the command reached the driver.
 */
export async function frameDriver(driver: PageDriver, view: PageView, frame: WebFrameMain): Promise<PageDriver> {
  const box = await frameBox(driver, view, frame);
  const deps = frameDeps(driver, frame);
  const mouse = frameMouse(driver, view, box, frame);
  const keyboard = frameKeyboard(driver, view, frame);
  return Object.assign(Object.create(driver), { deps, mouse, keyboard }) as PageDriver;
}

/** The frame's content box in the page, its <iframe> scrolled into view; refused when that <iframe> is gone. */
async function frameBox(driver: PageDriver, view: PageView, frame: WebFrameMain): Promise<Box> {
  const box = await driver.deps.worldEval<Box>(view, BOX_JS(originOf(frame.url)));
  if (!box?.ok) throw new Error(FRAME_GONE_ERROR);
  return box;
}
