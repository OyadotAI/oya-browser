/** Private sandboxed picker UI; source titles are text and only opaque choice tokens return to main. */
import { ipcRenderer } from 'electron';
import { SHARE_PICKER, type ShareKind, type ShareOption, type SharePresentation } from '../shared/share-picker.ts';

/** Resolve a fixed element from the packaged browser-owned page. */
const element = (id: string): HTMLElement => document.getElementById(id)!;
/** Build literal text nodes, never website-controlled markup. */
function text(tag: string, className: string, value: string): HTMLElement {
  const node = document.createElement(tag);
  node.className = className;
  node.textContent = value;
  return node;
}
/** Render browser-produced PNG data only, with a readable placeholder for sleeping tabs. */
function preview(option: ShareOption): HTMLElement {
  const box = text('span', 'preview placeholder', 'Preview unavailable');
  if (!option.thumbnail.startsWith('data:image/png;base64,') || option.thumbnail.endsWith(',')) return box;
  loadPreview(box, option.thumbnail);
  return box;
}
/** Keep the placeholder until actual PNG bytes have decoded successfully. */
function loadPreview(box: HTMLElement, thumbnail: string): void {
  const image = document.createElement('img');
  image.alt = '';
  image.addEventListener('load', () => {
    box.className = 'preview';
    box.replaceChildren(image);
  });
  image.src = thumbnail;
}
/** A real radio input provides keyboard selection and accessible checked state. */
function sourceCard(option: ShareOption, select: () => void): HTMLLabelElement {
  const label = document.createElement('label');
  label.className = 'source';
  const input = document.createElement('input');
  Object.assign(input, { type: 'radio', name: 'source', value: option.id, ariaLabel: option.name });
  input.addEventListener('change', select);
  label.append(input, preview(option), text('span', 'name', option.name), text('span', 'detail', option.detail));
  return label;
}

/** Native sheet state stays in the isolated preload, with no bridge exposed to the page world. */
class PickerView {
  /** Safe presentation data from the requesting operation. */
  private readonly data: SharePresentation;
  /** Each category change requires an explicit new choice. */
  private selected: ShareOption | null = null;

  /** Bind static controls once and show the tabs category first. */
  constructor(data: SharePresentation) {
    this.data = data;
    element('origin').textContent = data.origin;
    this.controls();
    this.category('tab');
    element('content').hidden = false;
    element('cancel').focus();
    requestAnimationFrame(() => ipcRenderer.send(SHARE_PICKER.READY));
  }

  /** Category navigation, Cancel and explicit Share are the only intents. */
  private controls(): void {
    for (const kind of ['tab', 'window', 'screen'] as const) {
      element(kind).addEventListener('click', () => this.category(kind));
      element(kind).addEventListener('keydown', (event) => this.tabKey(event, kind));
    }
    element('cancel').addEventListener('click', () => this.answer(null));
    element('settings').addEventListener('click', () => this.settings());
    element('content').addEventListener('submit', (event) => this.submit(event));
    document.addEventListener('keydown', (event) => this.escape(event));
  }

  /** A failed OS handoff leaves a readable manual recovery path in the picker. */
  private settings(): void {
    void ipcRenderer.invoke(SHARE_PICKER.SETTINGS).catch(() => {
      element('empty').textContent = 'Open System Settings → Privacy & Security → Screen & System Audio Recording.';
    });
  }

  /** Escape cancels without preserving a previous source selection. */
  private escape(event: KeyboardEvent): void {
    if (event.key === 'Escape') this.answer(null);
  }

  /** Submitting without a selected source never grants a stream. */
  private submit(event: Event): void {
    event.preventDefault();
    if (this.selected) this.answer(this.selected.id);
  }

  /** Switching a category clears selection, so Enter cannot accidentally share a different source. */
  private category(kind: ShareKind): void {
    this.selected = null;
    this.selection();
    for (const category of ['tab', 'window', 'screen'] as const) this.tabState(category, kind);
    const sources = this.data.sources.filter((option) => option.kind === kind);
    element('sources').replaceChildren(...sources.map((option) => sourceCard(option, () => this.select(option))));
    element('sources').setAttribute('aria-labelledby', kind);
    this.empty(kind, sources.length === 0);
  }

  /** Empty categories explain an OS failure without blocking the other source categories. */
  private empty(kind: ShareKind, empty: boolean): void {
    element('empty').hidden = !empty;
    element('recovery').hidden = !empty || kind === 'tab' || !this.data.canOpenSettings;
    element('empty').textContent =
      kind !== 'tab' && this.data.desktopError ? this.data.desktopError : 'No sources available in this category.';
  }

  /** Keep visible categories and their source counts readable to assistive technology. */
  private tabState(kind: ShareKind, active: ShareKind): void {
    const button = element(kind);
    button.setAttribute('aria-selected', String(kind === active));
    button.tabIndex = kind === active ? 0 : -1;
    element(`${kind}-count`).textContent = String(this.data.sources.filter((source) => source.kind === kind).length);
  }

  /** Arrow keys move between category tabs with the standard roving-tab pattern. */
  private tabKey(event: KeyboardEvent, kind: ShareKind): void {
    const next: Record<ShareKind, ShareKind> = { tab: 'window', window: 'screen', screen: 'tab' };
    const previous: Record<ShareKind, ShareKind> = { tab: 'screen', window: 'tab', screen: 'window' };
    if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return;
    event.preventDefault();
    const chosen = event.key === 'ArrowRight' ? next[kind] : previous[kind];
    this.category(chosen);
    element(chosen).focus();
  }

  /** Source selection enables Share and names exactly what the person chose. */
  private select(option: ShareOption): void {
    this.selected = option;
    this.selection();
  }

  /** No default source, and no implicit audio capture. */
  private selection(): void {
    (element('share') as HTMLButtonElement).disabled = !this.selected;
    element('selection').textContent = this.selected ? this.selected.name : 'Choose what you want to share';
  }

  /** IPC carries no raw window identifier, frame handle or page-supplied URL. */
  private answer(id: string | null): void {
    ipcRenderer.send(SHARE_PICKER.ANSWER, id);
  }
}

/** A missing or rejected request closes the sheet instead of leaving an empty modal. */
function start(): void {
  ipcRenderer
    .invoke(SHARE_PICKER.READ)
    .then((data: SharePresentation) => new PickerView(data))
    .catch(() => ipcRenderer.send(SHARE_PICKER.ANSWER, null));
}
document.addEventListener('DOMContentLoaded', start, { once: true });
