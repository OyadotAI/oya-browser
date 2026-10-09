/** Browser-owned page execution and capture, with no debugging protocol transport. */
import type { WebContents } from 'electron';
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
