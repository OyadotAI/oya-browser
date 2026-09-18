/**
 * Unit tests for the snippets: which tabs a browser gets, what the code
 * points at, and the dialog's key masking and copying.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

vi.mock('@/lib/api', () => ({ apiOrigin: () => 'https://oya.test' }));

import SnippetsDialog, { browserSnippets, fleetSnippets } from '@/components/dashboard/snippets';
import type { BrowserRow } from '@/components/dashboard/types';

/** A browser row with just what snippets read. */
const row = (over: Partial<BrowserRow>) =>
  ({ id: 'b1', name: 'Checkout Bot', clientType: 'oya', provider: null, ...over }) as BrowserRow;

describe('snippet builders', () => {
  it('a CDP browser gets a working Playwright attach, second', () => {
    const snippets = browserSnippets(row({ clientType: 'cdp' }));
    expect(snippets.map((s) => s.id)).toEqual(['sdk', 'playwright', 'cli', 'mcp', 'curl']);
    expect(snippets[1].code('KEY')).toContain('wss://oya.test/connect?token=KEY&browser=b1');
  });

  it('an Oya client explains it has no CDP endpoint, last', () => {
    const snippets = browserSnippets(row({ provider: 'oya-cloud' }));
    expect(snippets.map((s) => s.id)).toEqual(['sdk', 'cli', 'mcp', 'curl', 'playwright']);
    expect(snippets[4].code('KEY')).toContain('This browser is an Oya client (oya-cloud)');
    expect(snippets[4].note).toBeUndefined();
  });

  it('names the MCP server after the browser, safely', () => {
    const mcp = browserSnippets(row({ name: 'Checkout Bot!' })).find((s) => s.id === 'mcp')!;
    expect(mcp.code('K')).toContain('"checkout-bot-"');
    const unnamed = browserSnippets(row({ name: '' })).find((s) => s.id === 'mcp')!;
    expect(unnamed.code('K')).toContain('"oya-browser"');
  });

  it('fleet snippets cover every way to start a browser', () => {
    const snippets = fleetSnippets();
    expect(snippets.map((s) => s.id)).toEqual(['sdk', 'cli', 'playwright', 'mcp', 'curl']);
    expect(snippets[1].code('KEY')).toContain('oya login --url https://oya.test --key KEY');
  });
});

describe('SnippetsDialog', () => {
  afterEach(cleanup);

  /** Opens the dialog on the fleet snippets. */
  const setup = () =>
    render(<SnippetsDialog open onClose={vi.fn()} apiKey="SECRET" title="Connect" snippets={fleetSnippets()} />);

  it('masks the key until asked to show it', async () => {
    setup();
    expect(document.body.textContent).not.toContain('SECRET');
    await userEvent.click(screen.getByRole('button', { name: /Show key/ }));
    expect(document.body.textContent).toContain('SECRET');
  });

  it('copying copies the real key, not the mask', async () => {
    const user = userEvent.setup();
    const write = vi.spyOn(navigator.clipboard, 'writeText');
    setup();
    await user.click(screen.getByRole('button', { name: /Copy with key/ }));
    expect(write.mock.calls[0][0]).toContain('apiKey: "SECRET"');
    expect(await screen.findByRole('button', { name: /Copied/ })).toBeTruthy();
  });

  it('a tab shows its own file and note', async () => {
    setup();
    await userEvent.click(screen.getByRole('tab', { name: 'Agents (MCP)' }));
    expect(screen.getByText('mcp.json')).toBeTruthy();
    expect(screen.getByText(/The pool hands each call/)).toBeTruthy();
  });

  it('renders nothing without snippets', () => {
    const { container } = render(<SnippetsDialog open onClose={vi.fn()} apiKey="k" title="t" snippets={[]} />);
    expect(container.innerHTML).toBe('');
  });
});
