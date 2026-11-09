/**
 * Docs: the MCP tools, one section each.
 */
'use client';

import type { ReactNode } from 'react';
import { CodeBlock, InlineCode, NoteBox, SectionHeading, Table } from '../_docs/blocks';
import { MoreTools } from './mcp-tools-more';

/** Rows of a table in the scroll section. */
const SCROLL_ROWS_1: ReactNode[][] = [
  [
    <InlineCode key="dir">direction</InlineCode>,
    <>
      <InlineCode>&quot;up&quot;</InlineCode> | <InlineCode>&quot;down&quot;</InlineCode>
    </>,
    'Scroll direction',
  ],
  [<InlineCode key="amt">amount</InlineCode>, 'number (optional)', 'Pixels to scroll, default 500'],
];

/** The analyze_page section. */
function AnalyzePage() {
  return (
    <>
      {/* ============ TOOLS ============ */}
      <SectionHeading id="analyze_page">analyze_page</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Analyzes the current page. Returns the full page with every interactive element numbered, written as TOON (the
        default), markdown or JSONL. Left out, <InlineCode>format</InlineCode> falls back to the server&apos;s{' '}
        <InlineCode>OYA_PAGE_FORMAT</InlineCode>, which is <InlineCode>toon</InlineCode> unless set.
      </p>
      <CodeBlock>{`analyze_page()                      // TOON, the default: fewer tokens
analyze_page({ format: 'markdown' }) // markdown
analyze_page({ format: 'jsonl' })    // one JSON object per line`}</CodeBlock>
      <p className="mb-3 text-[15px] leading-relaxed">Returns:</p>
      <ul className="list-disc list-inside space-y-1 mb-4 text-[15px] leading-relaxed">
        <li>Page metadata, URL, title, viewport size, scroll position</li>
        <li>
          Full page content with element tags like <InlineCode>{`[#5 button "Submit"]`}</InlineCode> in markdown, or one{' '}
          <InlineCode>{`blocks[N]{id,region,kind,text,target,state}`}</InlineCode> row per block in TOON, or one JSON
          object per block in JSONL
        </li>
        <li>Element index, all elements listed with IDs, types, labels, visibility flags</li>
      </ul>
      <NoteBox>
        Always call <InlineCode>analyze_page</InlineCode> before using <InlineCode>click</InlineCode> or{' '}
        <InlineCode>type</InlineCode>. Element IDs only exist after analysis and reset on every call.
      </NoteBox>
    </>
  );
}

/** The navigate section. */
function Navigate() {
  return (
    <>
      <SectionHeading id="navigate">navigate</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">Navigate the browser to a URL.</p>
      <CodeBlock>{'navigate({ url: "https://example.com" })'}</CodeBlock>
      <NoteBox>
        After navigating, call <InlineCode>analyze_page</InlineCode> again, old element IDs are invalid on the new page.
      </NoteBox>
    </>
  );
}

/** The click section. */
function Click() {
  return (
    <>
      <SectionHeading id="click">click</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Click an interactive element by its ID number from <InlineCode>analyze_page</InlineCode>.
      </p>
      <CodeBlock>{'click({ element_id: 13 })'}</CodeBlock>
      <p className="mb-3 text-[15px] leading-relaxed">
        The element was tagged with <InlineCode>data-ac-id=&quot;13&quot;</InlineCode> during analysis, so the click
        resolves via a single <InlineCode>querySelector</InlineCode>.
      </p>
    </>
  );
}

/** The type section. */
function Type() {
  return (
    <>
      <SectionHeading id="type">type</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Type text into an input element. Clears existing content first, then types character by character with realistic
        key events.
      </p>
      <CodeBlock>{'type({ element_id: 9, text: "hello world" })'}</CodeBlock>
    </>
  );
}

