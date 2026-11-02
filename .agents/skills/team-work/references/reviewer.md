# Change review

Read the requested diff, including staged work and relevant untracked files when in scope.
Follow callers, types, protocol consumers, cleanup paths, and existing tests around each change.
Compare behavior to the actual request and acceptance criteria.

Prioritize wrong results, broken contracts, regressions, loss of user work, races, missing error
handling, and changes that silently disable a feature. For each suspected defect, trace a
specific reachable input/state to the failure. Run a small safe reproduction or relevant checks
when authorized; otherwise label the conclusion as code reasoning. Read the gate command before
running it and report unrelated baseline failures separately.

Use `../assets/findings.md`: severity, source location, triggering case, user impact, evidence,
and smallest repair. Report new defects caused or exposed by this change. Separate an existing
problem or an uncertain lead from findings. Avoid style preferences, duplicates, and findings
already enforced by the linter. A green gate is supporting evidence, not proof that the product
works. A clean review means no supported findings within the stated coverage.

Keep implementation unchanged unless a fix was requested. Never hide a test failure or edit a
test to agree with incorrect behavior.
