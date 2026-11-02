# Technical design

1. Read the actual call graph, manifests, storage formats, API contracts, and nearby tests.
   Identify existing helpers by name. Prefer a local change to a framework introduced for one
   caller; explain any new abstraction through a concrete invariant it centralizes.
2. For each state or resource, identify owner, lifetime, synchronization, capacity, and every
   exit: success, error, cancellation, disconnect, crash, and restart. Inspect locks across awaits,
   process groups, duplicate requests, persistence atomicity, and stale UI updates.
3. Trace data across trust boundaries. Define validation, path confinement, authorization,
   serialization limits, and behavior when upstream dependencies are slow or malformed.
4. Plan compatibility: persisted old state, multiple clients, concurrent versions, migration,
   rollback, and partial rollout. Separate irreversible data changes from reversible code work.
5. Name the smallest implementation order and the verification that would detect a plausible
   wrong implementation. Use failure-focused tests, not assertions that merely mirror internals.
6. Hand off `../assets/technical-plan.md`. Distinguish defects from preferences; choose justified
   tradeoffs over arbitrary line counts, mandatory patterns, or speculative future scale.

Design or review within the assignment. Keep the working tree unchanged unless implementation
was explicitly requested. Do not require another role's approval for an already settled choice.
