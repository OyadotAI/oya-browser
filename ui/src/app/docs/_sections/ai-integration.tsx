/**
 * Docs: AI integration: MCP setup for the desktop app, Cursor, Claude Desktop
 * and Claude Code, and the prompts that prove the setup works.
 */
'use client';

import { CodeBlock, InlineCode, InlineLink, SectionHeading, SubHeading } from '../_docs/blocks';

/** The paragraph style this section's prose uses. */
const PARA = 'mb-3 text-[15px] leading-relaxed';
/** The sub-heading style this section uses. */
const SUB = 'text-base font-semibold mt-6 mb-2 text-text scroll-mt-28';

/** Prompts a person pastes into their agent, each with what should happen: label, lead-in, prompt, expected. */
const TRY_PROMPTS: [string, string, string, string][] = [
  [
    'Prompt 1 \u00b7 any agent',
    'Use this on your own computer to connect the Oya desktop app.',
    'Read https://oyabrowser.com/llms.txt, install Oya on this computer and set yourself up to browse the web through it instead of Chrome. Tell me when you need me to click Connect, then open https://news.ycombinator.com in Oya and read me the top three titles.',
    'Expected: the agent installs and opens Oya, asks you for the Connect click, then reads the three Hacker News titles from Oya.',
  ],
  [
    'Prompt 2 \u00b7 connector, no key',
    'Add https://oyabrowser.com/mcp/pool as a connector with no key, then:',
    'List every tool the Oya connector offers and what each one does. Do not call any of them yet.\n\nNow call pool_status.',
    'Expected: all 32 tools listed without signing in, then a sign-in prompt on pool_status.',
  ],
  [
    'Prompt 3 \u00b7 human handback',
    '',
    'Using Oya only, sign in to my GitHub notifications page, tell me how many unread notifications I have, then stop and hand the tab back to me.',
    'Expected: Oya opens GitHub, the agent reports the unread count, then the tab returns to you.',
  ],
];

/** The MCP Setup section. */
function McpSetup() {
  return (
    <>
      {/* ============ MCP SETUP ============ */}
      <SectionHeading id="mcp-setup">MCP Setup</SectionHeading>
      <DesktopApp />
      <CloudSetup />
      <Cursor />
      <McpSetupPart2 />
    </>
  );
}

/** Remote MCP is a separate authenticated choice for environments without the desktop app. */
function CloudSetup() {
  return (
    <>
      <SubHeading id="cloud-mcp">Oya Cloud: servers, containers and CI</SubHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Without a desktop app, use Oya Cloud with an Oya API key or a supported OAuth client. The cloud endpoint starts
        browsers itself:
      </p>
      <CodeBlock>{'https://oyabrowser.com/mcp/pool'}</CodeBlock>
      <p className={PARA}>
        Thirty-two tools: twenty-nine that act on a page, round-robined across your browsers, plus{' '}
        <InlineCode>start_browser</InlineCode>, <InlineCode>stop_browser</InlineCode> and{' '}
        <InlineCode>pool_status</InlineCode>. Tab tools, <InlineCode>read_elements</InlineCode>,{' '}
        <InlineCode>read_console</InlineCode>, <InlineCode>read_network</InlineCode> and the browser-specific tools
        marked in <InlineLink href="/docs#more-tools">More tools</InlineLink> are not on this endpoint; to use those,
        point at one browser instead, <InlineCode>/mcp/{'{BROWSER_ID}'}</InlineCode>, with the id from the{' '}
        <InlineLink href="/dashboard">dashboard</InlineLink>.
      </p>
      <p className={PARA}>
        <InlineCode>/mcp/pool</InlineCode> answers <InlineCode>initialize</InlineCode> and{' '}
        <InlineCode>tools/list</InlineCode> without a key, so an agent can list the tools before you sign in. Calls
        return 401 until authenticated. Listing tools does not grant browser access. For everything an agent needs in
        one page, point it at <InlineLink href="/llms.txt">oyabrowser.com/llms.txt</InlineLink>.
      </p>
    </>
  );
}

/** The manual Cursor example configures cloud access; local configuration is owned by desktop Connect. */
function Cursor() {
  return (
    <>
      <h3 id="cursor" className={SUB}>
        Cursor
      </h3>
      <p className={PARA}>
        For Oya Cloud, add this to <InlineCode>.cursor/mcp.json</InlineCode> with your Oya API key. For the desktop app,
        use Connect instead; it writes the local configuration for you.
      </p>
      <CodeBlock>{`{
  "mcpServers": {
    "oya-browser": {
      "url": "https://oyabrowser.com/mcp/pool",
      "transport": "streamable-http",
      "headers": {
        "Authorization": "Bearer YOUR_API_KEY"
      }
    }
  }
}`}</CodeBlock>
    </>
  );
}

