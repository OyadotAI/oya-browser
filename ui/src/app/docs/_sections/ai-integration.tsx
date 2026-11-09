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
    'Works with the desktop app connected or the cloud endpoint configured.',
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
      <p className="mb-3 text-[15px] leading-relaxed">
        One endpoint for everything you have. It starts browsers itself, so there is no id to look up first:
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
        return 401 until you do. For everything an agent needs in one page, point it at{' '}
        <InlineLink href="/llms.txt">oyabrowser.com/llms.txt</InlineLink>.
      </p>
      <DesktopApp />

      <h3 id="cursor" className={SUB}>
        Cursor
      </h3>
      <p className={PARA}>
        Add to <InlineCode>.cursor/mcp.json</InlineCode> in your project, or let the desktop app write it for you with
        Connect:
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

      <McpSetupPart2 />
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
      <p className={PARA}>Or install the plugin, which brings the MCP server and a skill that teaches the workflow:</p>
      <CodeBlock label="Terminal">{`claude plugin marketplace add OyadotAI/oya-browser
claude plugin install oya-browser@oya

# any agent that reads skills, without the plugin:
npx skills add OyadotAI/oya-browser`}</CodeBlock>
      <Codex />
      <ClaudeAi />
    </>
  );
}

/** Codex: the TOML section the desktop app writes, pointed at the cloud when there is no desktop. */
function Codex() {
  return (
    <>
      <SubHeading id="codex">Codex</SubHeading>
      <p className={PARA}>
        Codex reads <InlineCode>~/.codex/config.toml</InlineCode>. This is the section the desktop app writes; swap the
        URL and token for the cloud endpoint and your key when there is no desktop:
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
        Install the <InlineLink href="/#download">desktop app</InlineLink>, then click <strong>Connect</strong> next to
        your agent&apos;s name in the Oya window footer. Claude Code, Codex, Cursor and VS Code appear there when
        installed. Connect writes an MCP server named <InlineCode>oya-browser</InlineCode> at{' '}
        <InlineCode>http://127.0.0.1:47321/mcp</InlineCode> into that agent&apos;s config; Claude Code also gets
        Oya&apos;s tools allowed without prompts. Start a new session and the tools are there. No account, no key.
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
