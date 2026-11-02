# Compare plausible interfaces

State the problem, current caller, invariants, dependencies, and compatibility constraints.
For a substantial choice, sketch two or three meaningfully different interfaces:

1. Minimize facts and entry points callers must learn.
2. Make the common caller's operation direct.
3. Where actual variation exists, isolate the varying dependency.

Produce these sketches in the current assignment. Delegate only when explicitly authorized and
supported by the assigned tools. Each sketch needs a usage example, failure contract, ownership,
hidden complexity, and migration cost; keep examples small and illustrative.

Compare caller leverage, maintenance locality, dependency placement, and compatibility. Recommend
one design and give the tradeoff that could change that recommendation. Resolve routine choices
from the existing request; request missing user decisions only when they materially affect scope.

Return to [the design workflow](../SKILL.md) with the selected interface and rationale.
