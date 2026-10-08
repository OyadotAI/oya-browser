/** Most-recent selection is independent of tab-strip order and forgets closed tabs. */
import type { Tab } from './types.ts';
/** Remembers where a person was before following a link into another tab. */
export class TabSelection {
  /** Selected ids, newest last; only one entry per tab. */
  private recent: number[] = [];
  /** Moves the selected tab to the top of the return stack. */
  visit(id: number): number {
    this.forget(id);
    this.recent.push(id);
    return id;
  }
  /** Cycles in strip order, with either positive or negative offsets. */
  cycle(tabs: readonly Pick<Tab, 'id'>[], active: number | null, offset: number): number | undefined {
    if (!tabs.length) return undefined;
    const index = tabs.findIndex((tab) => tab.id === active);
    return tabs[(((index + offset) % tabs.length) + tabs.length) % tabs.length]?.id;
  }
  /** Closed tabs cannot become a future return destination. */
  forget(id: number): void {
    this.recent = this.recent.filter((previous) => previous !== id);
  }
  /** Ignores windows removed outside the tab-strip close path. */
  previous(tabs: readonly Pick<Tab, 'id'>[]): number | undefined {
    this.recent = this.recent.filter((id) => tabs.some((tab) => tab.id === id));
    return this.recent.at(-1);
  }
}
