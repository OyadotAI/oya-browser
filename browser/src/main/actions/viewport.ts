/** Session-owned native viewport overrides preserve CSS layout independently of Oya's window. */
import type { WebContents } from 'electron';
import { NativeDeviceMetrics, type NativePage, type CaptureViewport } from '../native/index.ts';
import { currentLane } from '../control/agent-tab.ts';
import type { PageDriver } from './driver.ts';
import { viewportDeadline } from './viewport-deadline.ts';
import type { CommandParams } from './types.ts';

/** Renderer allocations stay bounded even with a high device pixel ratio. */
const MAX_PIXELS = 16777216;
/** Per-axis CSS dimension ceiling shared with Keel's tool contract. */
const MAX_DIMENSION = 4096;
/** Bounding session state avoids abandoned clients growing a permanent registry. */
const MAX_SESSIONS = 256;
/** The native renderer accepts zero for its natural device pixel ratio. */
const NATIVE_DPR = 0;
/** Explicit density limits exclude invalid and implausible allocation requests. */
const MIN_DPR = 0.25,
  MAX_DPR = 4;
/** Only these fields can alter this operation; ids cannot select a different tab. */
const FIELDS = new Set(['width', 'height', 'deviceScaleFactor', 'mobile', 'reset']);

/** A session's native lease and the settings retained for each of its owned tabs. */
interface Owner {
  /** Revocation fences operations paused on renderer acknowledgements. */
  active: boolean;
  /** Failed restoration requires an explicit reset before any further sizing. */
  uncertain: Set<WebContents>;
  /** The existing native helper enforces exclusive emulation ownership. */
  metrics: NativeDeviceMetrics;
  /** Closing the last tab releases this session record. */
  pages: Map<WebContents, Record<string, unknown>>;
  /** Explicit reset removes destruction callbacks as well as retained settings. */
  listeners: Map<WebContents, () => void>;
}
/** An admitted session carries the lease belonging to its exact lane. */
interface Session {
  /** MCP ownership identity is never supplied as a viewport argument. */
  lane: string;
  /** Native emulation owner retained through this command. */
  owner: Owner;
}
/** References are removed on session end, shutdown, or destruction of their last native tab. */
const owners = new Map<string, Owner>();

/** Read actual CSS metrics in Oya's existing isolated world after the native resize has painted. */
export async function viewportInfo(driver: PageDriver, view: NativePage): Promise<Record<string, number | string>> {
  return viewportDeadline(
    driver.deps.worldEval(
      view,
      `new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve({width:innerWidth,height:innerHeight,dpr:devicePixelRatio,url:location.href}))))`,
    ),
  );
}

/** Validate the whole request before creating an owner or touching native emulation. */
function validate(params: CommandParams): void {
  if (Object.keys(params).some((key) => !FIELDS.has(key))) throw Error('Unknown viewport setting');
  if (params.mobile === true)
    throw Error('mobile/touch emulation is unsupported. The viewport was not changed. Retry without mobile:true.');
  if (params.mobile !== undefined && typeof params.mobile !== 'boolean') throw Error('mobile must be a boolean');
  if (params.reset !== undefined && typeof params.reset !== 'boolean') throw Error('reset must be a boolean');
  if (params.reset) return validateReset(params);
  validateSize(params);
}

/** Reset cannot silently discard other settings in the same request. */
function validateReset(params: CommandParams): void {
  if (Object.keys(params).some((key) => key !== 'reset'))
    throw Error('reset:true cannot be combined with viewport settings');
}

/** Keep invalid dimensions and explicit DPRs out of native renderer allocation. */
function validateSize(params: CommandParams): void {
  for (const value of [params.width, params.height])
    if (!Number.isInteger(value) || Number(value) < 1 || Number(value) > MAX_DIMENSION)
      throw Error('width and height must be whole numbers from 1 to 4096 CSS pixels');
  const dpr = params.deviceScaleFactor;
  if (dpr !== undefined && (!Number.isFinite(dpr) || dpr < MIN_DPR || dpr > MAX_DPR))
    throw Error('deviceScaleFactor must be between 0.25 and 4');
}

