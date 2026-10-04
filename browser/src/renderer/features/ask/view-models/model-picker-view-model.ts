/**
 * The model picker in Ask's model card: a button naming the chosen model that
 * opens a searchable list. The arrow keys, Enter and Escape work as in any
 * list, and an id the list lacks can be used as typed. Model names come from
 * the server (OpenRouter's are third-party text), so views set them as text.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { ModelOption } from '../model/types.ts';
import { matchingModels } from '../model/model-search.ts';

/** What the picker shows. */
export interface ModelPickerState {
  /** Every model the provider offers. */
  models: ModelOption[];
  /** The chosen model id. */
  value: string;
  /** The list is open. */
  open: boolean;
  /** The search as typed. */
  query: string;
  /** The options listed now, in order. */
  shown: ModelOption[];
  /** The index of the option the keyboard is on. */
  active: number;
}

/** The searchable model picker. */
export class ModelPickerViewModel extends ViewModel<ModelPickerState> {
  /** What each key does while the list is open. */
  private readonly keys: Record<string, () => void> = {
    ArrowDown: () => this.move(1),
    ArrowUp: () => this.move(-1),
    Enter: () => this.chooseActive(),
    Escape: () => this.close(),
  };

  /** Empty and closed. */
  constructor() {
    super({ models: [], value: '', open: false, query: '', shown: [], active: 0 });
  }

  /** Offers `models`, with `value` chosen, and closes the list. */
  setModels(models: ModelOption[], value: string): void {
    this.set({ models, value, open: false });
  }

  /** The button was clicked: opens or closes the list. */
  toggle(): void {
    if (this.state.open) this.close();
    else this.openList();
  }

  /** Opens the list with an empty search, on the chosen model. */
  openList(): void {
    this.set({ open: true });
    this.search('');
  }

  /** Closes the list. */
  close(): void {
    this.set({ open: false });
  }

  /** Lists the models matching `query`, on the chosen one when it is listed and nothing is typed. */
  search(query: string): void {
    const trimmed = query.trim();
    const shown = matchingModels(this.state.models, trimmed);
    const chosen = shown.findIndex((o) => o.id === this.state.value);
    this.set({ query, shown, active: trimmed || chosen < 0 ? 0 : chosen });
  }

  /** A key in the search box: answers whether the picker used it (so the view stops it). */
  key(key: string): boolean {
    if (!Object.hasOwn(this.keys, key)) return false;
    this.keys[key]();
    return true;
  }

  /** The pointer is over option `index`: Enter would choose it. */
  hover(index: number): void {
    this.set({ active: index });
  }

  /** Chooses `id` and closes the list. */
  choose(id: string): void {
    this.set({ value: id, open: false });
  }

  /** Moves the keyboard `delta` options, wrapping at either end. */
  private move(delta: number): void {
    const count = this.state.shown.length;
    if (count) this.set({ active: (this.state.active + delta + count) % count });
  }

  /** Chooses the option the keyboard is on. */
  private chooseActive(): void {
    const option = this.state.shown[this.state.active];
    if (option) this.choose(option.id);
  }
}