/** The press_key section. */
function PressKey() {
  return (
    <>
      <SectionHeading id="press_key">press_key</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Press a keyboard key. Useful for submitting forms (Enter), dismissing dialogs (Escape), or navigating (Tab,
        arrows).
      </p>
      <CodeBlock>{'press_key({ key: "Enter" })'}</CodeBlock>
      <p className="mb-3 text-[15px] leading-relaxed">
        Allowed keys: Enter, Escape, Tab, Backspace, Delete, Space, ArrowUp, ArrowDown, ArrowLeft, ArrowRight, Home,
        End, PageUp, PageDown. No modifiers and no F-keys; the desktop app&apos;s local port and the cloud allow the
        same list. For a combination such as Shift+Tab use <InlineCode>key_chord</InlineCode>.
      </p>
    </>
  );
}

/** The screenshot section. */
function Screenshot() {
  return (
    <>
      <SectionHeading id="screenshot">screenshot</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">Capture the visible tab as a base64 PNG image.</p>
      <CodeBlock>screenshot()</CodeBlock>
    </>
  );
}

/** The scroll section. */
function Scroll() {
  return (
    <>
      <SectionHeading id="scroll">scroll</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">Scroll the page up or down.</p>
      <CodeBlock>{'scroll({ direction: "down", amount: 500 })'}</CodeBlock>
      <Table headers={['Param', 'Type', 'Description']} rows={SCROLL_ROWS_1} />
    </>
  );
}

/** The Tab Management section. */
function Tabs() {
  return (
    <>
      <SectionHeading id="tabs">Tab Management</SectionHeading>
      <NoteBox>
        Per-browser endpoint only: <InlineCode>list_tabs</InlineCode>, <InlineCode>open_tab</InlineCode>,{' '}
        <InlineCode>switch_tab</InlineCode> and <InlineCode>close_tab</InlineCode>, along with{' '}
        <InlineCode>read_elements</InlineCode>, <InlineCode>read_console</InlineCode>,{' '}
        <InlineCode>read_network</InlineCode> and the rows marked per-browser under More tools, are not served by{' '}
        <InlineCode>/mcp/pool</InlineCode>.
      </NoteBox>

      <h3 className="text-base font-semibold mt-6 mb-2 text-text">list_tabs</h3>
      <p className="mb-3 text-[15px] leading-relaxed">List all open tabs with ID, title, URL, and which is active.</p>
      <CodeBlock>list_tabs()</CodeBlock>

      <h3 className="text-base font-semibold mt-6 mb-2 text-text">open_tab</h3>
      <p className="mb-3 text-[15px] leading-relaxed">Open a new tab, optionally at a URL.</p>
      <CodeBlock>{'open_tab({ url: "https://gmail.com" })'}</CodeBlock>

      <h3 className="text-base font-semibold mt-6 mb-2 text-text">switch_tab</h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        Switch to one of your own tabs; <InlineCode>list_tabs</InlineCode> marks them yours. Tabs the person or another
        agent is using are refused; use <InlineCode>open_tab</InlineCode> instead.
      </p>
      <CodeBlock>{'switch_tab({ tab_id: 2 })'}</CodeBlock>

      <h3 className="text-base font-semibold mt-6 mb-2 text-text">close_tab</h3>
      <p className="mb-3 text-[15px] leading-relaxed">
        Close a tab you opened with <InlineCode>open_tab</InlineCode> (default: your current tab). Tabs the person or
        another agent opened are refused; leave them open.
      </p>
      <CodeBlock>{'close_tab({ tab_id: 3 })'}</CodeBlock>
    </>
  );
}

/** The wait section. */
function Wait() {
  return (
    <>
      <SectionHeading id="wait">wait</SectionHeading>
      <p className="mb-3 text-[15px] leading-relaxed">
        Wait for an element matching a CSS selector to appear on the page.
      </p>
      <CodeBlock>{'wait({ selector: ".results", timeout: 10000 })'}</CodeBlock>
    </>
  );
}

/** The MCP tools, one section each. */
export function McpToolsDocs() {
  return (
    <>
      <AnalyzePage />
      <Navigate />
      <Click />
      <Type />
      <PressKey />
      <Screenshot />
      <Scroll />
      <Tabs />
      <Wait />
      <MoreTools />
    </>
  );
}
