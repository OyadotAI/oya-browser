/**
 * The Ask pane's setup cards, so Ask works without a trip to the web console:
 * "Sign in" while this browser has no project, and "Connect an AI model" to
 * pick the project's provider, model and key. The card always shows what the
 * server runs on: it is built from the server's catalog, re-read whenever it
 * opens, and again when the server says the settings changed elsewhere. A
 * question asked before a model was set is sent again once it is.
 */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import { ASK_TEXT } from '../model/constants.ts';
import type { CatalogEntry, ModelStatus } from '../model/types.ts';
import { ModelPickerViewModel } from './model-picker-view-model.ts';

/** What the server said last about the project's model. */
export interface ModelSummary {
  /** The providers it offers. */
  catalog: CatalogEntry[];
  /** The provider in use. */
  provider: string;
  /** The model in use. */
  model: string;
  /** Whether a key is set. */
  hasLlmKey: boolean;
  /** Effective endpoint reported by the server. */
  baseUrl?: string;
}

/** What the setup cards show. */
export interface ModelSetupState {
  /** This browser belongs to a project (else "Sign in" shows, and the Model button hides). */
  signedIn: boolean;
  /** The model card is open. */
  open: boolean;
  /** The card was opened by choice, so it offers Cancel. */
  optional: boolean;
  /** What the server said last. */
  status: ModelSummary;
  /** The provider picked in the card. */
  picked: string;
  /** The key typed. */
  key: string;
  /** Custom endpoint edited in this card. */
  baseUrl: string;
  /** Why saving failed, or ''. */
  error: string;
}

/** What the key field says. */
export interface KeyHint {
  /** Its placeholder: the saved key's dots, or what a key looks like. */
  placeholder: string;
  /** The line under it. */
  help: string;
}

/** The parts of the bridge the setup cards use. */
export type ModelBridge = Pick<
  OyaBrowser,
  'modelStatus' | 'saveModelKey' | 'openConsole' | 'newTab' | 'onWsStatus' | 'onSettingsChanged'
>;

/** What an unreadable status counts as: signed in with a key, so nothing nags. */
const UNSURE: ModelStatus = { signedIn: true, error: 'Could not load model settings. Try again.' };

/** The key field's placeholder and help: whether the saved key carries over (same provider), or what to paste. */
export function keyHint(status: ModelSummary, entry: CatalogEntry | undefined): KeyHint {
  const keeps = !!entry && entry.id === status.provider && status.hasLlmKey;
  if (keeps) return { placeholder: ASK_TEXT.keySaved, help: ASK_TEXT.keyKept };
  return { placeholder: entry?.hint || ASK_TEXT.keyPlaceholder, help: `Paste your ${entry?.label || ''} key.` };
}

/** The setup cards. */
export class ModelSetupViewModel extends ViewModel<ModelSetupState> {
  /** The model list inside the card. */
  readonly picker = new ModelPickerViewModel();
  /** The main process. */
  private readonly bridge: ModelBridge;
  /** Asks again the question that waited for a model. */
  private readonly resend: () => void;
  /** A question waits for a model, and goes again once one is saved. */
  private pending = false;

  /** Hears the connection and settings changes, and reads the server now; `resend` asks a waiting question again. */
  constructor(bridge: ModelBridge, resend: () => void) {
    const status = { catalog: [], provider: '', model: '', hasLlmKey: true };
    super({ signedIn: true, open: false, optional: false, status, picked: '', key: '', baseUrl: '', error: '' });
    this.bridge = bridge;
    this.resend = resend;
    this.own(bridge.onWsStatus(() => void this.refresh()));
    this.own(bridge.onSettingsChanged(() => void this.changed()));
    this.own(() => this.picker.dispose());
    void this.refresh();
  }

  /** The provider picked in the card, from the catalog. */
  get entry(): CatalogEntry | undefined {
    return this.state.status.catalog.find((p) => p.id === this.state.picked);
  }

