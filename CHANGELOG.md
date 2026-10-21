# Changelog

What changed in each Oya Browser release: the server, the desktop app, the SDK and the CLI ship together under one version. The same notes are published at [oyabrowser.com/release-notes](https://oyabrowser.com/release-notes).

Add what a change does under **Unreleased** as it lands. `make release` refuses to cut a release with nothing listed there, turns Unreleased into the new version linked to its GitHub release, and puts the same notes on the release and its tag.

## Unreleased

### Fixed

- The address bar no longer replaces what you are typing when the page changes its own address. On YouTube, Gmail and other sites that update their address as you use them, typing a new address or search used to be wiped mid-word. Your text now stays until you press Enter, pick a suggestion, press Escape or switch tabs, and an address you selected to type over stays selected. Escape with no suggestions open puts the page's address back, selected, as in Chrome. The bookmark star always reflects the page that is open, not the text being typed.

- **Operators who ran a residential gateway (`OYA_RESIDENTIAL_PROXY_URL`): rotate its credentials after upgrading.** A desktop browser, or a cloud browser on a plan without proxy traffic, could be sent the operator's residential proxy credentials when it ran as a persona that a paid cloud browser had used since the server started. It then used the gateway without its traffic being metered. Each connection now gets its own copy of the persona's fingerprint, so a proxy given to one browser never reaches another. A change to a persona's proxy also applies on its next connect instead of after a restart.

- The key `oya login` saved is sent only to the control plane it was saved for. Pointing the CLI or the SDK at another address with `--url`, `OYA_BASE_URL` or `baseUrl` no longer carries that key there; instead they say the saved key is for another control plane and how to log in for this one.

## [1.0.202](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.202) · 2026-10-20

### Fixed

- The Downloads button's Open and Show in Finder act only on the file that was on screen when you clicked, and only once it has been there for half a second, so a page cannot start a download that swaps its own file in under your click.

## [1.0.201](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.201) · 2026-10-20

### Added

- The desktop Downloads button shows each download as it happens: "Downloading" with how far it has got (bytes of the total and a percent, with a bar along the button), then "Saved" with the file's name and Open and Show in Finder. It shows in the window the download started in, never takes focus, and goes back to the plain button a few seconds after you have seen it. A failed or cancelled download says so instead.

### Fixed

- A download that paused and recovered on its own could show as failed on the Downloads page until it finished; only how a download ends decides that now.
- The Downloads page says "Show in Finder" on a Mac, as the toolbar does.

### Fixed

- A flow recording whose browser went away (closed, disconnected or torn down) now ends at its 30-minute limit with the steps recorded so far. The stop used to be retried every 5 seconds for as long as the server ran.

## [1.0.200](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.200) · 2026-10-19

### Added

- The desktop address bar shows the page's zoom, as Chrome does: a "110%" chip beside the star whenever a page is not at actual size, following every zoom, tab switch and change of site. Click it, or press Enter on it, to put the page back to 100%. It is hidden at 100%, on the start page and in the library, and refuses while OyaAI holds the tab.

### Fixed

- The View menu's Zoom In, Zoom Out and Actual Size could zoom a tab OyaAI was driving; they now do nothing there, like the keyboard shortcuts.
- After a short network drop, the desktop no longer overrides the latest decision on who drives: pressing AI Mode just as the connection dropped no longer lands you back in Human Mode, and an agent handed control while the connection was down keeps it. A hold the server forgot, or one that lapsed while offline, is still taken back.

## [1.0.199](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.199) · 2026-10-19

### Fixed

- Pages on the desktop could stop taking clicks, in every tab, until a restart: when your Human Mode hold ran out or its renewal was refused, the browser paused with nobody driving and kept every tab fenced, with nothing on screen saying why. It now takes control back once, as it already did after a reconnect, also when the connection drops partway through. It never takes control from an agent or another person who holds it, never while you are handing back to AI Mode, and a clock running ahead of the server's gets one retake rather than one a second.
- A press on a page fenced for an agent is now noted in `tab-health.json` in the app's data folder: once per control state and shown tab, with who held control and what was stacked over the page (sites only, nothing for private tabs), so the next report of dead clicks can be told apart from a hung page. It stays on your machine.

## [1.0.198](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.198) · 2026-10-17

### Fixed

- A follow-up in a desktop chat on a Gemini model failed with "Your AI provider refused the request (400)" since 1.0.197: what earlier runs read was handed to the model as a tool call it had not made, which Gemini refuses without its thought signature. It now rides on the assistant's side of the conversation instead, which every provider accepts.

## [1.0.197](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.197) · 2026-10-17

### Added

- A finished run's summary on the desktop shows the tokens it used, after the time ("14 steps · 48s · 212K tokens"), with the exact input and output split on hover. A run you stop shows the tokens counted up to its last finished model call. The chat API's answer carries the same figure as `usage: { input, output }`; the SDK's `ask()` still returns the text, and the raw answer carries it.

### Changed

- Gemini thinking models' thought tokens now count as output tokens, as Google bills them. Runs on a hosted Gemini model are billed for those tokens from this release, and the per-run figure includes them; they were left out before.

### Fixed

- A follow-up in a desktop chat can use what the earlier runs read, even a run that was stopped, hit its step limit or failed. Asking "what did you find?" after stopping a research run used to get "0 candidates were reviewed", because only what was said in the chat carried over. Kept in the server's memory for the tab's chat, capped, and started over when the chat is cleared.
- A file downloaded from a page the agent drives now appears on the Downloads page, with Show in folder, once you pick where to save it in Save As. It used to be saved wherever you chose without being listed.
- Cap ready and desktop-pairing poll sleeps to the remaining timeout, and check once more after the deadline, so a short `readyTimeoutMs` is not overshot by a full poll interval and a browser that dials in during the last pause is not reported as a 504.

## [1.0.196](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.196) · 2026-10-17

### Changed

- The console follows your system's light or dark appearance until you pick one with the sun/moon button, and switches live when the system does. It used to open dark every time.
- Light mode reads correctly everywhere: text selection, focus rings, the health dot, error messages and the "Saved" notes now use the theme's colours instead of ones fixed for dark.

## [1.0.195](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.195) · 2026-10-16

### Fixed

- A ChatGPT account that cannot connect is told so in plain words (Oya needs a ChatGPT Plus or Pro plan; sign in with an account that has one) instead of OpenAI's error code, and a passing OpenAI problem says to try again rather than to upgrade.

## [1.0.194](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.194) · 2026-10-14

### Fixed

- Tabs drag into another window in AI Mode again. With no agent running, the shown tab in each window was marked as the agent's, so a one-tab window, or the tab just clicked, could not be dragged.

### Fixed

- Playbook replay now re-finds a recorded control and retries when its native agent frame detaches before the action runs.

## [1.0.193](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.193) · 2026-10-14

### Fixed

- Saving a playbook from an agent run now removes unsupported tab and reload commands that could trigger avoidable replay healing.

## [1.0.192](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.192) · 2026-10-14

### Fixed

- The active tab's close button works again when many tabs are open. The tab's title covered it, so a click only selected the tab.
- Sites that open a blank window and then load their own page into it, such as Rippling's course player, get that window. They used to see "your browser is blocking popups", and a tab was left on about:blank.

## [1.0.191](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.191) · 2026-10-14

### Changed

- Maintenance and fixes.

## [1.0.190](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.190) · 2026-10-14

### Changed

- Maintenance and fixes.

## [1.0.189](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.189) · 2026-10-14

### Changed

- Maintenance and fixes.

## [1.0.188](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.188) · 2026-10-14

### Changed

- Maintenance and fixes.

## [1.0.187](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.187) · 2026-10-14

### Fixed

- Sign in with ChatGPT keeps this install's ChatGPT registration, as OpenAI requires, so trying again after `invalid_grant` works instead of failing the same way every time. The same fix applies to signing in from a self-hosted server.

## [1.0.186](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.186) · 2026-10-14

### Fixed

- On the desktop app, a run started from the SDK or the API now gives its tab back as soon as it ends. The next run works in the tab you are watching, instead of a hidden background tab for the first two minutes.

## [1.0.185](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.185) · 2026-10-13

### Changed

- Maintenance and fixes.

## [1.0.184](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.184) · 2026-10-13

### Changed

- Maintenance and fixes.

## [1.0.183](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.183) · 2026-10-13

### Fixed

- Request macOS Screen Recording access before refusing screen sharing, with a direct link to the correct System Settings pane and instructions for missing app entries.
- Generate real screen-sharing previews for background and restored tabs, including tabs that have never been shown. Wait for the first rendered frame and display a clear fallback when a preview cannot be captured.
- Keep desktop capture choices consistent with macOS permissions and prevent a picker shutdown crash when its requesting window closes.

## [1.0.182](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.182) · 2026-10-13

### Fixed

- Keep agent highlights aligned during page movement, clip them to visible controls and scaled frames, and discard stale highlights after navigation or element replacement.
- Recognize custom app controls containing decorative test markers, highlight styled radio and checkbox labels, and correctly distinguish iframe contents from covering overlays. Avoid a highlight cleanup crash during window shutdown.
- Keep visible disabled toolbar controls, such as iCloud Mail’s Reply and Trash, in the highlight scan with muted dashed outlines.

- Links can open installed desktop apps, including Docker, Slack and Zoom, after confirmation. Human-owned tabs work in AI Mode and separate windows; callback parameters are preserved.

## [1.0.181](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.181) · 2026-10-13

### Added

- Separate, searchable **Bookmarks** and **History** tabs, with compact adjacent buttons on the bookmarks row, your browser theme, website filters, sorting, date sections for history, and clear save/remove actions.
- A screen-sharing picker with Oya tabs, windows and entire screens. Every share requires an explicit source selection; Cancel and Escape share nothing. Sharing is video only.

- Received Keel’s anonymous daily step reports at `/api/telemetry/keel/ping`, with validated fields, rate limiting, and one stored report per installation and usage day.

### Fixed

- Settings now separates General, Logins & sync, and Account, with expandable import history and appearance controls in General. Compact “AI Mode” and “Assistant” labels free space in the toolbar.
- Camera and microphone consent is remembered per site and profile, including after an operating-system permission refusal. Screen sharing now reaches the source picker correctly.
- Human-owned tabs can be moved while agents work in their own tabs. Detached windows wait for their toolbar and skip the app-start animation.
- Open tabs restore after their profile's cookies are ready, without the startup page overwriting the saved session. Importing or pairing no longer silently switches an established profile.
- Plan limits now show a readable account notice with a billing button instead of a raw error paragraph. Oya's upcoming-invoice preview omits unused zero-quantity, zero-value meters while preserving charges, credits and included usage.

## [1.0.180](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.180) · 2026-10-12

### Changed

- Maintenance and fixes.

## [1.0.179](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.179) · 2026-10-12

### Added

- **Check for updates** button at the bottom of Settings, next to the version. It reports the result, and restarts into an update once one is downloaded.

### Fixed

- **Playwright works with cloud browsers again.** Since 1.0.178, `chromium.connectOverCDP(browser.cdpUrl)` failed at connect (`Target.setAutoAttach`). Page sessions now pass the standard CDP methods the native layer has no answer for straight to their own tab, so `goto` returns its Response, and `fill`, `check`, `click`, `waitForLoadState('networkidle')`, `screenshot({ fullPage })`, `newPage` and `context.clearCookies()` all work. The front door still refuses persona overrides (user agent, timezone, locale), non-web addresses, addresses the egress rules block, remote file uploads and browser-wide methods, and keeps tab ownership and Human Mode. `OYA_CDP_PASSTHROUGH=0` turns the passthrough off.

## [1.0.178](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.178) · 2026-10-12

### Added

- **Connect your coding agent.** The desktop app now runs an MCP server on your own computer (`127.0.0.1:47321`, protected by a token). The footer shows Claude Code, Codex, Cursor and VS Code when they are installed; one click adds Oya to them, and they drive this browser directly. Claude Code and Codex are also set to run Oya's tools without asking; Cursor and VS Code ask once. Human Mode pauses them like any agent.
- **Choose an account.** Logging out keeps the account on this Mac (sealed in the keychain), and the welcome screen offers it again: one click signs back in. Remove forgets it.
- **Touch ID passkeys** on macOS: create a passkey in Oya and sign in with your fingerprint. Passkeys kept in iCloud or on your phone are not supported yet.
- Privacy Policy and Terms of Service at oyabrowser.com/privacy and /terms, linked from the footer and sign-up.

### Changed

- Agents work only in their own tab: the tab you asked from and the tabs they open. They no longer move into your other tabs, and they close only tabs they opened themselves.
- Importing another browser's logins asks before switching you to its profile, instead of switching silently.
- Pages are read in TOON by default (fewer tokens for the same accuracy); markdown and JSONL stay available.
- Profile and model are now dropdowns under the Ask box, and Clear is **New chat**. The Ask shortcut closes the panel when Ask is already open.

### Fixed

- **Connect desktop** on oyabrowser.com works from inside Oya, as it does from Chrome.
- Sign in with ChatGPT no longer fails with `invalid_grant`: it returns to a fixed local port and registers afresh each time.

## [1.0.177](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.177) · 2026-10-11

### Changed

- Who drives the browser is now one switch in the toolbar: **OyaAI Mode** while the agent works, **Human Mode** while you do. Click the other side to hand over; after a pause, OyaAI Mode picks up where the agent stopped.
- The Oya button is now **Oya Assistant**.

### Fixed

- While control was being handed back, the agent's next step could find the page still yours and leave it uncovered.

## [1.0.176](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.176) · 2026-10-11

### Fixed

- Sign in with ChatGPT works when Oya is already your default browser: the sign-in opens in your usual browser (where you are signed in to ChatGPT) instead of looping back into Oya, and Oya comes back to the front when it is done.
- Trying again after a setup step fails no longer sends you through the ChatGPT sign-in a second time.
- Links from other apps open in a new tab while agents work in their own tabs, instead of being refused.
- A tab an agent opens without an address shows the Oya start page instead of a blank page.

## [1.0.175](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.175) · 2026-10-11

### Fixed

- The desktop app no longer crashes on sites whose service worker uploads files, such as claude.ai.
- Websites can use your camera and microphone again after you allow them, also while agents work in other tabs, and the approval lasts until the page reloads or you leave it.

## [1.0.174](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.174) · 2026-10-11

### Changed

- Maintenance and fixes.

## [1.0.173](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.173) · 2026-10-11

### Added

- Set up in under a minute: the desktop app's first run is a short checklist. Sign in with ChatGPT, bring in your logins from your usual browser, make Oya your default browser, and start. No API key and no account to create.
- Oya's agent runs on your ChatGPT Plus or Pro plan. The sign-in stays on your computer; the agent's model calls go to OpenAI from there, never through Oya's servers.
- A browser set up without an account gets a free daily allowance of agent steps; past it, Ask offers a free Oya account (500 steps a month, cloud browsers and routines), and the browser keeps everything it had.
- Connect Oya from claude.ai, Claude Desktop or Claude mobile: add `https://oyabrowser.com/mcp/pool` as a custom connector, sign in, and pick a project. No API key.
- Self-hosted servers can sign in with ChatGPT themselves, for the agent and the console.

### Fixed

- Importing logins from Safari or Chrome no longer fails as a whole over a few cookies the browser cannot store, and a failed switch reopens your tabs instead of leaving an empty window.
- Restarting after an import no longer signs the browser out ("Not connected to server").
- A new tab no longer shows the previous page beneath it; the panel stays open on pages the agent opens, and a link the agent's page opens stays marked as the agent's.
- Signing out closes the agent panel.
- The agent knows today's date, asks at most once before it starts, and treats a budget as a ceiling.

## [1.0.172](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.172) · 2026-10-11

### Changed

- Maintenance and fixes.

## [1.0.171](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.171) · 2026-10-11

### Changed

- Maintenance and fixes.

## [1.0.170](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.170) · 2026-10-11

### Changed

- Maintenance and fixes.

## [1.0.169](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.169) · 2026-10-11

### Changed

- Maintenance and fixes.

## [1.0.168](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.168) · 2026-10-11

### Changed

- Maintenance and fixes.

## [1.0.167](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.167) · 2026-10-11

### Changed

- Maintenance and fixes.

## [1.0.166](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.166) · 2026-10-10

### Added

- Require an explicitly selected native Oya runtime during desktop packaging, with per-architecture capability checks before unpacking. Missing native engines fail rather than silently shipping stock Electron.

- Multiple full browser windows with live tab tear-out and transfers onto another window's tab strip, preserving page state and navigation history. Added Cmd/Ctrl+N, Cmd/Ctrl+Alt+W, and Move Tab to New Window in the tab menu. Window-scoped controls and independent shields retain agent access through globally unique tab ids and `list_tabs.window_id`.

### Fixed

- Keep the agent panel interactive after startup or shell reload, including its close button.
- Show a floating page preview during tab tear-out and prepare destination tabs and toolbars before revealing new windows, preserving the live source page if preparation fails.
- Hand completed Gmail and Calendar sign-ins back to browser tabs instead of leaving the app inside an authentication dialog.

- Stop auto-cancelling desktop WebAuthn requests; add native account selection and provisioning-bound macOS Touch ID setup. Native-capability packaging now requires an explicit Apple provisioning profile instead of producing an app macOS rejects. Existing iCloud passkeys, phone/QR authentication and security-key PIN entry are not supported by the current Electron integration; real-device verification is still required.

## [1.0.165](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.165) · 2026-10-08

### Added

- Agent-loop and MCP notification tools read the toolbar inbox, mark selected entries read, and dismiss a specific notification with explicit confirmation.

- Add a toolbar notification inbox for non-blocking page alerts, unread counts, read/clear controls, and a keyboard shortcut. Confirmations and prompts stay explicit.

- Address-bar autocomplete from the current profile's local history and bookmarks, with prefix ranking, deduplication, arrow-key selection, Enter to open, and Escape to dismiss. Typed queries are not sent to a remote suggestion provider.

### Fixed

- Add explicit microphone/camera permission prompts and macOS capture declarations; restore native WebRTC for direct unmanaged browsing while preserving proxy/managed leak protection.

- Zoom meeting app links now offer an explicit Open Zoom handoff from page navigation, redirects, popups, and the address bar, without replacing the meeting page.

- Fix Windows installer compilation by correctly escaping quotes in the default-browser registration commands, preserving executable paths with spaces and URL arguments.

## [1.0.164](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.164) · 2026-10-08

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
