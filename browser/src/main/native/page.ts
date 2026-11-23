/** Browser-owned page execution and capture, with no debugging protocol transport. */
import type { NativeImage, WebContents } from 'electron';
import {
  CAPTURE_VIZ_BACKOFF_MS,
  CAPTURE_VIZ_TRIES,
  JPEG_MAX_QUALITY,
  NATIVE_CAPTURE_TIMEOUT_MS,
  NATIVE_CAPTURE_TIMEOUT_MESSAGE,
} from './constants.ts';
import { withinTime } from '../../shared/within-time.ts';
/** Validate explicit capture options before reading pixels or invoking the native encoder. */
export function screenshotOptions(params: ScreenshotInput): ScreenshotOptions {
  const format = params.format ?? 'png';
  const quality = params.quality;
  if (format !== 'png' && format !== 'jpeg') throw Error('Unsupported screenshot format');
  if (quality === undefined) return { format };
  if (format !== 'jpeg' || !validQuality(quality)) throw Error('JPEG quality must be an integer from 0 to 100');
  return { format, quality: quality as number };
}
/** Untrusted optional encoder arguments from either command boundary. */
interface ScreenshotInput {
  /** A requested native image encoding. */
  format?: unknown;
  /** An untrusted percentage, validated without coercion. */
  quality?: unknown;
}
/** The validated native encoder selection. */
interface ScreenshotOptions {
  /** PNG is lossless; JPEG accepts an explicit quality. */
  format: string;
  /** Optional integer percentage passed unchanged to Electron's encoder. */
  quality?: number;
}
/** Refuse coercion, fractional percentages and values outside the encoder's range. */
function validQuality(value: unknown): boolean {
  return typeof value === 'number' && Number.isInteger(value) && value >= 0 && value <= JPEG_MAX_QUALITY;
}
/** A live page surface owned by Oya. */
export interface NativePage {
  /** The actual renderer, not a remote debugging target. */
  webContents: WebContents;
}
/**
 * Evaluate in the page's main world only when the calling command explicitly requests that world.
 * Through the main frame: `webContents.executeJavaScript` waits until the page stops loading, so
 * one request that never ends held every read (and the server's sign-in checks) for a full timeout.
 */
export async function evaluatePage<T = unknown>(view: NativePage, expression: string): Promise<T> {
  if (view.webContents.isDestroyed()) throw new Error('View is destroyed');
  return (view.webContents.mainFrame ?? view.webContents).executeJavaScript(expression) as Promise<T>;
}
/** The emulated viewport's logical capture region, independent of unused window space. */
export interface CaptureViewport {
  /** Width in native view pixels before the encoder applies its host scale. */
  width: number;
  /** Height in native view pixels. */
  height: number;
}
/** Capture rendered page pixels, preserving the existing PNG/JPEG data-URL response contract. */
export async function capturePage(view: NativePage | null, jpegQuality?: number, viewport?: CaptureViewport) {
  if (!view || view.webContents.isDestroyed()) throw new Error('View is destroyed');
  const image = await boundedFrame(view, viewport);
  if (image.isEmpty()) throw new Error('The page has no rendered pixels to capture');
  verifyCaptureSize(image, viewport);
  return imageUrl(image, jpegQuality);
}
/** Bound the entire native read, so a wedged compositor cannot hold automation control forever. */
function boundedFrame(view: NativePage, viewport?: CaptureViewport): Promise<NativeImage> {
  return withinTime(
    compositorFrame(view.webContents, viewport),
    NATIVE_CAPTURE_TIMEOUT_MS,
    NATIVE_CAPTURE_TIMEOUT_MESSAGE,
  );
}
/** Native encoding preserves captured pixels; emulated DPR is reported separately from PNG dimensions. */
function imageUrl(image: NativeImage, jpegQuality?: number): string {
  const jpeg = jpegQuality !== undefined;
  const bytes = jpeg ? image.toJPEG(jpegQuality) : image.toPNG();
  return `data:image/${jpeg ? 'jpeg' : 'png'};base64,${bytes.toString('base64')}`;
}

/**
 * The page's current frame. Under Xvfb, as in the Linux cloud runtime, the first capture after a
 * navigation can fail with UnknownVizError while the GPU process catches up; that is the display,
 * not the page, so it is tried again a few times. Any other failure is reported at once.
 */
async function compositorFrame(contents: WebContents, viewport?: CaptureViewport, attempt = 1): Promise<NativeImage> {
  try {
    return await contents.capturePage(viewport ? { x: 0, y: 0, ...viewport } : undefined);
  } catch (err) {
    if (attempt >= CAPTURE_VIZ_TRIES || !/UnknownVizError/.test(String(err))) throw err;
    await new Promise((resolve) => setTimeout(resolve, CAPTURE_VIZ_BACKOFF_MS * attempt));
    return compositorFrame(contents, viewport, attempt + 1);
  }
}

/** A native compositor can clip requests to its window; never pass that off as a full viewport. */
function verifyCaptureSize(image: NativeImage, viewport?: CaptureViewport): void {
  if (!viewport) return;
  const size = image.getSize();
  if (size.width !== viewport.width || size.height !== viewport.height)
    throw Error(
      'The viewport does not fit Oya’s native capture surface. Enlarge the Oya window or choose a smaller viewport, then retry the screenshot.',
    );
}
