# Workflow studio

The desktop recorder now keeps a local draft separately from a published playbook. **Finish recording** reveals the name field and **Save playbook** action immediately. The draft remains recoverable if publishing fails or the browser is offline. Publishing requires a server that supports schema version 2.

## Operator flow

1. Take control and start recording. Only trusted interactions with visible controls are captured. Passwords, one-time codes, and fields with credential names become secret placeholders. Recording pauses when desktop control is handed away.
2. Finish recording. The save prompt distinguishes the encrypted local draft from the version saved to Oya. Name the playbook and save it, or review the steps first.
3. Select a step to edit its target, value, frame path, timeout, breakpoint, or enabled state. Reorder, duplicate, delete, undo, and redo are available. A native target picker avoids activating the selected control. Add explicit assertions to check outcomes.
4. Validate in fresh tabs using the current login session. The confirmation explains that actions affect real websites. Pause happens at a step boundary; Continue and Step retain the running browser context. Stop terminates validation, without undoing website effects.
5. Review the run timeline and generated Playwright module. Completing actions without assertions is described as **steps completed**, not proof that the business outcome is correct.

## Execution and repair

The shared schema and generator live in `browser/scripts/workflow.cjs`. Desktop validation imports the same generated module that is exported. Playwright runs in an Electron utility process and connects to the installed Chromium; customers do not need Node or a separate browser installation. New exports use strict locators and never silently select the first matching element. Unsupported interactions and unidentified frames block validation until reviewed.

Each run uses fresh tabs. The authenticated run proxy filters unrelated targets and retains the normal human/agent control gate. Chromium uses an ephemeral loopback debugging endpoint when no explicit debugging port is configured. This is not a security boundary against other privileged local processes.

Automatic repair only considers already recorded alternative locators before input has been dispatched. It requires a unique match, permits at most two repairs within a 30-second repair window, and never changes assertion expectations, submitted values, or the remaining task. A repair creates a separate encrypted draft; the original and published playbook remain unchanged. Assertion failures are not automatically repaired. A potentially dispatched input failure is reported as **outcome unknown**.

Version 2 publications preserve stable step IDs, candidates, frame paths, assertions, variables, and timeouts. Server playback dispatches the shared Playwright workflow to the desktop. Workflows containing human checkpoints must be run interactively in the desktop workspace. Legacy playbooks retain their existing runner and export behavior.

## Storage and diagnostics

Drafts use AES-256-GCM with an OS-encrypted key, private file permissions, an fsynced temporary file, and atomic replacement. There is no plaintext fallback. If OS secure storage is unavailable, the workspace reports that the draft is only in memory. Secret variable defaults are removed before persistence/export.

Run records are encrypted separately. Active records found after restart become interrupted, with no automatic replay. Retention is seven days, at most 30 runs, and at most 250 MB. Drafts are not automatically evicted. Network diagnostics exclude bodies and headers; console messages and raw Playwright error logs are omitted. Support exports show a preview of the same redacted object written to disk. Screenshots are omitted when safe masking cannot be guaranteed.

## Verification and limits

- `npm test --prefix browser`: model, encryption/corruption, editing, redaction, existing browser/control regressions.
- `npm run test:recording --prefix browser`: trusted input, secrets, hidden/synthetic events, labels, native controls, navigation, frames, stop and clear.
- `npm run test:workflow --prefix browser`: three consecutive real Playwright runs, assertions, multi-tab isolation, bounded repair, stepping, resume and stop.
- `npm run test:shell --prefix browser`: real record/finish/review/validate flow, prominent save prompt, failed publication, export, themes, native menus, keyboard use and four layouts. `OYA_TEST_EXECUTABLE` runs this against a packaged app.
- `npm run test:unit --prefix server`: the version-2 round trip, its caps and secret defaults
  (`tests/unit/modules/playbooks/sanitize.test.ts`), and server dispatch and replay
  (`tests/unit/modules/playbooks/replay.test.ts`, `catalog.test.ts`). The harness this line
  used to name, `server/test-workflow-v2.js`, no longer exists; these cover what it covered.

macOS development and an unsigned packaged arm64 app were exercised. Windows and Linux runtime behavior still need release-platform testing. Third-party portals, cross-process frames, native file dialogs, downloads, and popup-heavy workflows need customer-specific coverage. A frame that cannot be identified is surfaced for manual correction. Drag/drop and canvas interactions are explicitly unsupported rather than exported as if successfully captured. Failure diagnostics preserve metadata and the tab, but do not provide a DOM/screenshot time-travel trace or AI-generated replacement locators.
