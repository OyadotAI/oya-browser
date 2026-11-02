/** Renderer-only native selection states; all capture authority remains in the private main-process sheet. */
import { ipcRenderer } from 'electron';
import {
  SHARE_PICKER,
  type NativePickerPresentation,
  type ShareOption,
  type SharePresentation,
  type ShareKind,
} from '../shared/share-picker.ts';
import { pickerElement as element, pickerText, pickerPreview } from './share-picker-elements.ts';

/** Native interaction is optional; its final Oya confirmation uses only opaque candidate tokens. */
export class NativePickerView {
  /** Direct action follows the adapter's current availability. */
  private availability: NonNullable<SharePresentation['native']>;
  /** Avoid changing state after a replacement UI action has completed. */
  private generation = 0;
  /** One pending native bridge action at a time. */
  private pending = false;
  /** Native candidates require the separate CONFIRM IPC, never an ordinary card answer. */
  private candidate: ShareOption | null = null;
  /** The existing category controller clears every previous selection. */
  private readonly list: () => void;
  /** The footer describes the exact offered candidate. */
  private readonly select: (source: ShareOption | null) => void;
  /** Recipient disclosure is visible before opening Apple's system surface. */
  constructor(data: SharePresentation, list: () => void, select: (source: ShareOption | null) => void) {
    this.availability = data.native ?? 'unsupported';
    this.list = list;
    this.select = select;
    element('native-recipient').textContent = data.origin;
    element('native-start').addEventListener('click', () => this.choose());
    element('native-another').addEventListener('click', () => this.choose());
    element('native-list').addEventListener('click', () => this.back());
  }
  /** Category transitions retire a prior native candidate and never preserve an implicit selection. */
  category(kind: ShareKind): void {
    if (this.candidate) void ipcRenderer.invoke(SHARE_PICKER.LIST).catch(() => this.end());
    this.generation++;
    this.candidate = null;
    this.pending = false;
    this.layout(false);
    element('native-entry').hidden = kind !== 'window' || this.availability === 'unsupported';
    this.availabilityCopy();
  }
  /** A quarantined adapter leaves the list usable, with no misleading permission diagnosis. */
  private availabilityCopy(): void {
    (element('native-start') as HTMLButtonElement).disabled = this.availability !== 'ready';
    element('native-status').textContent =
      this.availability === 'unavailable'
        ? 'On-screen window selection is temporarily unavailable. You can still choose a window from the list.'
        : '';
  }
  /** An explicit native choice clears old radio selection before the OS opens. */
  private choose(): void {
    if (this.pending) return;
    this.select(null);
    document.querySelectorAll<HTMLInputElement>('input[name="source"]').forEach((input) => {
      input.checked = false;
    });
    element('native-status').textContent = 'Opening macOS window selection…';
    this.action(SHARE_PICKER.NATIVE);
  }
  /** Returning to cards is explicit and retires the candidate in main before restoring list affordances. */
  private back(): void {
    if (!this.pending) this.action(SHARE_PICKER.LIST);
  }
  /** The normal form can delegate only an offered candidate, without trusting hidden native identifiers. */
  submit(): boolean {
    if (!this.candidate) return this.pending;
    if (!this.pending) {
      element('native-status').textContent = 'Starting sharing…';
      this.action(SHARE_PICKER.CONFIRM, this.candidate.id);
    }
    return true;
  }
  /** A late rejected bridge action cannot replace a newer selection or confirmation. */
  private action(channel: string, id?: string): void {
    const generation = ++this.generation;
    this.pending = true;
    this.buttons(true);
    this.response(ipcRenderer.invoke(channel, id), generation);
  }
  /** Async response handlers stay scoped to the last explicit UI action. */
  private response(pending: Promise<NativePickerPresentation>, generation: number): void {
    void pending
      .then((state: NativePickerPresentation) => {
        if (generation === this.generation) this.received(state);
      })
      .catch(() => {
        if (generation === this.generation) this.end();
      });
  }
  /** Only main's current candidate enables Select and share; failures return to an unselected catalog. */
  private received(state: NativePickerPresentation): void {
    this.pending = false;
    if (!state || state.status === 'ended') return;
    if (state.status === 'confirmation') return this.confirmation(state.source);
    this.availability = state.availability;
    this.candidate = null;
    this.list();
    element('native-status').textContent = state.message;
    (this.availability === 'ready' ? element('native-start') : element('window')).focus();
  }
  /** A real source summary remains separate from the unconfirmed source cards. */
  private confirmation(source: ShareOption): void {
    this.candidate = source;
    this.layout(true);
    this.candidateSummary(source);
    this.buttons(false);
    this.select(source);
    element('cancel').focus();
  }
  /** Literal titles can be expanded without making truncation the only way to identify a window. */
  private candidateSummary(source: ShareOption): void {
    element('native-preview').replaceChildren(pickerPreview(source));
    element('native-title').textContent = source.name;
    element('native-title-full').replaceChildren(pickerText('p', '', source.name));
    element('native-status').textContent = '';
  }
  /** Final confirmation uses the existing sheet and footer; no Oya overlay covers Apple's native picker. */
  private layout(confirmation: boolean): void {
    element('picker-heading').textContent = confirmation ? 'Share this window?' : 'Choose what to share';
    element('categories').hidden = confirmation;
    element('list-content').hidden = confirmation;
    element('native-candidate').hidden = !confirmation;
    element('native-entry').hidden = confirmation;
    element('share').textContent = confirmation ? 'Select and share' : 'Share';
  }
  /** Repeated clicks cannot issue overlapping native choices or confirmations. */
  private buttons(disabled: boolean): void {
    for (const id of ['native-start', 'native-another', 'native-list', 'share'])
      (element(id) as HTMLButtonElement).disabled = disabled;
  }
  /** A broken private IPC connection safely ends the share; no renderer error is treated as capture consent. */
  private end(): void {
    ipcRenderer.send(SHARE_PICKER.ANSWER, null);
  }
}
