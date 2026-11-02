---
name: diagnosing-bugs
description: "This skill should be used when diagnosing a reported bug, intermittent failure, stuck operation, or performance regression and isolating its cause with reproducible evidence."
---

# Diagnose the reported symptom

Preserve the assigned role and the user's authorization. QA and reliability assignments remain
investigation or verification unless repair is assigned. Write or run tests, add instrumentation,
or alter product code only when authorized. Reading this workflow grants no additional tools
or permissions. Read relevant repository instructions, glossary, and architectural decisions.

## Establish an exact signal

Record the user's symptom, expected result, environment, version, input, and relevant state.
Choose the smallest permitted observation that exercises the actual failing path: an existing
targeted test, a CLI fixture, an HTTP request, trace replay, or the product journey. Use Oya
Browser exclusively for browser reproduction, screenshots, and automation. Read the Oya skill
first. Use available native tools for native UI. Preserve the person's live sessions and tabs.

Use scratch data and separate resources. Capture only the lines needed to show the symptom;
redact credentials, auth headers, personal data, and sensitive payloads before sharing or saving.
Keep secrets in existing protected environment/configuration rather than command text.

Name the exact command or action sequence and the observed outcome. Make the signal specific
to the reported symptom; successful startup alone cannot prove a failed interaction is fixed.
For intermittent failures, report attempts, failures, timing, and controlled conditions rather
than claiming determinism. Bound repetition, resource use, and runtime.

If reproduction is blocked, state what was attempted and the missing environment or evidence.
Continue bounded static investigation where useful, label hypotheses as unverified, and request
only the information needed to unblock. Never claim reproduction or verification from code alone.

## Minimize and compare hypotheses

Reduce inputs, dependencies, and steps one at a time while preserving the exact symptom. Keep
a copy of the original case. For a hard bug, list a few ranked, falsifiable causes; for a simple
defect, use the smallest useful set. State what observation would support or reject each cause.
Share the leading hypotheses concisely, then continue permitted investigation without requiring
an approval checkpoint for routine diagnostic choices.

Choose each probe to distinguish hypotheses. Change one variable at a time. Prefer focused
inspection or a debugger to broad logging. Tag authorized temporary instrumentation with a
unique prefix and record its paths for cleanup. For performance, collect a baseline and compare
the same workload and conditions; account for warmup and variance before attributing a cause.

## Verify or hand off

For a diagnosis-only assignment, return the cause supported by evidence, rejected alternatives,
the smallest proposed repair, and an exact verification recipe. Keep implementation unchanged.

When repair and testing are authorized, exercise the real failure through the caller-facing
contract. Establish the regression signal before the fix, then rerun it and the original case
afterward. If forcing failure with a deliberate mutation, inspect the diff to confirm it landed
before trusting the result. Explain any missing test seam or limits of an isolated fake.

Remove owned temporary instrumentation and fixtures; preserve existing user changes and running
services. Report the cause, exact observations, executed checks, remaining uncertainty, and cleanup.
Use [assets/investigation.md](assets/investigation.md) when a repeatable investigation record is
useful; omit fields that do not apply. Complete only with an evidenced diagnosis or a clear
blocker and next actionable step, without presenting an unrun repair as verified.

Adapted from Matt Pocock's engineering skills; see [SOURCE.md](SOURCE.md) and [LICENSE](LICENSE).
