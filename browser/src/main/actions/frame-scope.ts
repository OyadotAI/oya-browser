/**
 * Cross-origin iframes in the agent's page read and in its actions. The analyzer reads a
 * same-origin iframe through its document, but another site's iframe (the Azure portal's
 * blades, embedded consoles) is a separate document in another process: the read showed
 * one "[iframe: src]" row with nothing inside, and the agent could neither read nor click
 * anything there. Each such frame is now read in its own isolated world (the tab preload
 * reaches every frame, so the page never sees it), its elements get ids from a range of
 * their own, and an action on one of those ids runs in the frame that owns it. Frames are
 * found at any depth and reached through their exact owner <iframe>, as Keel's read does, so
 * a frame nested in another and two frames from one site are each read and acted on.
 */
import type { WebFrameMain } from 'electron';
import type { NativePage as PageView } from '../native/index.ts';
import type { Analysis, Block, PageElement } from '../../page/render/types.ts';
import type { Mouse } from '../input/mouse.ts';
import type { Keyboard } from '../input/keyboard.ts';
import type { PageDriver } from './driver.ts';
import type { CommandParams, ScriptResult } from './types.ts';
import { debugRegistry, debugFrames, linkedDocument, topPoint } from '../browser-debug/index.ts';
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
  /** The document generation each slot's frame had when read: a same-address reload is another document too. */
  generation: Map<number, string>;
}

/** Each view's slots; a closed view takes its slots with it. */
const views = new WeakMap<PageView, Slots>();

