/** Classify real native stacking without moving a shield, personal view or owned page. */
import type { BrowserWindow, BrowserView, Rectangle } from 'electron';
import type { NativePage } from '../native/index.ts';
/** A native browser layer is distinct from an overlay inside the web document. */
export type DebugObstruction = 'browser_control_shield' | 'native_occlusion';
/** Exact native identities establish ownership; overlapping mounted views establish obstruction. */
export function inspectDebugMouseSurface(window: BrowserWindow, page: NativePage, shield: BrowserView | null) {
  const views = window.getBrowserViews(),
    index = views.findIndex((view) => view.webContents === page.webContents);
  if (index < 0 && window.webContents !== page.webContents) throw Error('The owned native page is no longer mounted.');
  const size = window.getContentSize(),
    bounds = index < 0 ? { x: 0, y: 0, width: size[0], height: size[1] } : views[index].getBounds();
  const above = views.slice(index + 1).filter((view) => overlapsDebugSurface(bounds, view.getBounds()));
  const obstruction: DebugObstruction | undefined = above.includes(shield!)
    ? 'browser_control_shield'
    : above.length
      ? 'native_occlusion'
      : undefined;
  return { bounds, obstruction };
}
/** Nonoverlapping native surfaces cannot prevent this owned view's pointer routing. */
function overlapsDebugSurface(a: Rectangle, b: Rectangle): boolean {
  return (
    a.width > 0 &&
    a.height > 0 &&
    b.width > 0 &&
    b.height > 0 &&
    a.x < b.x + b.width &&
    a.x + a.width > b.x &&
    a.y < b.y + b.height &&
    a.y + a.height > b.y
  );
}
