/** Exact synchronous pointer-focus provenance keeps shield reclamation outside admitted native dispatch. */
const focusing = new WeakSet<object>();
/** Focus hooks exempt only the exact browser-owned contents marked during this synchronous interval. */
export const isNativePointerFocus = (contents: object): boolean => focusing.has(contents);
/** Nested native calls retain their caller's marker and every exit revokes newly acquired provenance. */
export function withNativePointerFocus<T>(contents: object, operation: () => T): T {
  const nested = focusing.has(contents);
  focusing.add(contents);
  try {
    return operation();
  } finally {
    if (!nested) focusing.delete(contents);
  }
}

/** A focus source exposes only the native event needed by the tab's existing shield fence. */
interface FocusSource {
  /** Subscribe to native focus. */ on(event: 'focus', listener: () => void): unknown;
}
/** Normal and deferred focus reclamation remains active outside this exact synchronous provenance. */
export function installPointerFocusFence(contents: FocusSource, humanMay: () => boolean, reclaim: () => void): void {
  contents.on('focus', () => {
    if (!isNativePointerFocus(contents) && !humanMay()) reclaim();
  });
}
