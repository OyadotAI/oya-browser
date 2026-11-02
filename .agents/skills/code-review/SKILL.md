---
name: code-review
description: "This skill should be used when reviewing a branch, pull request, working tree, or changes since a commit for repository standards and the user's requested behavior."
---

# Review standards and requested behavior

Review two distinct questions: whether the change follows the project's documented standards,
and whether it implements the requested behavior. Perform both passes within the assigned role.
Delegate only when explicitly authorized and the assigned tools support delegation. Return the
report to the coordinator; remote comments or issue publication require existing authorization.

## Establish scope

Read the assignment and identify the requested files or comparison. For branch work, resolve
the supplied ref and inspect the appropriate merge-base diff and commit list. For an exact
commit comparison, use the requested endpoints. For work in progress, inspect unstaged and
staged changes and relevant untracked files; a committed-only diff misses that assignment.
Name the range reviewed, and report invalid refs or empty scope plainly.

Read applicable repository instructions, surrounding code, callers, and tests. Include
CONTRIBUTING.md and CODING_STANDARDS.md when present. Recover acceptance criteria from the
user's request, supplied spec, or relevant local issue material. Read linked remote sources
through Oya Browser when access is authorized. Use the current request as the spec when no
formal document exists. State missing requirements as coverage limits rather than inventing
them or requiring tracker setup.

## Standards pass

Apply documented rules in their proper scope and account for established exceptions. Trace
correctness, ownership, compatibility, failure handling, and concurrency through real callers.
Skip mechanical nits already enforced by tooling. Use
[references/design-signals.md](references/design-signals.md) only to guide investigation of
maintainability; a code smell alone does not establish a defect.

Support each blocking finding with a reachable case or a concrete documented requirement,
source location, impact, and a small repair. Keep optional maintainability advice separate and
include it only when requested. Respect a correctness-only review's scope.

## Spec pass

Map each requirement to implementation and evidence. Look for missing or partial behavior,
incorrect interpretation, scope growth, and apparent success that fails on cancellation,
retry, disconnect, restart, or duplicate input. Inspect affected callers beyond the edited hunk.
Distinguish a demonstrated defect from an unresolved question and from a preexisting issue.

## Report

Lead with the verdict and covered scope. Use separate **Standards** and **Spec** sections, each
with its result, supported findings, and coverage limits. For each finding give severity,
file/line, triggering input or state, expected and actual behavior, evidence, and smallest repair.
Cross-reference a shared underlying defect instead of counting it twice. An empty findings list
is valid; do not manufacture findings to fill a section.

List exact checks executed and their results. A passing check supports only the behavior it
exercises. Write or run tests only within the user's authorization. Preserve implementation
unless a repair was assigned. Use the team's findings asset when acting as reviewer; fit these
two passes into it without duplicating the report.

Adapted from Matt Pocock's engineering skills; see [SOURCE.md](SOURCE.md) and [LICENSE](LICENSE).