/** Restore exactly one session's leases, including on an MCP DELETE. */
export function releaseViewport(lane: string): void {
  const owner = owners.get(lane);
  if (owner) {
    owner.active = false;
    owner.metrics.dispose();
    for (const contents of owner.pages.keys()) forget(owner, contents);
  }
  owners.delete(lane);
}
/** Shutdown restores all leases before the port stops accepting work. */
export function releaseAllViewports(): void {
  for (const lane of owners.keys()) releaseViewport(lane);
}

/** Register native destruction once, so closed tabs cannot retain session state indefinitely. */
function remember(lane: string, owner: Owner, view: NativePage, params: Record<string, unknown>): void {
  const contents = view.webContents;
  if (!owner.pages.has(contents)) watchDestruction(lane, owner, contents);
  owner.pages.set(contents, params);
  owners.set(lane, owner);
}
/** Native destruction releases the last session without retaining callbacks across resets. */
function watchDestruction(lane: string, owner: Owner, contents: WebContents): void {
  const cleanup = () => {
    forget(owner, contents);
    if (!owner.pages.size && owners.get(lane) === owner) owners.delete(lane);
  };
  owner.listeners.set(contents, cleanup);
  contents.once('destroyed', cleanup);
}
/** Reset and session end remove the exact registered callback. */
function forget(owner: Owner, contents: WebContents): void {
  const listener = owner.listeners.get(contents);
  if (listener) contents.off('destroyed', listener);
  owner.listeners.delete(contents);
  owner.pages.delete(contents);
  owner.uncertain.delete(contents);
}

/** Apply only to the command runner's exact owned view and retain optional density across navigation. */
export async function setViewport(driver: PageDriver, view: NativePage, input: CommandParams) {
  validate(input);
  const session = ownerFor(view);
  requireConfirmed(session, view, input);
  try {
    return await (input.reset
      ? reset(driver, view, session.lane, session.owner)
      : applyViewport(driver, view, input, session));
  } finally {
    if (!session.owner.pages.size && owners.get(session.lane) === session.owner) owners.delete(session.lane);
  }
}
/** Resolve a bounded session lease before any mutation. */
function ownerFor(view: NativePage): Session {
  const lane = currentLane();
  if (!lane.startsWith('mcp-')) throw Error('Viewport control is available through local MCP only');
  requireViewportNative(view);
  if (!owners.has(lane) && owners.size >= MAX_SESSIONS)
    throw Error('Viewport session capacity reached. Close an inactive session.');
  const owner = owners.get(lane) || newOwner();
  owners.set(lane, owner);
  return { lane, owner };
}
/** Read and validate the merged effective density before applying native settings. */
async function applyViewport(driver: PageDriver, view: NativePage, input: CommandParams, session: Session) {
  const { owner } = session;
  const before = await viewportInfo(driver, view);
  const params = { deviceScaleFactor: NATIVE_DPR, ...owner.pages.get(view.webContents), ...input, mobile: false };
  requireActive(session);
  checkArea(params, Number(before.dpr));
  const change = { driver, view, session, params, previous: owner.pages.get(view.webContents) };
  owner.metrics.set(view, params);
  return confirm(change);
}

/** Reset native metrics before forgetting retained settings; a foreign lease still fails closed. */
async function reset(driver: PageDriver, view: NativePage, lane: string, owner: Owner) {
  owner.metrics.clear(view);
  forget(owner, view.webContents);
  if (!owner.pages.size) owners.delete(lane);
  return viewportInfo(driver, view);
}
/** Natural density uses the measured renderer value; invalid measurements cannot relax the cap. */
function checkArea(params: Record<string, unknown>, natural: number): void {
  const dpr = Number(params.deviceScaleFactor) || natural;
  if (!Number.isFinite(dpr) || dpr <= 0 || Number(params.width) * Number(params.height) * dpr * dpr > MAX_PIXELS)
    throw Error('Viewport exceeds 16,777,216 physical pixels. Use a smaller viewport or DPR.');
}

