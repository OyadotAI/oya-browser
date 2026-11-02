# Consolidate without losing behavior

Classify dependencies before moving responsibilities:

| Dependency | Design and evidence |
| --- | --- |
| In-process computation or memory | Keep direct composition; check observable outcomes through the caller-facing interface. |
| Local storage with a realistic substitute | Keep the substitution internal where possible; document which persistence or concurrency guarantees it cannot reproduce. |
| Owned remote service | Separate domain decisions from transport; make retries, cancellation, and error translation explicit. |
| Third-party service | Isolate the external contract behind an owned adapter; distinguish simulated responses from verified provider behavior. |

Choose an adapter only for a concrete production, compatibility, or verification need. A test
substitute justifies a seam when it exercises the real contract, rather than merely confirming
the fake's own implementation.

Map old behavior to the proposed interface before migrating callers. Preserve coverage of
distinct outcomes and integration guarantees. Replace an old test only after equivalent coverage
exists and removal is within the task's authorization. Keep tests for meaningful internal
algorithms when their contract remains useful. Avoid exposing private internals solely to keep
an obsolete test shape alive.

Return to [the design workflow](../SKILL.md) with dependency owners, migration steps, and limits.