/** The MCP Setup section, continued: Claude Code, Codex and the OAuth connector. */
function McpSetupPart2() {
  return (
    <>
      <h3 id="claude-code" className={SUB}>
        Claude Code
      </h3>
      <p className={PARA}>On a machine with no Oya app, add the cloud endpoint with Claude Code&apos;s own CLI:</p>
      <CodeBlock label="Terminal">{`claude mcp add --transport http oya https://oyabrowser.com/mcp/pool \\
  --header "Authorization: Bearer $OYA_API_KEY"`}</CodeBlock>
      <p className={PARA}>
        <strong>Cloud plugin for Claude Code:</strong> requires <InlineCode>OYA_API_KEY</InlineCode> and includes the
        remote MCP server and a browsing skill. For desktop browsing, use Connect in Oya instead.
      </p>
      <CodeBlock label="Terminal">{`claude plugin marketplace add OyadotAI/oya-browser
claude plugin install oya-browser@oya`}</CodeBlock>
      <p className={PARA}>The optional skill teaches the workflow; it does not configure an MCP connection:</p>
      <CodeBlock label="Agent skill">{'npx skills add OyadotAI/oya-browser'}</CodeBlock>
      <Codex />
      <ClaudeAi />
    </>
  );
}

/** The Codex example is explicitly cloud-only so readers do not replace app-managed local credentials. */
function Codex() {
  return (
    <>
      <SubHeading id="codex">Codex</SubHeading>
      <p className={PARA}>
        For Oya Cloud, add this to <InlineCode>~/.codex/config.toml</InlineCode> with your Oya API key. For the desktop
        app, use Connect instead; Oya writes the correct local URL and authentication for you.
      </p>
      <CodeBlock label="~/.codex/config.toml">{`[mcp_servers.oya-browser]
url = "https://oyabrowser.com/mcp/pool"
http_headers = { Authorization = "Bearer YOUR_API_KEY" }
default_tools_approval_mode = "approve"`}</CodeBlock>
    </>
  );
}

/** claude.ai and Claude Desktop sign in with OAuth; no key changes hands. */
function ClaudeAi() {
  return (
    <>
      <SubHeading id="claude-ai">claude.ai and Claude Desktop</SubHeading>
      <p className={PARA}>
        On a Pro, Max, Team or Enterprise plan you need no API key. Open Settings, then Connectors, then{' '}
        <strong>Add custom connector</strong>, paste <InlineCode>https://oyabrowser.com/mcp/pool</InlineCode>, and sign
        in to Oya when asked. Pick the project to connect and allow it. The Claude plan pays for the model; Oya is the
        browser. The connection shows as a credential in the project&apos;s settings, where it can be revoked. Any MCP
        client that signs in with OAuth works the same way.
      </p>
    </>
  );
}

/** The desktop app: the person clicks Connect and the agent is configured, no key involved. */
function DesktopApp() {
  return (
    <>
      <h3 id="desktop-app" className={SUB}>
        Desktop app
      </h3>
      <p className={PARA}>
        Download and open the <InlineLink href="/#download">Oya desktop app</InlineLink>. Complete the welcome screen,
        click <strong>Connect</strong> next to your installed agent in the window footer, then start a new agent
        session. Claude Code, Codex, Cursor and VS Code are supported. Oya writes the local MCP configuration for you:{' '}
        <InlineCode>oya-browser</InlineCode> at <InlineCode>http://127.0.0.1:47321/mcp</InlineCode>. No Oya account or
        API key is required. The desktop app includes the MCP server; no separate extension or MCP package is required.
      </p>
      <p className={PARA}>
        You confirm the connection and any OS permission for optional login import. If you skip import, sign in within
        Oya. Keep Oya open for repeat use; your agent uses its configured tools. If tools are missing, open Oya and
        check Connect, then start a new agent session. If you see &quot;Browser not ready&quot;, complete Oya&apos;s
        welcome screen and retry. Report persistent connection errors.
      </p>
    </>
  );
}

/** One prompt to paste, with its lead-in and what should happen. */
function TryPrompt({
  entry: [label, lead, prompt, expected],
}: {
  /** Label, lead-in, prompt, expected. */ entry: (typeof TRY_PROMPTS)[number];
}) {
  return (
    <>
      {lead && <p className={PARA}>{lead}</p>}
      <CodeBlock label={label} plain>
        {prompt}
      </CodeBlock>
      <p className={PARA}>{expected}</p>
    </>
  );
}

/** Three prompts that each prove a different part of the setup. */
function TryIt() {
  return (
    <>
      <h3 id="try-it" className={SUB}>
        Try it
      </h3>
      <p className={PARA}>Paste one of these into your agent. Each one proves a different part of the setup.</p>
      {TRY_PROMPTS.map((entry) => (
        <TryPrompt key={entry[0]} entry={entry} />
      ))}
    </>
  );
}

/** AI integration: MCP setup for the desktop app, Cursor, Claude Desktop and Claude Code, then prompts to try. */
export function AiIntegrationDocs() {
  return (
    <>
      <McpSetup />
      <TryIt />
    </>
  );
}
