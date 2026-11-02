/**
 * Unit tests for the playbook editor (editor/playbook-editor.tsx), through what
 * a person sees and clicks: it loads the definition into CodeMirror, flags text
 * that is not one, opens files from the explorer as tabs, edits a variable,
 * saves with PUT or ⌘S, asks before throwing changes away, and tests the
 * unsaved edit on a browser in the test panel, showing each step as it plays.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { EditorView } from '@codemirror/view';

vi.mock('@/lib/api-client', async (orig) => ({ ...(await orig<object>()), api: vi.fn() }));
vi.mock('@/components/ui/syntax-code', () => ({ default: ({ code }: { code: string }) => <code>{code}</code> }));

import { api } from '@/lib/api-client';
import PlaybookEditor from '@/components/dashboard/playbooks/editor/playbook-editor';
import type { PlaybookInfo } from '@/components/dashboard/playbooks/types';
import type { BrowserRow } from '@/components/dashboard/types';

const apiMock = vi.mocked(api);
const DEF = {
  steps: [{ action: 'navigate', url: 'https://a.test/{{path}}' }],
  defaults: { path: 'signup' },
  secrets: [],
};
const PB = {
  name: 'order',
  variables: ['path'],
  defaults: {},
  steps: 1,
  code: 'await page.goto(url)',
  createdAt: 'c',
  promotedAt: null,
  draft: null,
} as unknown as PlaybookInfo;
const BROWSERS = [{ id: 'b-1', name: 'Local' }] as BrowserRow[];

/** Answers the definition, and `other` for anything else. */
function serve(other: (path: string, opts: { method?: string; body?: unknown }) => unknown = () => ({})) {
  apiMock.mockImplementation(async (path, opts) =>
    path === '/playbooks/order' && !opts?.method ? structuredClone(DEF) : other(path, opts),
  );
}

/** Renders the editor and waits for the definition. */
async function setup(onClose = vi.fn(), onSaved = vi.fn()) {
  render(<PlaybookEditor apiKey="k" playbook={PB} browsers={BROWSERS} onClose={onClose} onSaved={onSaved} />);
  await act(async () => {});
  return { onClose, onSaved };
}

/** The CodeMirror view labelled `label`. */
const viewOf = (label: string) => EditorView.findFromDOM(screen.getByLabelText(label))!;
/** The definition's text in the editor. */
const definitionText = () => viewOf('Playbook definition (JSON)').state.doc.toString();
/** Replaces the definition's text, as typing would. */
function typeDefinition(text: string) {
  const view = viewOf('Playbook definition (JSON)');
  act(() => view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } }));
}
/** Opens a file from the explorer. */
const openFile = (name: RegExp) => userEvent.click(screen.getByRole('treeitem', { name }));