  /** Shows whichever card this browser needs, if any, from what the server says now. */
  async refresh(): Promise<void> {
    const signedIn = !!(await this.load()).signedIn;
    this.set({ signedIn });
    if (!signedIn) this.set({ open: false });
    else if (!this.state.status.hasLlmKey) this.show(false);
    else if (!this.state.optional) this.set({ open: false });
  }

  /** The "Model" button: re-reads the server first, so the card opens on what it runs now. */
  async openCard(): Promise<void> {
    await this.load();
    if (!this.state.error) this.render();
    this.show(true);
  }

  /** The server refused a question for want of a working key (`reason` when the provider refused it): ask for one, then send it again. */
  needed(reason?: string): void {
    this.pending = true;
    this.show(false);
    if (reason) this.set({ error: reason });
  }

  /** A provider chip was clicked: picks it, on its default model. */
  choose(id: string): void {
    if (id !== this.state.picked) {
      this.pick(id);
      this.set({ baseUrl: '' });
    }
  }

  /** The key was typed. */
  setKey(key: string): void {
    this.set({ key });
  }

  /** Edits the optional provider-compatible endpoint. */
  setEndpoint(baseUrl: string): void {
    this.set({ baseUrl });
  }

  /** Hides the card, forgetting the typed key. */
  hide(): void {
    this.set({ open: false, key: '' });
  }

  /** Saves the provider, model and key to the project, then sends a waiting question again. */
  async save(): Promise<void> {
    const choice = this.choice();
    const saved = await this.bridge.saveModelKey(choice).catch((e: Error) => ({ error: e.message }));
    if (saved?.error) return this.set({ error: saved.error });
    this.hide();
    await this.load();
    if (this.pending) this.resend();
    this.pending = false;
  }

  /** Build the saved fields without ever returning the stored credential. */
  private choice() {
    const { picked: provider, key, baseUrl } = this.state;
    return { provider, key, baseUrl, model: this.picker.state.value.trim() };
  }

  /** Opens the picked provider's key page in a new tab. */
  getKey(): void {
    const url = this.entry?.keysUrl;
    if (url) void this.bridge.newTab(url);
  }

  /** "Sign in with Oya": the console signs this browser in. */
  signIn(): void {
    void this.bridge.openConsole();
  }

  /** The settings changed elsewhere: re-read them, and redraw the card unless the person is editing it. */
  private async changed(): Promise<void> {
    const editing = this.state.open;
    await this.refresh();
    if (!editing) this.render();
  }

  /** Reads the project's model from the server; unsure counts as signed in with a key. */
  private async load(): Promise<ModelStatus> {
    const status = ((await this.bridge.modelStatus().catch(() => null)) as ModelStatus | null) ?? UNSURE;
    this.loaded(status);
    return status;
  }

  /** Apply successful settings or retain the last known selection on failure. */
  private loaded(status: ModelStatus): void {
    if (status.error) return this.set({ error: status.error });
    const { catalog = [], provider = '', model = '', hasLlmKey = false } = status;
    this.set({ error: '', status: { catalog, provider, model, hasLlmKey, baseUrl: status.baseUrl } });
  }

  /** Opens the card on the server's current choice (not while signed out, nor when open); `optional` offers Cancel. */
  private show(optional: boolean): void {
    if (!this.state.signedIn || this.state.open) return;
    this.render();
    this.set({ open: true, optional });
  }

  /** Puts the card on the server's choice: its provider, and its model on that provider. */
  private render(): void {
    const { catalog, provider, model, baseUrl = '' } = this.state.status;
    this.set({ baseUrl });
    const known = catalog.some((p) => p.id === provider);
    this.pick(known ? provider : catalog[0]?.id || '', known ? model : '');
  }

  /** Picks provider `id` and offers its models, on `model` or else its default. */
  private pick(id: string, model = ''): void {
    this.set({ picked: id });
    const entry = this.entry;
    this.picker.setModels(entry?.models ?? [], model || entry?.model || '');
  }
}
