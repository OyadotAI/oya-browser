/** Native committed navigation and removal events invalidate pending requests without a polling loop. */
import type { Event, WebContents } from 'electron';
/** Exact native event fields, independent of any external protocol schema. */
type Navigation = [
  event: Event,
  url: string,
  status: number,
  statusText: string,
  main: boolean,
  process: number,
  routing: number,
];
/** Native invalidation callback needs no external frame IDs or URLs. */
type Changed = (process?: number, routing?: number) => void;
/** Own only these listeners; other browser protection and page observers remain untouched. */
export function watchRequestFrames(contents: WebContents, changed: Changed): () => void {
  requireLifecycle(contents);
  const navigated = (...args: Navigation) => changed(args[5], args[6]);
  const removed = () => changed();
  contents.on('did-frame-navigate', navigated);
  contents.on('oya-frame-tree-changed' as 'dom-ready', removed);
  return () => unwatch(contents, navigated, removed);
}
/** Refuse old engines rather than pretending detached-frame notifications exist. */
export function requireLifecycle(contents: WebContents): void {
  const native = contents as WebContents & {
    /** Patched browser-owned lifecycle support. */ _supportsOyaFrameLifecycle?: () => boolean;
  };
  if (!native._supportsOyaFrameLifecycle?.()) throw Error('Oya engine lacks native request frame lifecycle');
}

/** Remove only the callbacks installed by this network observer. */
function unwatch(contents: WebContents, navigated: (...args: Navigation) => void, removed: () => void): void {
  contents.off('did-frame-navigate', navigated);
  contents.off('oya-frame-tree-changed' as 'dom-ready', removed);
}