describe('PlaybookEditor', () => {
  beforeEach(() => {
    apiMock.mockReset();
  });
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it('loads the definition into the editor', async () => {
    serve();
    await setup();
    expect(JSON.parse(definitionText())).toEqual(DEF);
    expect(screen.getByRole('tab', { name: 'playbook.json' }).getAttribute('aria-selected')).toBe('true');
    expect((screen.getByRole('button', { name: 'Save' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('says why text that is not a definition cannot be saved', async () => {
    serve();
    await setup();
    typeDefinition('{"steps": ');
    expect(screen.getByRole('alert').textContent).toMatch(/^Not valid JSON/);
    expect(screen.getByText('1 problem')).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Save' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('edits a variable’s default from the Variables tab, and saves with PUT', async () => {
    const { onSaved } = await (serve((path, opts) => (opts.method === 'PUT' ? { name: 'order' } : {})), setup());
    await openFile(/variables/);
    fireEvent.change(screen.getByLabelText('Default for path'), { target: { value: 'checkout' } });
    await userEvent.click(screen.getByRole('button', { name: 'Save' }));
    const put = apiMock.mock.calls.find(([, o]) => o?.method === 'PUT');
    expect(put?.[0]).toBe('/playbooks/order');
    expect((put?.[1]?.body as typeof DEF).defaults).toEqual({ path: 'checkout' });
    expect(onSaved).toHaveBeenCalled();
  });

  it('saves under a new name as an export the import route reads', async () => {
    serve((path) => (path === '/playbooks/import' ? { name: 'order-2', code: 'c' } : {}));
    await setup();
    typeDefinition(toTextWith('copy'));
    vi.spyOn(window, 'prompt').mockReturnValue('order-2');
    await userEvent.click(screen.getByRole('button', { name: 'Save as…' }));
    const call = apiMock.mock.calls.find(([p]) => p === '/playbooks/import');
    expect(call?.[1]?.body).toMatchObject({
      name: 'order-2',
      playbook: { format: 'oya-playbook', version: 1, playbook: { name: 'order-2' } },
    });
  });

  it('shows the code for the steps just saved', async () => {
    serve((path, opts) => (opts.method === 'PUT' ? { name: 'order', code: 'await page.goto(saved)' } : {}));
    await setup();
    typeDefinition(toTextWith('new'));
    await userEvent.click(screen.getByRole('button', { name: 'Save' }));
    await openFile(/playwright\.ts/);
    expect(viewOf('The Playwright code for the saved steps. It updates when you save.').state.doc.toString()).toBe(
      'await page.goto(saved)',
    );
    expect(screen.getByText('TypeScript · Read-only')).toBeTruthy();
  });

  it('saves with ⌘S, and shows the unsaved dot until then', async () => {
    serve((path, opts) => (opts.method === 'PUT' ? { name: 'order' } : {}));
    await setup();
    typeDefinition(toTextWith('k'));
    expect(screen.getAllByText('●').length).toBeGreaterThan(0);
    fireEvent.keyDown(window, { key: 's', metaKey: true });
    await act(async () => {});
    expect(apiMock.mock.calls.some(([, o]) => o?.method === 'PUT')).toBe(true);
    expect(screen.queryAllByText('●')).toHaveLength(0);
  });

  it('opens files as tabs, closes them, and hides the explorer with ⌘B', async () => {
    serve();
    await setup();
    await openFile(/variables/);
    expect(screen.getAllByRole('tab').map((t) => t.textContent)).toEqual(['playbook.json', 'variables']);
    await userEvent.click(screen.getByRole('button', { name: 'Close variables' }));
    expect(screen.getByRole('tab', { name: 'playbook.json' }).getAttribute('aria-selected')).toBe('true');
    await userEvent.click(screen.getByRole('button', { name: 'Close playbook.json' }));
    expect(screen.getByText('Open a file from the explorer.')).toBeTruthy();
    fireEvent.keyDown(window, { key: 'b', ctrlKey: true });
    expect(screen.queryByRole('navigation', { name: 'Explorer' })).toBeNull();
  });

  it('closes on Escape, but not when the editor used the key itself (closing its search)', async () => {
    serve();
    const { onClose } = await setup();
    const used = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
    used.preventDefault();
    act(() => void window.dispatchEvent(used));
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalled();
  });

  it('holds focus while open, with the console behind it inert, and gives focus back on close', async () => {
    serve();
    const opener = document.body.appendChild(document.createElement('button'));
    opener.focus();
    const { unmount } = render(
      <PlaybookEditor apiKey="k" playbook={PB} browsers={BROWSERS} onClose={vi.fn()} onSaved={vi.fn()} />,
    );
    await act(async () => {});
    expect(document.activeElement).toBe(screen.getByRole('dialog'));
    expect(opener.hasAttribute('inert')).toBe(true);
    unmount();
    expect([opener.hasAttribute('inert'), document.activeElement]).toEqual([false, opener]);
  });

  it('does nothing on ⌘S in the editor with no changes', async () => {
    serve();
    await setup();
    const content = screen.getByLabelText('Playbook definition (JSON)');
    fireEvent.keyDown(content, { key: 's', metaKey: true });
    await act(async () => {});
    expect(apiMock.mock.calls.some(([, o]) => o?.method === 'PUT')).toBe(false);
  });

  it('asks before closing with unsaved changes, and keeps them when told no', async () => {
    serve();
    const { onClose } = await setup();
    typeDefinition(toTextWith('x'));
    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(false);
    await userEvent.click(screen.getByRole('button', { name: 'Close' }));
    expect(confirm).toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('tests the unsaved edit on a browser and shows each step as it played', async () => {
    serve((path) =>
      path === '/browsers/b-1/runs'
        ? {
            id: 'run_1',
            status: 'failed',
            attention: null,
            error: 'Step 1 broke',
            events: [{ index: 0, action: 'navigate', ok: false }],
          }
        : {},
    );
    await setup();
    typeDefinition(toTextWith('edited'));
    await userEvent.click(screen.getByRole('button', { name: 'Toggle the test panel' }));
    await userEvent.type(screen.getByLabelText('Sample value for path'), 'cart');
    await userEvent.click(screen.getByRole('button', { name: 'Run test' }));
    const body = apiMock.mock.calls.find(([p]) => p === '/browsers/b-1/runs')?.[1]?.body as {
      playbook: typeof DEF;
      data: Record<string, string>;
      autoHeal: boolean;
    };
    expect(body.playbook.steps[0].url).toBe('https://a.test/{{path}}/edited');
    expect(body).toMatchObject({ data: { path: 'cart' }, autoHeal: false });
    expect(screen.getByText('Failed: Step 1 broke')).toBeTruthy();
    expect(screen.getByText('broke')).toBeTruthy();
  });
});

/** The definition with its URL given a suffix, as text. */
function toTextWith(suffix: string) {
  return JSON.stringify({ ...DEF, steps: [{ action: 'navigate', url: `https://a.test/{{path}}/${suffix}` }] });
}