/** Native capability checks precede acquiring a renderer lease. */
function requireViewportNative(view: NativePage): void {
  if (typeof view.webContents.enableDeviceEmulation !== 'function')
    throw Error('Native viewport control is unavailable in this Oya build');
}
/** Revoked sessions cannot resume a pending request into a new lease. */
function requireActive({ lane, owner }: Session): void {
  if (!owner.active || owners.get(lane) !== owner)
    throw Error('Viewport session ended before native changes; no viewport was changed.');
}

/** Empty native leases allocate no renderer state until a validated request is applied. */
function newOwner(): Owner {
  return {
    active: true,
    uncertain: new Set(),
    metrics: new NativeDeviceMetrics(),
    pages: new Map(),
    listeners: new Map(),
  };
}

/** A viewport transaction keeps old settings until the renderer confirms new dimensions. */
interface Change {
  /** Metrics are measured through the existing isolated world. */
  driver: PageDriver;
  /** Exact native surface admitted by the command runner. */
  view: NativePage;
  /** Revocation and restoration belong to this session. */
  session: Session;
  /** Validated native settings awaiting confirmation. */
  params: Record<string, unknown>;
  /** Previous confirmed override, absent for natural sizing. */
  previous?: Record<string, unknown>;
}
/** Commit retained density only after measurement succeeds; failures restore the prior lease. */
async function confirm(change: Change) {
  try {
    const measured = await viewportInfo(change.driver, change.view);
    verify(change, measured);
    remember(change.session.lane, change.session.owner, change.view, change.params);
    return measured;
  } catch (error) {
    return rollback(change, error);
  }
}
/** Acknowledgement of different dimensions cannot count as a successful viewport override. */
function verifySize(params: Record<string, unknown>, measured: Record<string, number | string>): void {
  if (measured.width !== params.width || measured.height !== params.height)
    throw Error('Native renderer did not confirm the requested viewport dimensions');
}
/** A revoked session already disposed its lease; it must never restore it into a replacement. */
function rollback(change: Change, error: unknown): never {
  const { session } = change;
  if (!session.owner.active) throw error;
  try {
    restore(change);
  } catch (restore) {
    return uncertain(change, error, restore);
  }
  throw Error(`Could not measure the new viewport; previous settings restored: ${String(error)}`);
}
/** Preserve the lease for reset/cleanup without allowing later calls to inherit uncertain density. */
function uncertain(change: Change, error: unknown, restore: unknown): never {
  const { session, view } = change;
  remember(session.lane, session.owner, view, change.previous || change.params);
  session.owner.uncertain.add(view.webContents);
  throw Error(
    `Viewport measurement failed: ${String(error)}. Restoring prior settings also failed: ${String(restore)}. Use reset:true before resizing.`,
  );
}

/** Uncertain restoration cannot silently supply density to the next request. */
function requireConfirmed(session: Session, view: NativePage, input: CommandParams): void {
  if (!input.reset && session.owner.uncertain.has(view.webContents))
    throw Error('Viewport restoration is unconfirmed. Use reset:true before changing its size.');
}
/** Renderer acknowledgement must belong to this live lease and the requested size. */
function verify(change: Change, measured: Record<string, number | string>): void {
  requireActive(change.session);
  verifySize(change.params, measured);
}
/** Restore the last confirmed native override, or clear a first unconfirmed lease. */
function restore({ session, view, previous }: Change): void {
  if (previous) session.owner.metrics.set(view, previous);
  else session.owner.metrics.clear(view);
}

/** Native screenshots crop the active lease rather than the surrounding window. */
export function viewportCaptureSize(view: NativePage): CaptureViewport | undefined {
  const owner = owners.get(currentLane());
  if (owner?.uncertain.has(view.webContents))
    throw Error('Viewport settings are unconfirmed. Use reset:true before taking a screenshot.');
  const params = owner?.pages.get(view.webContents);
  return params ? { width: Number(params.width), height: Number(params.height) } : undefined;
}
