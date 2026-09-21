/**
 * Docs: aI integration: MCP setup for Cursor, Claude Desktop and Claude Code.
 */
'use client';

import { CodeBlock, InlineCode, InlineLink, SectionHeading } from '../_docs/blocks';

/** The MCP Setup section. */
function McpSetup() {
  return (
    <>
      {/* ============ MCP SETUP ============ */}
      <SectionHeading id="mcp-setup">MCP Setup</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        One endpoint for everything you have. It starts browsers itself, so there is no id to look up first:
      </p>
      <CodeBlock>{'https://browser.getoya.ai/mcp/pool'}</CodeBlock>
      <p className="mb-3 text-[15px] leading-relaxed">
        Seventeen tools: fourteen that act on a page, round-robined across your browsers, plus{' '}
        <InlineCode>start_browser</InlineCode>, <InlineCode>stop_browser</InlineCode> and{' '}
        <InlineCode>pool_status</InlineCode>. Tab tools and <InlineCode>read_elements</InlineCode> are not on this
        endpoint; to use those, point at one browser instead, <InlineCode>/mcp/{'{BROWSER_ID}'}</InlineCode>, with the
        id from the <InlineLink href="/dashboard">dashboard</InlineLink>, which serves nineteen.
      </p>

      <h3 id="cursor" className="text-base font-semibold mt-6 mb-2 text-text">
        Cursor
      </h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        Add to <InlineCode>.cursor/mcp.json</InlineCode> in your project:
      </p>
      <CodeBlock>{`{
  "mcpServers": {
    "oya-browser": {
      "url": "https://browser.getoya.ai/mcp/pool",
      "transport": "streamable-http",
      "headers": {
        "Authorization": "Bearer YOUR_API_KEY"
      }
    }
  }
}`}</CodeBlock>

      <h3 id="claude-desktop" className="text-base font-semibold mt-6 mb-2 text-text">
        Claude Desktop
      </h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        Add to Claude Desktop&apos;s MCP config (Settings → Developer → Edit Config):
      </p>
      <McpSetupPart2 />
    </>
  );
}

/** The MCP Setup section, continued. */
function McpSetupPart2() {
  return (
    <>
      <CodeBlock>{`{
  "mcpServers": {
    "oya-browser": {
      "url": "https://browser.getoya.ai/mcp/pool",
      "transport": "streamable-http",
      "headers": {
        "Authorization": "Bearer YOUR_API_KEY"
      }
    }
  }
}`}</CodeBlock>

      <h3 id="claude-code" className="text-base font-semibold mt-6 mb-2 text-text">
        Claude Code
      </h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        Same config in <InlineCode>.claude/mcp.json</InlineCode>, or install the plugin, which brings the MCP server and
        a skill that teaches the workflow:
      </p>
      <CodeBlock>{`claude plugin marketplace add OyadotAI/oya-browser
claude plugin install oya-browser@oya

# any agent that reads skills, without the plugin:
npx skills add OyadotAI/oya-browser`}</CodeBlock>
    </>
  );
}

/** AI integration: MCP setup for Cursor, Claude Desktop and Claude Code. */
export function AiIntegrationDocs() {
  return (
    <>
      <McpSetup />
    </>
  );
}
