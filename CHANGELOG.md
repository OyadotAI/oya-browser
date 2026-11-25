# Changelog

What changed in each Oya Browser release: the server, the desktop app, the SDK and the CLI ship together under one version. The same notes are published at [oyabrowser.com/release-notes](https://oyabrowser.com/release-notes).

Add what a change does under **Unreleased** as it lands. `make release` refuses to cut a release with nothing listed there, turns Unreleased into the new version linked to its GitHub release, and puts the same notes on the release and its tag.

## Unreleased

### Added

- Added optional macOS on-screen window selection with a separate Oya confirmation before capture. The existing window list remains available, and closed windows or changed requesting tabs cannot inherit a pending choice.

## [1.0.295](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.295) · 2026-11-25

### Changed

- claude.ai and ChatGPT connectors on `/mcp/pool` now work like the desktop app's MCP: ten tools, and one cloud browser that starts itself the first time a page is opened. There is no start, stop or pool tool to call. Only six tools act on a page, so there are fewer approvals to give. API keys keep the full pool tool set, unchanged.
- A connector keeps its own browser, apart from other connectors and API clients in the same project, and finds it again after a deploy or a dropped connection.
- Connecting an app to Oya is one Allow click when you have one project. With several, the last project you connected is preselected. A new account gets its first project on that screen.

### Fixed

- The OAuth server no longer advertises refresh tokens it does not issue.
- An expired connection request has its own screen, and a failed one can be tried again.

### Documentation

- Added ChatGPT setup steps. The tool counts in the docs and llms.txt are checked against the code.

## [1.0.294](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.294) · 2026-11-24

### Fixed

- Fixed the release check for native desktop clicks. It now reads the page and uses the returned button reference to verify input before, during, and after human takeover.

## [1.0.293](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.293) · 2026-11-24

### Documentation

- Added a prompt users can paste into Claude Code or Codex to install Oya Browser on macOS or Windows and connect the desktop app to their agent.

## [1.0.292](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.292) · 2026-11-24

### Documentation

- Added desktop and cloud installation steps for Claude Code and Codex, including a prompt to verify the connection.

## [1.0.291](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.291) · 2026-11-24

### Added

- Remote MCP now selects verified cloud sessions. Desktop browsing uses local MCP.
- Browsing turns can show a fresh live card with a temporary watch-only link. Requested screenshots appear as fixed images in compatible chat hosts.
- Oya Browser is being submitted to the ChatGPT plugin directory: the package lives in plugins/oya-browser-chatgpt, and oyabrowser.com answers OpenAI's domain check at /.well-known/openai-apps-challenge.

## [1.0.290](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.290) · 2026-11-24

### Added

- In Claude, and other chat apps that show MCP Apps, starting a browser now shows a card in the conversation: the browser's page, refreshed every two seconds while you can see it, with its name and address, and a Take over button that opens the live page. The card gets its pictures through the chat app's own connection, so no sign-in token ever reaches it. Every chat app also gets a "Watch or take over" link in the start reply.

## [1.0.289](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.289) · 2026-11-24

### Fixed

- The Claude Code plugin's author is now "OyadotAI", the GitHub organization's name: the Claude directory refused "Oya" and "Oya AI" as too close to other listings' names. Its author link is oyabrowser.com, not getoya.ai.

## [1.0.288](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.288) · 2026-11-24

### Fixed

- Agents' `analyze_page`, `click` and `type` now reach every frame from another site, as Keel's read does: a frame nested inside another (an embedded checkout inside a portal) and two frames from the same site are each read and acted on, where before they were skipped. A frame that reloaded since the read, even at the same address, is refused rather than acted on blind. This reaches local agents and claude.ai or ChatGPT alike.
- The Claude Code plugin's manifest names itself "Oya Browser" to match its directory listing, and its description no longer carries compliance wording, which the directory could not accept automatically.
- Keel's page reads now show the scan animation and the outlines on the page, and its clicks and typing are narrated on the shield, as other agents' actions are.

## [1.0.287](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.287) · 2026-11-24

### Fixed

- The agent's page scan and its on-page outlines now agree on common form patterns. A field under its own floating label is no longer reported as covered and gets its outline; a hidden "I agree" checkbox is seen and outlined through its label; controls inside a fully transparent banner are no longer reported as visible.

## [1.0.286](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.286) · 2026-11-24

### Fixed

- The desktop browser no longer freezes with "Could not change control" until it is force-quit. A control change that hit an error left the takeover waiting forever and closed the server connection for good; the takeover now finishes, and only a failed sign-in ends the connection.

### Added

- Added owned-frame page evaluation, native hit testing, actionable cross-origin frame reads and verified browser/profile/DPR identity for Keel diagnostics. Native click paths report their actual input method; shielded macOS input returns an explicit capability error before scrolling or focus, preserving the browser control fence. Native clicking beneath that shield remains unavailable.

## [1.0.285](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.285) · 2026-11-24

### Fixed

- The desktop app no longer stops taking agent commands after a page closes its own tab (for example Gmail's pop-out compose). Every command failed with "Cannot read properties of undefined (reading 'isFocused')" until Oya was restarted.

## [1.0.284](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.284) · 2026-11-23

### Added

- A comparison of Oya and Amazon Nova Act for healthcare browser agents at /vs/amazon-nova-act, with every Nova Act fact sourced from AWS's own pages.
- Short addresses for healthcare buyers: /security, /hipaa, /healthcare, /use-cases/* and /compare/* now lead to the trust center, the BAA page, the healthcare page, the workflow pages and the comparisons.
- The BAA page answers whether you can self-host to keep PHI in your own VPC, and whether the free community edition is covered.
- llms.txt opens with a healthcare and HIPAA summary, and the self-hosting docs link to the BAA page and trust center.

## [1.0.283](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.283) · 2026-11-23

### Fixed

- Added session-scoped viewport sizing to local MCP and preserved screenshot metadata; wrapped links now use reachable client-rect points for both coverage analysis and native clicks.

## [1.0.282](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.282) · 2026-11-23

### Fixed

- A self-hosted license whose payment is refunded in full or disputed is revoked, and its subscription issues no new keys; a revoked key stops working at the server's next daily check-in. Previously the key kept working until it expired, up to a year and 14 days later.

## [1.0.281](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.281) · 2026-11-23

### Added

- Self-hosted servers can buy more cloud browsers themselves. The billing page has a Self-hosted license section: choose 6 to 50 browsers, billed $20 a browser a month or $240 a year, pay through Stripe, and copy the key to set as `OYA_LICENSE_KEY`. Each renewal issues a new key; every key works 14 days past its paid period so there is time to swap it in.

### Changed

- A self-hosted server past its cap now answers with a link to buy a license (`upgrade_url`) instead of an email address. For more than self-serve sells, refusals, the billing page and the docs link a call with the founders instead of sales@getoya.ai.

## [1.0.280](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.280) · 2026-11-21

### Changed

- Maintenance and fixes.

## [1.0.279](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.279) · 2026-11-21

### Changed

- One palette of five colours across the desktop app, the website and the console: ink, paper, the logo's lime, red and amber. Lime is the only green; the olive, forest and other greens, the blues and purples are gone. Light and dark themes use the same five.
- Desktop: buttons, the start page's "DO?", switches and the connected dot are lime with dark content in the light theme, and success shows in lime. Profile avatars differ by fill style rather than hue. The control shield, the share picker and the page dialogs use the same palette.
- Desktop: the connect screen shows the ChatGPT, Codex and VS Code marks.
- Website: dark sections are charcoal rather than green-black, and every lime is the logo's. The trust, research, release-notes, legal and answer pages and the favicon follow the same palette.
- Console and docs: the green-black dark theme becomes charcoal, light-theme buttons are lime with dark text, form outlines are easier to see, code highlighting uses the five colours, and info notes are neutral. Docs and sign-in links are underlined.
- A test in each part fails if a colour outside the five is added, if lime is used as text on a light background, or if text drops below 4.5:1 contrast.

## [1.0.278](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.278) · 2026-11-21

### Changed

- Maintenance and fixes.

## [1.0.277](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.277) · 2026-11-21

### Fixed

- Desktop: pairing the app with a different account now switches the account at once in the toolbar avatar, the profile menu and Settings. Before, the previous account stayed on screen until Settings was opened, and its cached details and claim page were kept on disk.
- Desktop: the app names the account it remembers from launch, even when the server cannot be reached; it used to say "Signed in with an API key" until it connected.
- Desktop: an answer about the previous account that arrives after a switch is discarded and can no longer overwrite the new one.

### Changed

- Desktop: every colour in the shell's stylesheets now comes from the design tokens, with new tokens for success, warning, information, the dialog scrim and the profile avatar tones. Status badges, avatars and Studio error text are easier to read in the light theme. A check in the test suite fails on any colour written outside the tokens.

## [1.0.276](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.276) · 2026-11-21

### Changed

- Maintenance and fixes.

## [1.0.275](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.275) · 2026-11-21

### Changed

- Maintenance and fixes.

## [1.0.274](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.274) · 2026-11-21

### Changed

- Maintenance and fixes.

## [1.0.273](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.273) · 2026-11-21

### Added

- Added the official dark-theme Product Hunt launch embed badge to the landing hero on `/` and allowed `https://api.producthunt.com` in `Content-Security-Policy` `img-src`.

## [1.0.272](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.272) · 2026-11-20

### Added

- Emitted full semantic HTML inside `<content:encoded>` (`xmlns:content`) on `/feed.xml` for `/research/*` posts so RSS readers and syndication importers receive full research tables and methodology instead of one-sentence summaries.
- Expanded `Organization.sameAs` structured data to 15 verified profiles, adding `x.com/OyaAgents`, `dev.to/oya-browser`, `oyabrowser.hashnode.dev` and `reddit.com/user/OyaAI/`.

## [1.0.271](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.271) · 2026-11-20

### Changed

- Maintenance and fixes.

## [1.0.270](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.270) · 2026-11-20

### Changed

- Maintenance and fixes.

## [1.0.269](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.269) · 2026-11-20

### Added

- Added five sourced comparison pages (`/vs/playwright-mcp`, `/vs/chrome-devtools-mcp`, `/vs/firecrawl`, `/vs/stagehand`, `/vs/skyvern`) linked from `/vs`, site footers, `/sitemap.xml` and `/llms.txt`, with an upgraded comparison table provenance bar, highlighted Oya column, differentiated "Not stated" pills and side-by-side fit summary cards.

### Changed

- Re-audited all public vendor comparisons on `2026-11-20`, expanded root `Organization.sameAs` with verified public profiles (`oya.dev`, Glama, YouTube, X), and lengthened meta descriptions on `/release-notes`, `/privacy`, `/terms` and `/vs` to 150-160 characters.
- Notified IndexNow automatically after successful full and server-only production deployments, with bounded requests and a visible, nonblocking warning when notification failed.

## [1.0.268](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.268) · 2026-11-20

### Fixed

- Corrected the release secret scan's false positive on public documentation/feed metadata with one exact historical exception; all other secret detection remains enabled.

## [1.0.267](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.267) · 2026-11-20

### Added

- Added a dedicated pricing page, an RSS feed for research and releases, and structured data for public pages and published plans.
- Added cached agent-discovery aliases for the documentation index and agent card while preserving OAuth discovery routes.

### Changed

- Made coding-agent setup easier to discover from Product navigation and documentation, and aligned desktop welcome, Connect, session restart and recovery instructions across the website, README and agent guides.
- Distinguished authenticated cloud MCP and plugin setup from local desktop Connect, corrected headless SDK examples to start and stop cloud browsers, and replaced an absolute bot-detection claim with explicit limitations.
- Kept canonical HTML pages in the sitemap, retained Markdown discovery through agent docs and alternate links, and omitted modification dates because request times, benchmark dates and vendor-check dates do not establish an editorial update.
- Preserved RSS, agent-document and API alternate links on public pages with their own canonical metadata, including pricing, comparisons and documentation topics.
- Added a dedicated `/pricing` page, a static `/feed.xml` RSS feed, a concise `/llms.txt` index mirrored at `/.well-known/llms.txt` with public cache headers, unified JSON-LD structured data (`Organization`, `SoftwareApplication`, `TechArticle` and `BreadcrumbList`), and broadened coding-agent discovery metadata in `server.json` and `.claude-plugin/plugin.json`.

## [1.0.266](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.266) · 2026-11-20

### Changed

- The desktop app's first-run setup shows every way to connect on one screen: continue with ChatGPT, continue with Oya, one button per AI app on this computer (Claude Code, Codex, Cursor, VS Code), your own AI key, or your own server. Every option is disabled while a sign-in runs, an app that is not installed no longer shows a test prompt, and the skip link reads "No AI for now, just browse".

## [1.0.265](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.265) · 2026-11-20

### Added

- Coding agents connected to the desktop app can test a web app and pull data from pages: `read_console` and `read_network` show what the page in the agent's own tab logged and fetched, and `wait_for` waits for text or an element. An agent reads only a tab it opened itself, never the person's tabs or another agent's.
- One line installs and connects Oya for a coding agent: `curl -fsSL https://oyabrowser.com/install-desktop.sh | sh -s -- claude-code` (and a PowerShell line for Windows) installs the app if it is missing, checks Oya's signature before copying it, opens it, and asks the person in a dialog before connecting. Cancel is the default; after a Cancel, further requests are ignored for five minutes.
- llms.txt and the agent skill lead with that line, and add two recipes: testing a web app on localhost, and extracting data from pages into rows.

### Fixed

- Reading a page's console or requests through the browser commands returned every tab's entries, the person's own tabs among them. It now returns only the calling agent's own tab.

## [1.0.264](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.264) · 2026-11-20

### Changed

- Maintenance and fixes.

## [1.0.263](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.263) · 2026-11-20

### Changed

- Maintenance and fixes.

## [1.0.262](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.262) · 2026-11-20

### Fixed

- Hardened server authorization and connection lifecycle: restricted pairing, persona-group, pool-cookie and credential-import writes to administrators, confined shared-browser MCP links to single-browser page tools, rejected invalid MFA domain clears, serialized WebSocket admission and command-send failure cleanup, and handled malformed upgrade URLs, billing state, and usage accounting cleanly.
- Improved desktop browser reliability: kept alert/confirm/prompt dialogs scoped per page, preserved control ownership and local MCP session cleanup on disconnect or `DELETE /mcp`, kept offline MCP access to the starting tab after opening new tabs, aborted in-flight chats on sign-out and empty tab strips, deduplicated download titles, and fixed Library site filtering for hostless items.
- Fixed console and docs behavior: decoded Base64URL JWT expiry for background session renewal, preserved MFA requirements on OAuth/claim callbacks, kept `/dashboard?browser=` deep links intact, reset workspace tabs and modals across project and manual API-key switches, persisted shared `/live` tokens across same-tab reloads with accurate initial control mode, aligned `/docs` search anchors and mobile scrolling, and fixed header menu dismissal and keyboard shortcuts.
- Hardened the CLI, TypeScript SDK, and Python SDK: wrote config, `.env`, and cookie jar files atomically owner-only without following symlinks, scrubbed secret prompts from readline history, preserved existing `.env` secrets and quoted special characters on `oya install --config`, normalized schemeless `oya login` URLs, improved global flag and TTY menu input handling, URL-encoded SDK path segments, and aligned Python SDK start options, timeouts, header checks, and element validation.

## [1.0.261](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.261) · 2026-11-20

### Added

- A page on browser automation for regulated industries: the audit trail, access limits, human takeover and self-hosting a reviewer checks, with SOC 2 status and HIPAA scoped to healthcare on Oya Cloud and the enterprise edition. It is linked from the footer, the sitemap and llms.txt.

### Changed

- The prior authorization, eligibility, claim status and payer portal pages each explain their own task: the portal steps, what the agent reads and writes, and where a person takes over.
- Six healthcare pages now say what they are about in their main heading, such as "payer portal automation" and "browser for healthcare".
- The trust page shows HIPAA mode on Oya Cloud as available with a BAA on request, matching the enterprise trust center.

## [1.0.260](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.260) · 2026-11-19

### Changed

- Updated new-subscription billing choices to Developer $500/month and Startup $2,500/month, and preserved existing subscribers' plan recognition when Stripe price IDs change.
- Highlighted the homepage's browser-scaling sentence in lime, matching the requested marker treatment.
- Darkened supporting text across the public website so deployment guidance, answers, pricing notes and qualifications are easier to read on light surfaces.
- Made the homepage's HIPAA, BAA and SOC 2 Type I claims more prominent in a linked trust badge beside the primary actions.
- Redesigned the homepage hero with an editorial split layout, refined typography and a luminous frame around the product video, preserving its content and controls.
- Removed the heading and introduction above the homepage hero video.
- Replaced the homepage's abstract hero artwork with the recorded product demo and updated the website's listed monthly prices to $500 for Developer and $2,500 for Startup.
- Connected the homepage around a first portal case and its reusable steps, restored the recorded prior-authorization demo, and added visible pricing with a direct navigation link so visitors can evaluate the workflow and cost together.

## [1.0.259](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.259) · 2026-11-19

### Changed

- Setting up the desktop browser is simpler: the headline is "Connect the browser your way to enable AI mode", the three starting cards say what they do and what you need, and each shows the logos of the tools it works with in place of the mock-up pictures. The inner screens use plainer wording, and the getting-started docs use the same card names.

## [1.0.258](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.258) · 2026-11-19

### Changed

- The public website, the docs, sign-in and the console are back to the look and home page they had before 1.0.256. The desktop browser keeps its new look. The docs side menu still follows the page.

## [1.0.257](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.257) · 2026-11-19

### Changed

- The desktop browser has the website's look: flat charcoal in dark and warm paper in light, a lime accent, square edges and tall condensed headings. The Oya mark is drawn flat in lime, and the start page and setup screen have a plain background.

## [1.0.256](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.256) · 2026-11-19

### Changed

- The public website has a new look: flat charcoal with one lime accent, tall condensed headlines and square edges, across the website, the docs, sign-in and the console, in both the dark and light themes. The wording and sections are the same.
- The docs side menu now scrolls with the page, so the section you are reading stays highlighted and in view.
- The home page has a pricing section: Free, Developer, Startup and Enterprise, with Enterprise leading to sales. "Pricing" is in the menu and `/pricing` lands on it.
- The home page has a new headline, "No API? No problem. Automate any portal with browser agents.", and shows the result of a run straight after the demo.
- The Security and Release notes pages load when the site is started from the repository root.
- The home page's hero now shows the recorded prior-authorization demo in place of the artwork.

## [1.0.255](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.255) · 2026-11-19

### Fixed

- The website's macOS and Windows download buttons download again. They had pointed at a 1.0.83 file that no longer exists; they now always fetch the newest installer.
- The whole Product row in the phone menu opens the product links, not only the small arrow.

### Changed

- The download buttons in the site footer are now filled buttons with a download icon.
- Every public page now uses the home page's header and footer, so the menu and footer links are the same everywhere.

## [1.0.254](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.254) · 2026-11-19

### Fixed

- The landing page menu on iPhone works again: tapping Product, Docs or any other item no longer closes the menu before the tap lands.

## [1.0.253](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.253) · 2026-11-18

### Changed

- Extended the forest, ivory and lime design across the website, documentation, authentication and console. Mobile navigation and full scrolling tables keep every destination and column available; the landing diagram has a separate routing lane, and documentation anchors honor reduced motion.
- Added qualified workflow-maintenance and portal-document stories, with primary sources and a founder-led invitation to scope an SOP or screen recording.
- Gave all 22 answer pages subject-specific visual heroes and clearer feature, setup and comparison layouts. Added finite section entrances and disclosure feedback, with visible server content, reduced-motion support and immediate settling on interaction.
- Simplified the maintenance story to plain server and desktop outcomes, keeping the technical repair details available on demand.
- Rewrote the landing page and all 22 answer pages as task-led narratives: start with one case, explain reusable steps and intervention, then end with the relevant next action. Search metadata, comparison evidence and dated BAA facts stay intact.

## [1.0.252](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.252) · 2026-11-18

### Added

- The Python SDK, `oya-browser` 0.2.2: `Oya(model={"provider": ..., "name": ..., "api_key": ...})` picks the model for one client, as `@oya-ai/browser` does, and `oya.personas.create`, `clone` and `update` take keyword arguments (`create(name="us", max_concurrent=2)`), with a dict still accepted.
- The repository README links both SDKs: `@oya-ai/browser` on npm and `oya-browser` on PyPI.

### Fixed

- The server reads a `null` `data` or `secrets` on a chat or a background run as none, so an older client that sends `null` (the Python SDK before 0.2.1) is no longer refused.
- On oyabrowser.com, the "change one line" cards and the comparison tables' first column showed black; the site header no longer lets the page show through behind its links; the home page's opening paragraph is shorter.

## [1.0.251](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.251) · 2026-11-18

### Fixed

- Python SDK 0.2.1: `browser.ask()` and `extract()` called without `data` or `secrets` failed, because they sent `null` for both and the server refuses that. They now leave out what was not given, as the TypeScript SDK does.

## [1.0.250](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.250) · 2026-11-18

### Added

- The Python SDK, [oya-browser on PyPI](https://pypi.org/project/oya-browser/): `pip install oya-browser`. The same surface as `@oya-ai/browser`, sync (`Oya`) and async (`AsyncOya`), and `browser.for_browser_use()` hands an Oya browser to browser-use in one line.
- The home page and the docs open with the one line each stack changes to run on Oya: browser-use, Playwright (Python and TypeScript), Browserbase, and Claude Code, Codex or Cursor.

### Fixed

- A background run (`browser.submit()`) that brings its own login, as `secrets` or as a replay's secret variables, signs in itself instead of waiting forever for a person when no login is stored on the profile. A stored login, a refused password, a lockout, MFA and CAPTCHA still hand off as before.
- "No CAPTCHA solver configured" and "<vendor> is not configured" now say where to fix it on a hosted key: the console's Settings tab or `oya.config.set(...)`, with the server variable only for self-hosting.
- The `@oya-ai/browser` README: the portal example saves its login on the profile, `analyze()` asks for markdown, `extract()` is typed, and the `login` attention reason is listed.

## [1.0.249](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.249) · 2026-11-18

### Added

- [A BAA page](https://oyabrowser.com/baa): how to get a Business Associate Agreement for HIPAA mode (on Oya Cloud on request, or self-hosted in your own cloud), what HIPAA mode changes, where Oya Cloud runs PHI on Google Cloud, and the evidence available on request.

### Changed

- SOC 2: the site now states the issued SOC 2 Type I report; the Type II audit is in progress.
- The comparison pages say HIPAA mode uses no outside browser vendors, proxies or CAPTCHA solvers, and that audit records are kept six years in HIPAA mode.

## [1.0.248](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.248) · 2026-11-18

### Added

- Comparison pages built from each vendor's own public pages, linked and dated: [best browser for AI agents](https://oyabrowser.com/best-browser-for-ai-agents) (Browserbase, Steel, Anchor, Browser Use, Kernel), [best payer portal automation](https://oyabrowser.com/best-payer-portal-automation) (Skyvern, CloudCruise, QuickIntell, Champ AI) and [best AI browser](https://oyabrowser.com/best-ai-browser) (Comet, Dia, Claude in Chrome, Aside, ego). Each Oya vs page now has a side-by-side table too.

### Changed

- The home page leads with what Oya is for: build a browser automation on your desktop, deploy it anywhere, replay it with no model.
- When an agent connects to the desktop app's MCP server or loads the oya-browser skill, it is told Oya is the browser the person installed for it, and uses Oya for web tasks unless the person asks for another.

### Fixed

- browser-use could not drive an Oya browser: Oya's CDP endpoint refused the `background` option browser-use sends when it opens its tab. It is now accepted; the tab opens as every automation tab does.

## [1.0.247](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.247) · 2026-11-18

### Fixed

- An agent run in the desktop app now survives a dropped connection. The run used to stop whenever the network cut the long request behind it, with "the connection to the server dropped mid-run, likely a server restart". Now the server keeps the run going for two minutes, the panel says "Reconnecting…", and the app comes back for the same run and shows its real answer. The agent stays in the tab it was working in, and Stop still stops the run at once. Needs this release on both the server and the desktop app.
- The message shown when a run really could not be picked up again no longer blames a server restart.

## [1.0.246](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.246) · 2026-11-18

### Fixed

- Releases were blocked by a high-severity advisory in picomatch (GHSA-fc5q-39xc-vjpf) that has no fixed version yet. It reaches the server only through the Daytona SDK's image builder, which Oya never calls, so the dependency audit now allows it by id until a fix ships.

## [1.0.245](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.245) · 2026-11-18

### Added

- Two pages for people choosing an AI browser: [the browser for Claude Code, Codex and Cursor](https://oyabrowser.com/browser-for-claude-code) and [an AI browser that works with ChatGPT and Claude](https://oyabrowser.com/ai-browser-chatgpt-claude).

### Changed

- The home page title and description are shorter, and each payer portal page's title now starts with what it automates (for example, "Payer portal automation for portals with no API").

## [1.0.244](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.244) · 2026-11-17

### Added

- An [interactive API reference](https://oyabrowser.com/docs/api/reference): every public route, rendered from [/openapi.json](https://oyabrowser.com/openapi.json).

### Changed

- `/openapi.json` now describes the API the SDK uses, each path under its real `/api` prefix, including playbook run history and typed request bodies. Routes only the console, the desktop app or an operator call are left out, and a test keeps the spec and the server's routes in step.

## [1.0.243](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.243) · 2026-11-17

### Fixed

- A script driving your Oya desktop app through the SDK (`oya.desktop.connect()`) now works in one tab of its own: it no longer takes over the tab you are looking at, every `goto()`, `ask()`, `play()` and background run stays in that tab, and `stop()` closes it. Before, each run left a second tab open and replaced the page you had open.

## [1.0.242](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.242) · 2026-11-17

### Added

- Run history for playbooks. Every replay, from the console, the SDK, the desktop app, MCP or a routine, is kept when it ends: the newest 20 per playbook, sealed per project. The console shows them under a Runs button on each playbook, and `GET /playbooks/:name/runs` returns them. `GET /playbooks` now says how each playbook last ran (`lastRun`, `null` if never).
- A visual view of a playbook in the console's editor: Start, one card per step, End. Click a step to edit its fields; the JSON stays the source.

### Changed

- Record a flow in the console no longer needs a running browser. With none running, it starts one with your project defaults and records on it once it connects.
- The desktop app's Playbooks panel is tighter: one-line search, compact rows, and a dot for how each playbook last ran.

## [1.0.241](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.241) · 2026-11-17

### Added

- [Research](https://oyabrowser.com/research) on oyabrowser.com: dated write-ups with the method and the raw numbers, starting with [how well a recorded run replays](https://oyabrowser.com/research/replay) and [stealth, measured](https://oyabrowser.com/research/stealth).
- [Compare](https://oyabrowser.com/vs), one page listing every Oya comparison, linked from the navigation and the footers.
- [What is an agentic browser?](https://oyabrowser.com/agentic-browser), and a Browsers group in the site menu linking it next to the [AI browser](https://oyabrowser.com/ai-browser) and [HIPAA-compliant browser](https://oyabrowser.com/hipaa-compliant-browser) pages.

## [1.0.240](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.240) · 2026-11-14

### Added

- New pages on oyabrowser.com for the payer portal work teams automate with Oya: [payer portal automation](https://oyabrowser.com/payer-portal-automation), [eligibility verification](https://oyabrowser.com/eligibility-verification-automation), [claim status](https://oyabrowser.com/claim-status-automation), [denials and appeals](https://oyabrowser.com/denial-management-automation), [provider enrollment](https://oyabrowser.com/provider-enrollment-automation) and [state registry lookups](https://oyabrowser.com/state-registry-automation).

### Changed

- [Oya vs Browserbase](https://oyabrowser.com/vs/browserbase) now leads with Oya as a HIPAA-compliant Browserbase alternative.

### Fixed

- The plugin manifest's contact and legal links, and the desktop app's author address, pointed at oya.ai, which no longer resolves. They now use sales@getoya.ai and oyabrowser.com/terms.

## [1.0.239](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.239) · 2026-11-13

### Changed

- Maintenance and fixes.

## [1.0.238](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.238) · 2026-11-13

### Fixed

- Reading a page with a dialog open now covers the whole dialog, not just its body. On LinkedIn's "comment, react and repost as" picker, agents saw the choices but never the Save button, so they could select an identity and never confirm it. A dialog named by `aria-labelledby` is now reported by its heading text instead of an element id.

## [1.0.237](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.237) · 2026-11-12

### Changed

- Maintenance and fixes.

## [1.0.236](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.236) · 2026-11-12

### Changed

- Maintenance and fixes.

## [1.0.235](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.235) · 2026-11-12

### Fixed

- Corrected installer test formatting that blocked v1.0.234's release pipeline, allowing the guided Docker, Kubernetes, Amazon ECS and Google Cloud installer to ship to npm.

## [1.0.234](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.234) · 2026-11-12

### Changed

- Redesigned the installer around four clear phases, compact completed answers and readable choices, with inline instructions for finding every required and Advanced value. Added narrow-terminal and plain-output fallbacks, and made cancellation and EOF stop setup safely.

- Connected the installer to the existing Docker, Kubernetes, Amazon ECS and Google Cloud/GKE deployment examples, with destination-specific questions, preflight, saved plans and recovery commands. Kept the original `--yes` quickstart and version 1 plans compatible.
- Made installer previews skip checkout updates and deployment writes, and made deployment verification wait for readiness and clean up temporary browsers and port-forwards on failure.

## [1.0.233](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.233) · 2026-11-11

### Added

- oyabrowser.com/vs/steel, /vs/anchor and /vs/browser-use: Oya next to each provider its API drives, each from a different side of Oya, with a visible FAQ and matching structured data, linked from both footers and in the sitemap.
- Each docs topic has its own page (oyabrowser.com/docs/sdk, /docs/playbooks, /docs/mcp and eight more) with its own title, description and canonical address, linked from the docs sidebar and in the sitemap. The full /docs page and the Markdown docs are unchanged.

## [1.0.232](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.232) · 2026-11-10

### Fixed

- The secret scan accepts the site's IndexNow key, which is public by design, so v1.0.231's site pages deploy.

## [1.0.231](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.231) · 2026-11-10

### Added

- oyabrowser.com/ai-browser, /browser-for-healthcare, /hipaa-browser-automation and /prior-authorization-automation, each answering its question with a visible FAQ and matching structured data, in the sitemap and llms.txt.
- Every answer page is linked from the site footer and the home page footer, and has its own social card.
- `npm run indexnow` submits the live sitemap to IndexNow (Bing and the engines it shares with) after a site deploy.

### Changed

- HIPAA and BAA claims on the answer pages name Oya Cloud and the enterprise edition.

## [1.0.230](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.230) · 2026-11-10

### Added

- oyabrowser.com/hipaa-compliant-browser and oyabrowser.com/vs/browserbase, each answering its question with a visible FAQ and matching structured data.
- /llm.txt and /llm-full.txt serve the same files as /llms.txt and /llms-full.txt.

### Changed

- One description of Oya Browser everywhere: the site, llms.txt, the README, npm and the MCP registry now say it is a HIPAA-compliant browser, an AI browser on the desktop and an agentic browser for teams.

## [1.0.229](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.229) · 2026-11-10

### Added

- Website notifications: a site asks once with a Show notifications? box, Allow or Block is remembered per site and profile (forget it under History and bookmarks), and an allowed site notifies from a background tab and from its service worker. Never for a tab an agent drives, a governed browser or incognito.

## [1.0.228](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.228) · 2026-11-10

### Fixed

- macOS: a site asking for a security key or phone passkey (Google's "Use a security key", for example) no longer hangs and blocks every later passkey prompt on the page with "Something went wrong"; it is declined the way a closed passkey dialog is, so Touch ID passkeys and other sign-in options keep working.

## [1.0.227](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.227) · 2026-11-10

### Fixed

- After signing in with Google in a popup, Gmail now reliably moves into a normal tab instead of sometimes staying in the popup window.
- Right-clicking an image offers Save Image and Copy Image, plus Open Image in New Tab and Copy Image Address when the image has a web address (Google search results included).

## [1.0.226](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.226) · 2026-11-09

### Changed

- Maintenance and fixes.

## [1.0.225](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.225) · 2026-11-09

### Changed

- Maintenance and fixes.

## [1.0.224](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.224) · 2026-11-09

### Changed

- New version-free download addresses, oyabrowser.com/download/mac and /download/windows, always serve the newest installer.
- llms.txt, the install guide and the skill tell an agent to install the desktop app itself and ask the person only for the Connect click.

## [1.0.223](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.223) · 2026-11-09

### Changed

- Every MCP tool now carries a title and read-only, destructive and open-world hints, which connector directories require and Claude's tool picker shows.

## [1.0.222](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.222) · 2026-11-09

### Changed

- The docs at oyabrowser.com/docs are brought in line with the product: the first-launch flow and footer Connect, MCP setup for Claude Code, Codex, claude.ai and the desktop app, every MCP tool in a reference table, the console's tabs and shortcuts, and the REST API intro. A test now fails when a tool exists in the code but not in the docs.

## [1.0.221](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.221) · 2026-11-09

### Changed

- Maintenance and fixes.

## [1.0.220](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.220) · 2026-11-09

### Changed

- The cloud MCP endpoint answers `initialize` and `tools/list` without an API key, so registries and agents can read the tool list before signing up. Tool calls, resources and streams still need a key.
- The MCP Registry listing carries an icon and points to the desktop app first.
- llms.txt and the README tell agents to install the desktop app and connect to it directly, as their browser in place of Chrome, with Oya Cloud as the fallback.

## [1.0.219](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.219) · 2026-11-09

### Changed

- Connected the landing sections into a story from the first task and reusable playbook to reported results, enterprise control and deployment.
- Replaced the landing hero's technical positioning with plain-language portal automation and reusable-playbook copy.
- Put the healthcare workflow first, with a task-specific prompt, visible saved playbook and inputs, followed by architecture and enterprise controls.
- Excluded installed third-party agent skill assets from application linting.

- Clarified how prompts become reusable playbooks and made the customer-reported productivity comparison easier to read.

- Rebuilt the landing page around Oya's enterprise browser control plane, with an animated architecture overview, orchestration/governance/observability details, deployment choices and an interactive healthcare workflow with human takeover. Improved typography and isolated landing styles so navigation no longer restyles sign-in and other pages.

## [1.0.218](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.218) · 2026-11-08

### Changed

- Maintenance and fixes.

## [1.0.217](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.217) · 2026-11-08

### Changed

- Maintenance and fixes.

## [1.0.216](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.216) · 2026-11-08

### Changed

- Maintenance and fixes.

## [1.0.215](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.215) · 2026-11-08

### Changed

- Maintenance and fixes.

## [1.0.214](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.214) · 2026-11-08

### Changed

- Maintenance and fixes.

## [1.0.213](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.213) · 2026-11-08

### Changed

- Maintenance and fixes.

## [1.0.212](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.212) · 2026-11-08

### Changed

- Rewrote the README for developers: what Oya fixes, a record-and-replay example on the first screen, and a quickstart that installs the control plane locally and runs the examples against its Docker browser.
- `new Oya({ model: { provider, name, apiKey } })` picks the model behind `ask()`, runs and replays in code, for that client only and never saved to the key. `oya init` is no longer needed to choose one.
- A bare `oya.browser.start()` with no provider and nothing connected now starts an Oya Cloud browser where the deployment runs them, instead of answering 409. A connected desktop or Docker worker is still borrowed first.
- `oya install --yes` picks up `GEMINI_API_KEY` (after `OPENAI_API_KEY` and `ANTHROPIC_API_KEY`), whose free tier makes a first run cost nothing.
- The no-model and no-key errors now say how to fix them in code first.

### Fixed

- Saving an agent run as a playbook no longer drops typing into a focused field, key chords, tab switches and other recorded steps. Replays had been ignoring new inputs while reporting success.
- Read-only page scripts can no longer fill a field through bracket assignment, `execCommand` or the native value setter, which left no step to replay.
- A playbook over 500 steps is refused instead of being saved cut short.
- The local install no longer fails building the browser image: `docker compose` pulls the published one, and `docker-compose.build.yml` builds it for those with the engine. Updates pull the latest browser image.
- After a local install, `start()` borrows the Docker browser worker instead of asking for Oya Cloud settings.
- The Playwright example exits when done on a borrowed browser instead of hanging.

## [1.0.211](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.211) · 2026-11-05

### Changed

- Added equal Technical setup choices for Oya sign-in, local agents and model API keys. Local-agent status now matches this browser, launch actions keep a practical GitHub test prompt ready, and Gemini, Claude API and OpenRouter keys receive a bounded provider test.
- Simplified desktop setup into illustrated Technical, Non-technical and AI expert choices, with short instructions, confirmed connection progress, Claude Code subscription guidance and a visual control-plane walkthrough. Import and default-browser choices no longer interrupt first use.
- Required timestamped Windows signing and verified installer/application signatures before release upload. Windows releases now fail when signing credentials are missing.

### Fixed

- Restored standard CDP browser-session attachment so clients can create independent page CDP sessions, with scoped events and cleanup that preserve other attachments. Iframe and nested worker sessions now keep independent identities for each page attachment. Approved page commands run concurrently within a fixed limit, so an unfinished response-body read cannot stall the crawl. Response fulfillment now unblocks intercepted navigations instead of waiting behind them.
- Prepared Windows releases for protected Azure managed signing with GitHub OIDC, exact publisher verification and a non-publishing signing probe.
- Corrected meeting microphone, camera and screen-sharing permission routing across browser windows. Added device-specific macOS recovery and distinguished permission denial from unavailable desktop sources.
- Opened macOS development sessions through Launch Services so Oya receives its own microphone and camera consent, with private startup logs and explicit application lifetime.
- Added local Chrome/Edge bookmarks and history import with explicit profile selection, partial-result reporting and no external SQLite dependency. Protected Windows signed-in sessions still require signing in directly in Oya.
- Corrected Windows local-agent executable discovery and VS Code configuration paths while keeping credentials out of shell commands.

## [1.0.210](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.210) · 2026-11-03

### Added

- Prepared AWS Marketplace account setup with server-side buyer resolution, encrypted authentication continuation and immutable account linkage. Registration remains disabled by default and pending until authoritative AWS subscription activation is implemented.

### Fixed

- Included the Keel activity migration in every hosted deployment path and added a regression guard against future migration omissions.

## [1.0.209](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.209) · 2026-11-03

### Added

- Added a staff Keel tab for recently reporting installs, current reported license IDs, measured agent-hours and active projects, with daily coverage and explicit UTC windows. Legacy reports retain unavailable measurements rather than fabricated zeroes.

### Fixed

- Aligned admin charts and trends to completed UTC days, corrected distinct-account comparisons and full installation totals, and separated active fixed-price MRR from currency-specific paid invoice totals. Clarified customer allowance periods, reported versus issued licenses, and the responding server’s fleet scope. Independent refreshes preserve useful snapshots and support forms while credential changes discard protected results.

## [1.0.208](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.208) · 2026-11-03

### Fixed

- Restricted business Slack lines and cards to account signups, API keys and payment outcomes. Browser sessions and CDP connections no longer produce business cards; desktop activity, downloads and self-hosted leads use the separate operational feed.

## [1.0.207](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.207) · 2026-11-03

### Security

- Updated vulnerable runtime and development dependencies, including Electron. Adapted legacy build tooling to the patched minimatch implementation.

### Added

- Prepared a default-off macOS browser-passkey engine experiment with native request authorization, controller cancellation and per-credential transport checks. Native compilation, Apple provisioning and physical sign-in remain required; this does not enable phone passkeys in released apps.

- Fixed passkey account selection being refused in a human-controlled tab while an agent works elsewhere. Account selection follows its real window and requesting document, without cancelling for unrelated iframe or same-page navigation.
- Added page translation through the project's model, with confirmation before sending text, a disclosed same-site continuation limit, and Show original. Translation stops on navigation outside that scope, recording, or agent takeover; agent reads wait for the original page to be restored.

- Fixed screen sharing being refused while microphone or camera permission was still being confirmed. Overlapping requests now wait briefly for their own consent, with navigation, profile and human-control checks retained.

### Fixed

- Rate-limit buckets are reclaimed again once they are full and idle. The sweep used to reset a bucket's idle clock before checking it, so a bucket for every sign-in email and caller address the server had ever seen was kept until restart.
- Usage counted in the last minute before the top of each hour is now recorded and billed. It used to be dropped when the hour rolled over before the next write.
- A browser that reconnects, or that the server drops (Disconnect, a revoked key, a control-plane stop), now gives back what its connection held at once. It stops being billed, its persona is free for another browser and can be deleted, its waiting commands fail instead of timing out, and CDP clients relayed over it are disconnected instead of left waiting. A reconnect that is accepted bills the old connection's time up to the reconnect instead of dropping it.
- Bound delayed socket, driver and operator cleanup to the registration being released, so a reconnect kept its own billing clock and persona slot. Released those resources before disconnect observers ran, and kept deferred vendor cleanup from releasing a replacement's slot.

## [1.0.206](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.206) · 2026-10-22

### Fixed

- A new tab dragged out into its own window now opens the site you type or pick there. It used to keep loading with a blank page, and Reload did not help, until you opened another tab in that window. Site icons on a dragged-out tab now update too.

## [1.0.205](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.205) · 2026-10-21

### Added

- A persona can run with no proxy. Set `proxy: { direct: true }` on create or update, and its browsers connect straight from their own network instead of an assigned proxy or Oya residential. The persona's `exit` reads `null`, and it takes no proxy slot.

## [1.0.204](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.204) · 2026-10-21

### Changed

- Maintenance and fixes.

## [1.0.203](https://github.com/OyadotAI/oya-browser/releases/tag/v1.0.203) · 2026-10-21

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
