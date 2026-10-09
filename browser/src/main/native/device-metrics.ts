/** Native viewport emulation with connection ownership and restoration; no debugging protocol is involved. */
import type { WebContents, Parameters } from 'electron';
import type { NativePage } from './page.ts';
/** Largest supported viewport dimension, bounding renderer resource use. */
const MAX_DIMENSION = 10000;
/** Largest supported device pixel ratio. */
const MAX_PIXEL_RATIO = 4;
/** A second socket cannot clear or replace another socket's emulation. */
const OWNERS = new WeakMap<WebContents, NativeDeviceMetrics>();
/** Validate CDP metric values before any browser-owned state changes. */
export function metricParameters(params: Record<string, unknown>): Parameters {
  const { width, height, deviceScaleFactor, mobile } = params;
  validateDimension(width);
  validateDimension(height);
  validatePixelRatio(deviceScaleFactor);
  if (typeof mobile !== 'boolean') throw Error('mobile must be a boolean');
  const size = { width: Number(width), height: Number(height) };
  return buildParameters(size, Number(deviceScaleFactor), mobile);
}
/** Zero removes a dimension override according to the native engine's device metrics contract. */
function validateDimension(value: unknown): void {
  if (!Number.isInteger(value) || Number(value) < 0 || Number(value) > MAX_DIMENSION)
    throw Error('Viewport dimensions must be integers from 0 to 10000');
}
/** Own emulation only while the requesting connection is alive. */
export class NativeDeviceMetrics {
  /** Strong references exist only until clear, destruction, or disconnect. */
  private readonly pages = new Map<WebContents, () => void>();
  /** Apply validated metrics to the exact commanded native surface. */
  set(page: NativePage, params: Record<string, unknown>): object {
    const parameters = metricParameters(params),
      contents = page.webContents;
    this.check(contents);
    contents.enableDeviceEmulation(parameters);
    this.own(contents);
    return {};
  }
  /** Restore native metrics only when this connection owns the override. */
  clear(page: NativePage): object {
    const contents = page.webContents;
    this.check(contents);
    if (OWNERS.get(contents) !== this) return {};
    contents.disableDeviceEmulation();
    this.forget(contents);
    return {};
  }
  /** Shared pages cannot acquire two incompatible connection-local viewport leases. */
  private check(contents: WebContents): void {
    if (contents.isDestroyed()) throw Error('View is destroyed');
    const owner = OWNERS.get(contents);
    if (owner && owner !== this) throw Error('Device metrics are owned by another connection');
  }
  /** Native destruction releases ownership without calling methods on a dead renderer. */
  private own(contents: WebContents): void {
    if (this.pages.has(contents)) return;
    const cleanup = (): void => this.forget(contents);
    OWNERS.set(contents, this);
    this.pages.set(contents, cleanup);
    contents.once('destroyed', cleanup);
  }
  /** Forget exactly this connection's override and associated listener. */
  private forget(contents: WebContents): void {
    const listener = this.pages.get(contents);
    if (listener) contents.off('destroyed', listener);
    this.pages.delete(contents);
    if (OWNERS.get(contents) === this) OWNERS.delete(contents);
  }
  /** Socket cleanup restores defaults instead of leaving the person's browser emulated. */
  dispose(): void {
    for (const contents of this.pages.keys()) this.restore(contents);
  }
  /** A closing renderer must not abort cleanup of the connection’s other pages. */
  private restore(contents: WebContents): void {
    try {
      if (!contents.isDestroyed()) contents.disableDeviceEmulation();
    } catch {
      console.warn('[native-cdp] Could not restore device metrics');
    } finally {
      this.forget(contents);
    }
  }
}

/** Native parameter construction has one explicit screen, viewport and pixel-ratio mapping. */
function buildParameters(size: Electron.Size, deviceScaleFactor: number, mobile: boolean): Parameters {
  return {
    screenPosition: mobile ? 'mobile' : 'desktop',
    screenSize: size,
    viewSize: size,
    viewPosition: { x: 0, y: 0 },
    deviceScaleFactor,
    scale: 1,
  };
}
/** Non-finite or excessive pixel ratios must not reach native renderer allocation. */
function validatePixelRatio(value: unknown): void {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value > MAX_PIXEL_RATIO)
    throw Error('Device scale factor must be between 0 and 4');
}