/** The view's slots, made on first use. */
function slotsOf(view: PageView): Slots {
  let slots = views.get(view);
  if (!slots)
    views.set(
      view,
      (slots = { bySlot: new Map(), byFrame: new WeakMap(), next: 1, readAt: new Map(), generation: new Map() }),
    );
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

/**
 * How frames are found and reached: every frame in the tab's tree, each frame document's
 * generation, and a frame point moved up through each exact owner <iframe> to the page.
 * The same native owner links Keel's browser_read uses, so nested frames and two frames
 * from one site are each found and reached; tests pass their own.
 */
export interface FrameLinks {
  /** Every live frame under the page, nested ones included. */
  frames(view: PageView): WebFrameMain[];
  /** The frame's current document generation. */
  generation(view: PageView, frame: WebFrameMain): Promise<string>;
  /** A frame point in page pixels, refused when anything covers its owner there; scrolls and focuses each owner. */
  toTop(view: PageView, frame: WebFrameMain, x: number, y: number): Promise<PagePoint>;
}

/** A point in the page's own viewport pixels. */
interface PagePoint {
  /** Pixels from the left. */
  x: number;
  /** Pixels from the top. */
  y: number;
}

/** The browser's own links, through the browser-debug facade. */
const NATIVE_LINKS: FrameLinks = {
  frames: (view) => debugFrames(debugRegistry(view)),
  generation: async (view, frame) => (await debugRegistry(view).document(frame)).document,
  toTop: async (view, frame, x, y) => {
    const registry = debugRegistry(view);
    return topPoint(registry, await linkedDocument(registry, frame), { x, y }, true, false, true);
  },
};

/** The driver's frame links: a test's, or the browser's. */
const linksOf = (driver: PageDriver): FrameLinks => driver.deps.frameLinks ?? NATIVE_LINKS;

/** Whether the frame or any frame above it is a CAPTCHA challenge, never read or entered. */
function underChallenge(frame: WebFrameMain | null): boolean {
  for (let f = frame; f; f = f.parent) if (CHALLENGE.test(f.url)) return true;
  return false;
}

/**
 * Every frame the page's own read cannot reach: a web frame, at any depth, on another origin
 * than the frame that holds it (a same-origin child is read inside its parent).
 */
export function crossOriginFrames(driver: PageDriver, view: PageView): WebFrameMain[] {
  if (!view.webContents?.mainFrame) return []; // a view being torn down
  return linksOf(driver)
    .frames(view)
    .filter((f) => f.parent && !f.detached && /^https?:/.test(f.url) && !underChallenge(f))
    .filter((f) => originOf(f.url) !== originOf(f.parent!.url));
}

/** How many frames up `frame` sits from the page. */
function depthOf(frame: WebFrameMain): number {
  let depth = 0;
  for (let f = frame.parent; f; f = f.parent) depth++;
  return depth;
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

/** One frame's own read and document generation, or why it could not be read; a frame that loaded another document mid-read is not read. */
async function readOf(driver: PageDriver, view: PageView, frame: WebFrameMain, script: string) {
  const generation = await generationOf(driver, view, frame);
  const read = await driver.deps.worldEval<ScriptResult<Analysis>>(view, script, frame).catch((e: Error) => e);
  if (read instanceof Error) return `not readable: ${read.message}`;
  if (!read?.data?.blocks) return 'not readable: no answer';
  const after = generation && (await generationOf(driver, view, frame));
  return after === generation ? { data: read.data, generation } : 'not readable: it changed while being read';
}

/** The frame's document generation, or '' when it cannot be learned (the address check then stands alone). */
const generationOf = (driver: PageDriver, view: PageView, frame: WebFrameMain): Promise<string> =>
  linksOf(driver)
    .generation(view, frame)
    .catch(() => '');

/** A frame's read: its analysis and generation, or why there is none. */
type FrameRead = Awaited<ReturnType<typeof readOf>>;

/** Which read document each row came from: 0 for the page, else the frame's slot. */
type Owners = WeakMap<Block, number>;

/** The nearest frame above `frame` whose rows are in the read (the page, or a frame read on its own). */
function holderOf(view: PageView, frame: WebFrameMain, read: Set<WebFrameMain>): number {
  for (let f = frame.parent; f; f = f.parent) if (read.has(f)) return slotOf(view, f);
  return 0;
}

/** The merge's shared state: the read so far, who owns each row, and the frames being read. */
interface Merge {
  /** The page's read, frames merged in as they come. */
  data: Analysis;
  /** Each row's document. */
  owners: Owners;
  /** The frames being read. */
  frames: WebFrameMain[];
}

/**
 * The row a frame's read goes under: its holder's one iframe row from its origin, when its
 * holder has exactly one frame from that origin; otherwise a new row naming the frame. Rows
 * only place the read for the agent's eyes: actions find the frame by its slot, never its row.
 */
function rowFor(view: PageView, m: Merge, frame: WebFrameMain): Block {
  const [rows, siblings] = sameOrigin(view, m, frame);
  if (rows.length === 1 && siblings.length === 1) return rows[0];
  const row = { kind: 'iframe', target: frame.url, region: 'iframe' } as Block;
  m.data.blocks = [...(m.data.blocks ?? []), row];
  return row;
}

/** The iframe rows, and the frames being read, from `frame`'s origin in the same document as `frame`. */
function sameOrigin(view: PageView, m: Merge, frame: WebFrameMain): [Block[], WebFrameMain[]] {
  const read = new Set(m.frames);
  const holder = holderOf(view, frame, read);
  const same = (url: unknown) => originOf(String(url ?? '')) === originOf(frame.url);
  const mine = (b: Block) => b.kind === 'iframe' && (m.owners.get(b) ?? 0) === holder && same(b.target);
  return [
    (m.data.blocks ?? []).filter(mine),
    m.frames.filter((f) => holderOf(view, f, read) === holder && same(f.url)),
  ];
}

/** Puts one frame's read under its row, or says on the row why it could not be read. */
function mergeOne(view: PageView, m: Merge, frame: WebFrameMain, read: FrameRead): void {
  const row = rowFor(view, m, frame);
  if (typeof read === 'string') return void (row.target = `${row.target} (${read})`);
  const slot = slotOf(view, frame);
  slotsOf(view).readAt.set(slot, frame.url);
  slotsOf(view).generation.set(slot, read.generation);
  splice(m.data, row, adopted(m, shifted(read.data, slot * FRAME_ID_STRIDE), slot, row));
}

/** The frame's rows marked as its own, and on screen only where its row is: the frame judged them by its own viewport. */
function adopted(m: Merge, child: ReturnType<typeof shifted>, slot: number, row: Block): ReturnType<typeof shifted> {
  for (const b of child.blocks) {
    m.owners.set(b, slot);
    b.screen = !!(b.screen && row.screen);
  }
  return child;
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
 * Reads every frame at once, not in turn, so a silent frame waits out its deadline without
 * holding the others, then merges outer frames before inner ones, so a nested frame's row
 * is already in the read when its own read goes under it.
 */
async function readAll(driver: PageDriver, view: PageView, m: Merge, script: string) {
  const reads = await Promise.all(m.frames.map((f) => readOf(driver, view, f, script)));
  const order = m.frames.map((f, i) => [f, reads[i]] as const).sort(([a], [b]) => depthOf(a) - depthOf(b));
  for (const [frame, read] of order) mergeOne(view, m, frame, read);
}

/** The frames to read, or none when the tab's frame tree is too large or deep to walk. */
function framesOf(driver: PageDriver, view: PageView): WebFrameMain[] {
  try {
    return crossOriginFrames(driver, view);
  } catch {
    return []; // the page's own read still answers; its iframe rows stay as they are
  }
}

/** The page's read with every cross-origin frame, at any depth, read in place; `script` is the analyze script. */
export async function withFrames(driver: PageDriver, view: PageView, raw: unknown, script: string): Promise<unknown> {
  const data = (raw as ScriptResult<Analysis> | null)?.data;
  const frames = data?.blocks ? framesOf(driver, view) : [];
  if (!data?.blocks || !frames.length) return raw;
  const before = data.blocks.length;
  await readAll(driver, view, { data, owners: new WeakMap(), frames }, script);
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

/**
 * In a document: whether its focused element is a field of its own (not the body, not an
 * <iframe>), and the origin of the <iframe> it has focused, if any. A document keeps its focused
 * element while the window is in the background, so this holds when an agent drives an
 * unfocused Oya, where document.hasFocus() is false everywhere.
 */
const FOCUS_JS = `(() => {
  let a = document.activeElement;
  while (a && a.shadowRoot && a.shadowRoot.activeElement) a = a.shadowRoot.activeElement;
  const iframe = !!a && a.matches('iframe,frame');
  let origin = '';
  if (iframe) try { origin = new URL(a.src, location.href).origin; } catch {}
  return { leaf: !!a && a !== document.body && a !== document.documentElement && !iframe, origin };
})()`;

/** One document's focus: whether it holds a field, and which origin's <iframe> it has focused. */
interface FocusState {
  /** Its focused element is a field of its own. */
  leaf: boolean;
  /** The origin of the <iframe> it focused, or ''. */
  origin: string;
}

/** Every web frame's focus state, the page's under the main frame; a frame that cannot answer holds none. */
async function focusStates(driver: PageDriver, view: PageView): Promise<Map<WebFrameMain, FocusState>> {
  const main = view.webContents.mainFrame;
  const frames = webFrames(driver, view);
  const ask = (f?: WebFrameMain) => driver.deps.worldEval<FocusState>(view, FOCUS_JS, f).catch(() => null);
  const states = await Promise.all([ask(), ...frames.map((f) => ask(f))]);
  return new Map([main, ...frames].map((f, i) => [f, states[i] ?? { leaf: false, origin: '' }]));
}

/** Every live web frame under the page outside a CAPTCHA challenge; none when the tree is too large or deep to walk. */
function webFrames(driver: PageDriver, view: PageView): WebFrameMain[] {
  try {
    return linksOf(driver)
      .frames(view)
      .filter((f) => !f.detached && /^https?:/.test(f.url) && !underChallenge(f));
  } catch {
    return [];
  }
}

/** Whether focus runs from the page down to `frame`: each holder has focused an <iframe> from the next frame's origin. */
function onFocusPath(frame: WebFrameMain, states: Map<WebFrameMain, FocusState>): boolean {
  for (let f = frame; f.parent; f = f.parent) if (states.get(f.parent)?.origin !== originOf(f.url)) return false;
  return true;
}

/** Whether a frame sits behind another site's boundary, where native keys sent to the page never arrive; a same-site frame takes them. */
function crossesSite(frame: WebFrameMain): boolean {
  for (let f = frame; f.parent; f = f.parent) if (originOf(f.url) !== originOf(f.parent.url)) return true;
  return false;
}

/** The frame, at any depth, whose own field has the focus, where native keys cannot reach; null when the page holds it, or it cannot be told. */
async function focusedFrame(driver: PageDriver, view: PageView): Promise<WebFrameMain | null> {
  if (!view.webContents?.mainFrame) return null; // a view being torn down: the page keeps its keys
  const states = await focusStates(driver, view);
  const main = view.webContents.mainFrame;
  const focused = [...states].filter(([f, st]) => f !== main && st.leaf && onFocusPath(f, states) && crossesSite(f));
  return focused.length === 1 ? focused[0][0] : null;
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

/** A frame point as numbers, refused when it is not a finite point inside the frame's own pixels. */
function framePoint(x: unknown, y: unknown): [number, number] {
  const [fx, fy] = [Number(x), Number(y)]; // the point came from the frame: never let it be anything but a number
  if (![fx, fy].every(Number.isFinite) || fx < 0 || fy < 0) throw new Error(FRAME_OUTSIDE_ERROR);
  return [fx, fy];
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

/**
 * A click at a frame point: it is moved up through each exact owner <iframe> to the page,
 * which scrolls each owner into view, refuses a point any overlay covers (or that falls
 * outside the frame) and focuses each owner, then the element inside is pressed.
 */
function frameClick(driver: PageDriver, view: PageView, frame: WebFrameMain) {
  return async (_v: PageView, x: number, y: number): Promise<void> => {
    const [fx, fy] = framePoint(x, y);
    await linksOf(driver)
      .toTop(view, frame, fx, fy)
      .catch((e: Error) => Promise.reject(/covered/i.test(String(e?.message)) ? new Error(COVERED_TARGET_ERROR) : e));
    if (!(await driver.deps.worldEval<boolean>(view, call(CLICK_AT, fx, fy), frame)))
      throw new Error(COVERED_TARGET_ERROR);
  };
}

/**
 * The mouse inside a cross-process frame. Stock Electron sends native input to the page's
 * own widget and never routes it into another process's frame: a native click there gave
 * the frame focus but landed at the page's coordinates inside it, on whatever sat there.
 * So the frame's element is pressed from inside the frame, at the point the read found it.
 */
function frameMouse(driver: PageDriver, view: PageView, frame: WebFrameMain): Mouse {
  const inFrame = (code: string) => driver.deps.worldEval(view, code, frame);
  const move = async (_v: PageView, x: number, y: number) => void (await inFrame(call(HOVER_AT, ...framePoint(x, y))));
  const scroll = async (_v: PageView, x: number, y: number, dx: number, dy: number) =>
    void (framePoint(x, y), await inFrame(call(`(x, y) => window.scrollBy(x, y)`, Number(dx) || 0, Number(dy) || 0)));
  const click = frameClick(driver, view, frame);
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
export async function frameDriver(
  driver: PageDriver,
  view: PageView,
  frame: WebFrameMain,
  named = true,
): Promise<PageDriver> {
  // A key sent to whichever frame has focus names no element from a read, so no read's document is required.
  await (named ? requireSameDocument(driver, view, frame) : requireLive(frame));
  const scoped = { deps: frameDeps(driver, frame), mouse: frameMouse(driver, view, frame) };
  return Object.assign(Object.create(driver), scoped, { keyboard: frameKeyboard(driver, view, frame) }) as PageDriver;
}

/** Refuses a frame that is gone. */
async function requireLive(frame: WebFrameMain): Promise<void> {
  if (frame.detached) throw new Error(FRAME_GONE_ERROR);
}

/** Refuses a frame that is gone, or that loaded another document (even at the same address) since it was read. */
async function requireSameDocument(driver: PageDriver, view: PageView, frame: WebFrameMain): Promise<void> {
  if (frame.detached) throw new Error(FRAME_GONE_ERROR);
  const slot = slotsOf(view).byFrame.get(frame);
  const read = slot === undefined ? '' : (slotsOf(view).generation.get(slot) ?? '');
  if (!read) return; // focus routing, or a read that could not learn the generation: the address check stands
  const now = await linksOf(driver)
    .generation(view, frame)
    .catch(() => '');
  if (now !== read) throw new Error(FRAME_GONE_ERROR);
}
