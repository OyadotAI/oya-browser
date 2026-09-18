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
        Oya Browser exposes each connected browser as an MCP server at:
      </p>
      <CodeBlock>{'https://browser.getoya.ai/mcp/{BROWSER_ID}'}</CodeBlock>
      <p className="mb-3 text-[15px] leading-relaxed">
        Get your browser&apos;s ID from the <InlineLink href="/dashboard">dashboard</InlineLink> (shown under each
        browser name, or in the MCP Tools tab).
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
      "url": "https://browser.getoya.ai/mcp/YOUR_BROWSER_ID",
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
      "url": "https://browser.getoya.ai/mcp/YOUR_BROWSER_ID",
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
        Same config — add to your project&apos;s <InlineCode>.claude/mcp.json</InlineCode> or use the{' '}
        <InlineCode>/browse</InlineCode> skill command included in the repo.
      </p>
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
