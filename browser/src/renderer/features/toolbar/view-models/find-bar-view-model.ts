/**
 * The find bar under the toolbar (Cmd/Ctrl+F): what the person typed, and the
 * count the main process found for it. The main process owns whether the bar is
 * open and which page it searches; this only asks and shows.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { FindBarState } from '../../../../shared/ipc.ts';
import { TEXT } from '../model/constants.ts';

/** What the bar shows. */
export interface FindBarViewState extends FindBarState {
  /** What is in the field. */
  query: string;
  /** Bumped by each Cmd/Ctrl+F, so the field takes focus and selects its text. */
  focusRequest: number;
}

/** The parts of the bridge the bar uses. */
export type FindBarBridge = Pick<OyaBrowser, 'onFindBar' | 'onShellCommand' | 'findInPage' | 'findStep' | 'closeFind'>;

/** The keys the field reacts to, as a keydown carries them. */
export interface FindKey {
  /** The key's name. */
  key: string;
  /** Shift held: Enter goes back. */
  shiftKey: boolean;
  /** An input method is composing: Enter commits the text, not a step. */
  isComposing: boolean;
  /** Keeps the key from the field. */
  preventDefault(): void;
}

/** What the count says: nothing before a search, "No results", or "3 of 17". */
export function findCount({ active, total }: FindBarState): string {
  if (total === null) return '';
  return total ? active.toLocaleString() + TEXT.findOf + total.toLocaleString() : TEXT.findNone;
}

/** The find bar. */
export class FindBarViewModel extends ViewModel<FindBarViewState> {
  /** The main process. */
  private readonly bridge: FindBarBridge;

  /** Shut until the main process opens it. */
  constructor(bridge: FindBarBridge) {
    super({ open: false, active: 0, total: null, query: '', focusRequest: 0 });
    this.bridge = bridge;
    this.own(bridge.onFindBar((state) => this.follow(state)));
    this.own(
      bridge.onShellCommand((name) => name === 'find' && this.set({ focusRequest: this.state.focusRequest + 1 })),
    );
  }

  /** The main process's state; a shut bar forgets its query, so it reopens empty. */
  private follow({ open, active, total }: FindBarState): void {
    this.set(open ? { open, active, total } : { open, active: 0, total: null, query: '' });
  }

  /** The person typed: search as they type. */
  type(query: string): void {
    this.set({ query });
    void this.bridge.findInPage(query).catch(() => {});
  }

  /** The next or previous match; nothing while the field is empty. */
  step(forward: boolean): void {
    if (this.state.query) void this.bridge.findStep(forward).catch(() => {});
  }

  /** Closes the bar and gives the keyboard back to the page. */
  close(): void {
    void this.bridge.closeFind().catch(() => {});
  }

  /** Enter and Shift+Enter step through matches (not while composing); Esc closes. */
  key(event: FindKey): void {
    if (event.key === 'Escape') return (event.preventDefault(), this.close());
    if (event.key !== 'Enter' || event.isComposing) return;
    event.preventDefault();
    this.step(!event.shiftKey);
  }
}
