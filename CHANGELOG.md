# Changelog

What changed in each Oya Browser release: the server, the desktop app, the SDK and the CLI ship together under one version. The same notes are published at [oyabrowser.com/release-notes](https://oyabrowser.com/release-notes).

Add what a change does under **Unreleased** as it lands. `make release` refuses to cut a release with nothing listed there, turns Unreleased into the new version linked to its GitHub release, and puts the same notes on the release and its tag.

## Unreleased

### Fixed

- A replay no longer fails on a popup the recording closed but the next run never showed, such as a notice shown once a session or a cookie banner. Closing it is optional on replay.
- `make release` stopped at its last step: GitHub created the tag with the release, and pushing the annotated tag carrying the notes was refused. The tag is now pushed first and the release is made on it.

## [1.0.131](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.131) · 2026-09-27

### Added

- **Free-text answers in playbooks.** A field the agent wrote itself, such as a comment, the reason for a request or the answer to a question, is no longer replayed word for word. Each replay asks your model for fresh text from the prompt and that run's data. Playbooks list these fields as `answers`, and the Playwright export calls `oya.llm.answer(question, vars)` for them.
- **Export and import playbooks** to move them between environments: `oya.playbooks.export()` and `oya.playbooks.import()` in the SDK, `oya playbooks export|import` in the CLI, and Export and Import buttons in the dashboard. Secrets travel by name only.
- **Replays sign in by themselves.** A replay that meets a login page signs in with the profile's stored credentials and second factor, returns to where the flow was, and carries on. A step that fails because a session expired is tried again after signing in.
- **A whole chat becomes one playbook.** Follow-up messages in the same chat keep recording into the same run, so a playbook saved after three messages has all three messages' steps.
- `oya.llm.answer(question, vars)` in the SDK, and `POST /api/playbooks/answer` on the server.
- `oya playbooks` in the CLI lists saved playbooks.

### Changed

- **A healed replay fixes the playbook.** When a step no longer fits the page and the agent finishes the task, its steps replace the broken ones, so the next replay runs without the model again. There is no draft to promote.
- **Fewer hardcoded values.** A value from the prompt that reappears in a later address (`/search?q=...`), an option picked right after typing (`70450 - CT head`), and a choice the prompt names (a radio button, a checkbox, a category button) now become variables. Radio buttons and checkboxes are named after their group, such as `size` or `conservative`.
- **Cleaner Playwright code.** Form fields are found by their label, and picks from a typeahead are found by the value typed.
- The agent can start a task over without leaving its first attempt in the playbook, and it is told on a follow-up message that earlier messages are already done.
- **Releases carry their notes.** Each release's tag and GitHub release hold what this changelog lists for it, and each version here links to its release. A release with nothing listed under Unreleased is refused.

### Fixed

- **Missing steps when the desktop window was hidden or covered.** Chromium stopped drawing a window nobody could see, so every click and keystroke waited 30 seconds and some timed out and were dropped from the recording. Pages now keep rendering in the background.
- Clicks on a button covered by an ad or a banner now reach the button.
- A click that opened a confirm dialog ("Press OK to proceed") was left out of the recording, so replays answered a dialog nothing had opened.
- Replaying a closed tab closed the wrong tab.
- A radio button's answer was recorded without its label, so replays picked the group's first option.
- A tab opened only to read a page and closed again is no longer recorded.
- Steps saved by a healing agent now get variables like any other recording.

### Website

- A release notes page, a Discord link, and a "Talk to the founders" button on the landing page.

## [1.0.130](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.130) · 2026-09-26

### Fixed

- Fixes across the desktop app, the CLI, the SDK and the fleet manager.

## [1.0.129](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.129) · 2026-09-26

### Changed

- Cleanup of sign-in and MFA challenge handling, with more tests.

## [1.0.128](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.128) · 2026-09-26

### Fixed

- Agent stability fixes, and updated SDK examples.

## [1.0.127](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.127) · 2026-09-25

### Fixed

- Fixes to the desktop app, the CLI and the SDK.

## [1.0.126](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.126) · 2026-09-25

### Added

- OpenRouter as a model provider.
- Routines stored per project on the server.
- Agent onboarding.

## Earlier releases

Every earlier release is a `v1.0.x` tag in the [repository](https://github.com/OyadotAI/oya-browser/tags).
