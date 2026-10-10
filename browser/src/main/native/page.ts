/** Browser-owned page execution and capture, with no debugging protocol transport. */
import type { WebContents } from 'electron';
import { JPEG_MAX_QUALITY } from './constants.ts';
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
/** Evaluate in the page's main world only when the calling command explicitly requests that world. */
export async function evaluatePage<T = unknown>(view: NativePage, expression: string): Promise<T> {
  if (view.webContents.isDestroyed()) throw new Error('View is destroyed');
  return view.webContents.executeJavaScript(expression);
}
/** Capture rendered page pixels, preserving the existing PNG/JPEG data-URL response contract. */
export async function capturePage(view: NativePage | null, jpegQuality?: number): Promise<string> {
  if (!view || view.webContents.isDestroyed()) throw new Error('View is destroyed');
  const image = await view.webContents.capturePage();
  if (image.isEmpty()) throw new Error('The page has no rendered pixels to capture');
  const jpeg = jpegQuality !== undefined;
  const bytes = jpeg ? image.toJPEG(jpegQuality) : image.toPNG();
  return `data:image/${jpeg ? 'jpeg' : 'png'};base64,${bytes.toString('base64')}`;
}
