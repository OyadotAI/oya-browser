---
name: codebase-design
description: "This skill should be used when designing a module interface, reducing coupling, choosing dependency seams, improving testability, or comparing architecture alternatives."
---

# Codebase design

Design substantial behavior behind a small, understandable interface. Follow the project's
instructions, glossary, and existing primitives. Preserve the assignment's scope and authority:
a design assignment produces a proposal; implementation requires an implementation assignment.

## Trace the actual design

Read the relevant callers, implementation, tests, and architectural decisions. Identify what
each caller must know: types, invariants, ordering, errors, required configuration, resource
ownership, and performance guarantees. Treat all of these as part of the interface. A simple
signature that hides undocumented ordering requirements still has a complex interface.

Describe a concrete caller journey and where knowledge is repeated. Finish this step with
source locations and an inventory of the facts callers currently carry.

## Find useful depth

Evaluate capability per fact a caller must learn, rather than implementation line counts.
Concentrate decisions and failure handling where a fix can apply to all callers. Use the
deletion thought experiment: if removing an abstraction spreads complexity across callers,
it earns its place; if the complexity disappears, investigate whether it is only forwarding.
Retain forwarding required for compatibility, policy, or ownership when those purposes are real.

Distinguish the public interface from internal composition. Keep internal helpers private when
callers have no reason to know them. Use the project's terms; introduce new vocabulary only
when it makes a concrete distinction clearer.

## Place dependencies deliberately

A seam is a place to change behavior without changing its caller; an adapter supplies behavior
there. Justify a seam with actual variation, such as production I/O and a realistic test adapter.
Keep pure calculations separate from effects where useful. Accept dependencies at an ownership
boundary instead of constructing hidden global resources, without adding interfaces to every
function mechanically.

For consolidation or dependencies across storage and network layers, read
[references/deepening.md](references/deepening.md). For a consequential design choice with
credible alternatives, read [references/alternatives.md](references/alternatives.md).

## Return a bounded design

Provide the proposed interface, one representative caller, hidden behavior, dependency owners,
failure paths, compatibility impact, and migration order. Explain which existing primitives
are reused. Account for cancellation, cleanup, concurrency, and rollback where relevant.

Describe verification through the interface that callers actually use. Preserve distinct
behavioral coverage during a refactor; identify any adapter or integration behavior a fake
cannot prove. Run or write tests only within the user's authorization. Distinguish code-based
reasoning from executed evidence, and state what remains uncertain.

Complete when the recommendation explains how callers become simpler, where maintenance
concentrates, and how the proposed contract can be checked. Use the team technical-plan asset
when assigned as principal. Keep the proposal proportional to the change.

Adapted from Matt Pocock's engineering skills; see [SOURCE.md](SOURCE.md) and [LICENSE](LICENSE).
