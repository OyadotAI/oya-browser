# Changelog

What changed in each Oya Browser release: the server, the desktop app, the SDK and the CLI ship together under one version. The same notes are published at [oyabrowser.com/release-notes](https://oyabrowser.com/release-notes).

Add what a change does under **Unreleased** as it lands. `make release` refuses to cut a release with nothing listed there, turns Unreleased into the new version linked to its GitHub release, and puts the same notes on the release and its tag.

## Unreleased

### Added

- Profile-local browsing history and bookmarks, with agent-loop and MCP tools for searching, saving, removing, clearing, and reopening recent tabs.
- Keyboard-first navigation and workspace commands, a persistent shortcuts footer, and a searchable platform-aware guide. Agents can read the same live bindings with `list_keyboard_shortcuts`.
- Explicit Make Oya Browser default options in welcome, Commands, and Help, with OS registration and external HTTP/HTTPS link handling.

### Fixed

- Closing a tab returns to the most recently used tab; new tabs focus the address bar.
- Google sign-in popups that finish at Gmail hand off to a normal protected tab only after the mailbox loads successfully.
- Browser identity setup without a persona now applies the same native automation setting as the persona path and reports setup failures.

### Known limitations

- The Gmail sign-in changes have passed automated checks, but successful Google sign-in still requires verification in the installed release. They do not guarantee Google will accept the browser or add native passkey support.

## [1.0.163](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.163) · 2026-10-08

### Added

- In-browser playbook library with search, run inputs, progress, rename, delete, and JSON import/export.
- First-login invitation to import site sessions from the default supported browser.
- Takeover confirmation when a person clicks, types, or deliberately moves the pointer while an agent controls the page.
- Admin grants for extra agent steps and restoration of consumed step allowance, with retry protection and usage history preserved.
- Continue an unfinished task after its per-run step limit without clearing the conversation.

### Changed

- Refined white browser surfaces and made model settings, custom endpoints, profiles, routines, and imports easier to find.
- Saved playbooks open directly from Ask and Record confirmations.

### Fixed

- Windows browser import discovers Chrome and Edge across per-user and system installations and reports launch, timeout, and partial-import failures.
- Playbook runs retain the single-run guard when their pane closes and restore prior human control on completion.
- Complimentary plan upgrades retain the Free hosted AI credit allowance; additional credits remain available through admin grants.
- Admin plan saves verify the stored override before reporting success.

## [1.0.162](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.162) · 2026-10-07

### Fixed

- Hosted deployment passes the configured database connection explicitly to the admin billing migration, preventing psql from connecting to a local socket.

## [1.0.161](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.161) · 2026-10-07

### Added

- Admin customer controls to override plan access and grant extra cloud hours or hosted AI dollar credits, with reasons, grant history, and retry protection.
- Period grants extend usage limits and reduce usage not yet reported to Stripe. Access overrides remain separate from paid subscriptions and their charges.

### Fixed

- Customer billing shows support-granted access and offers Stripe management only when a billing customer exists.
- Delayed admin searches and adjustment refreshes cannot replace a newer customer selection.

## [1.0.160](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.160) · 2026-10-06

### Changed

- Maintenance and fixes.

## [1.0.159](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.159) · 2026-10-06

### Changed

- Maintenance and fixes.

## [1.0.158](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.158) · 2026-10-05

### Changed

- Maintenance and fixes.

## [1.0.157](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.157) · 2026-10-04

### Changed

- Maintenance and fixes.

## [1.0.156](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.156) · 2026-10-04

### Changed

- Maintenance and fixes.

## [1.0.155](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.155) · 2026-10-04

### Changed

- API keys in WebSocket URLs (`/connect?token=`, `/ws?key=`) are refused. Send `Authorization: Bearer <key>`, or use the one-use `cdpUrl` from `GET /api/browsers/:id`. `OYA_ALLOW_LEGACY_QUERY_KEYS=true` turns the old form back on for a self-hosted server.
- Console sign-ins end after 24 hours without use (`OYA_SESSION_IDLE_HOURS`), and signing out ends the session everywhere.
- Audit events are kept at least 365 days (`OYA_AUDIT_RETENTION_FLOOR_DAYS`); a shorter project setting is raised to it.
- Every dependency updated to its latest version.

### Added

- Two-factor sign-in with an authenticator app, under Account > Two-factor authentication. `oya login` asks for the code.
- API keys can expire after 30, 90 or 365 days.
- A project can choose which model providers may see its pages.
- `DELETE /api/auth/me` deletes an account, and a deleted project's data is purged after a day.

### Security

- Share links reach only their own session, on every route.
- Recordings are encrypted at rest.
- The audit trail records which credential acted, is signed, and can be checked at `GET /api/operator/audit/verify`.
- Blocked private-network addresses written in IPv6 form.

## [1.0.154](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.154) · 2026-10-03

### Changed

- Maintenance and fixes.

## [1.0.153](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.153) · 2026-10-03

### Changed

- Maintenance and fixes.

## [1.0.152](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.152) · 2026-10-03

### Changed

- Maintenance and fixes.

## [1.0.151](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.151) · 2026-10-03

### Changed

- Maintenance and fixes.

## [1.0.150](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.150) · 2026-10-02

### Changed

- Maintenance and fixes.

## [1.0.149](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.149) · 2026-10-02

### Changed

- Maintenance and fixes.

## [1.0.148](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.148) · 2026-10-02

### Changed

- Maintenance and fixes.

## [1.0.147](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.147) · 2026-10-02

### Changed

- Maintenance and fixes.

## [1.0.146](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.146) · 2026-10-02

### Changed

- Maintenance and fixes.

## [1.0.145](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.145) · 2026-10-01

### Changed

- Maintenance and fixes.

## [1.0.144](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.144) · 2026-09-30

### Changed

- Maintenance and fixes.

## [1.0.143](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.143) · 2026-09-30

### Changed

- Maintenance and fixes.

## [1.0.142](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.142) · 2026-09-29

### Changed

- Maintenance and fixes.

## [1.0.141](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.141) · 2026-09-29

### Changed

- Maintenance and fixes.

## [1.0.140](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.140) · 2026-09-29

### Changed

- Maintenance and fixes.

## [1.0.139](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.139) · 2026-09-27

### Changed

- Maintenance and fixes.

## [1.0.138](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.138) · 2026-09-27

### Changed

- Maintenance and fixes.

## [1.0.137](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.137) · 2026-09-27

### Changed

- Maintenance and fixes.

## [1.0.136](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.136) · 2026-09-27

### Changed

- **Coverage and status badges on the README.** CI now uploads the server's and the UI's test coverage to Codecov (an unreachable Codecov never fails the build), and the README shows build status, coverage, the latest release, npm downloads, bundled types, the supported Node version and GitHub stars.
- A regression test now holds the prod workflow, not `make release`, to moving `DAYTONA_SNAPSHOT`, and checks that a failed snapshot registration keeps the one the cluster already runs.

## [1.0.135](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.135) · 2026-09-27

### Fixed

- The macOS build in CI failed at signing: electron-builder's own temporary keychain could not be unlocked on GitHub's runner. The workflow now imports the certificate into a keychain of its own.

### Changed

- **`make release` returns as soon as the release is published.** It used to wait for CI to register the cloud-browser snapshot and then update the `DAYTONA_SNAPSHOT` secret from the releaser's machine. The prod workflow now does it all: a release deploys its own snapshot, a failed registration keeps the snapshot prod already runs, dev follows prod's snapshot, and a manual redeploy keeps it by default.

## [1.0.134](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.134) · 2026-09-27

### Fixed

- **Replays sign in through every stage of a login.** A username page, then a password page, then a choice of second factor, then the code: a replay used to stop after the first and carry on half signed in. It now works through them all and goes back to the flow's page only if the next step needs it.

### Changed

- **The macOS app is built in CI.** It is signed and notarized by GitHub Actions on the release tag, like Windows and Linux, so `make release` no longer builds it on the releaser's Mac. `make release-local-mac` keeps the old way as a fallback.

## [1.0.133](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.133) · 2026-09-27

### Fixed

- A replay test could hang CI when run with coverage, which held the 1.0.132 deploy at its tests.

### Changed

- **Faster CI.** The server's unit tests run once, with coverage, instead of twice; the test job is split into parallel jobs for the server, the SDK and CLI, and the desktop app; installs are cached for every part; and a release commit no longer runs the whole suite a second time on its way to dev, since its tag runs it for prod. A release's images reuse the layers dev already built, rather than starting from an empty cache.

## [1.0.132](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.132) · 2026-09-27

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
