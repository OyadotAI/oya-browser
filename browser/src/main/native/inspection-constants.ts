/** Hard bounds for native DOM inspection, independent of page-controlled size. */
export const INSPECTION = {
  /** Each document gets a disjoint node-id range. */
  nodes: 10000,
  /** Bound connection-owned native frame references. */
  frames: 128,
  /** Depth is bounded to avoid recursive page exhaustion. */
  depth: 32,
  /** Large text and markup fail explicitly instead of silently truncating. */
  text: 1048576,
};
