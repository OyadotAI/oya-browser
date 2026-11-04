/** A provider key remains in memory through Oya login and is ready only after a real bounded test. */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { TechnicalCatalogEntry as CatalogEntry } from '../model/technical-model.ts';
import { connectWorkspace, type TechnicalSessionBridge } from '../model/technical-session.ts';

/** Stable API-key providers are distinct from subscription tokens. */
export type TechnicalProvider = 'gemini' | 'anthropic' | 'openrouter';
/** The form needs only public catalog fields and the current in-memory draft. */
interface State {
  /** No provider is preferred or initially selected. */ provider: TechnicalProvider | '';
  /** Never persisted to browser storage or placed in a URL. */ key: string;
  /** Catalog default is selected after workspace connection. */ model: string;
  /** Public server entries, never stored credentials. */ catalog: CatalogEntry[];
  /** True while login, save or test is outstanding. */ busy: boolean;
  /** Accessible progress line. */ progress: string;
  /** Saved settings can remain useful after a test failure. */ saved: boolean;
  /** Only an authenticated successful model response sets readiness. */ ready: boolean;
  /** Safe recoverable message. */ error: string;
  /** Invalid-key recovery can focus the input. */ code: string;
}
/** Provider setup shares the existing Ask settings bridge. */
export type TechnicalModelBridge = TechnicalSessionBridge &
  Pick<
    OyaBrowser,
    | 'beginTechnicalModel'
    | 'modelStatus'
    | 'saveModelKey'
    | 'testModelKey'
    | 'cancelOnboardingAttempt'
    | 'finishOnboarding'
  >;
/** Resetting a flow forgets every credential and transient operation result. */
const INITIAL: State = {
  provider: '',
  key: '',
  model: '',
  catalog: [],
  busy: false,
  progress: '',
  saved: false,
  ready: false,
  error: '',
  code: '',
};

/** Each attempt is fenced so navigation cannot resume an obsolete credential draft. */
export class TechnicalModelViewModel extends ViewModel<State> {
  /** Main owns authentication, settings and diagnostic execution. */ private readonly bridge: TechnicalModelBridge;
  /** Local cancellation fences all asynchronous results. */ private abort = new AbortController();
  /** Start with no provider preference or retained credential. */
  constructor(bridge: TechnicalModelBridge) {
    super(INITIAL);
    this.bridge = bridge;
    this.own(() => this.reset());
  }
  /** Changing provider discards the old key instead of carrying it into another vendor. */
  choose(provider: TechnicalProvider): void {
    this.reset();
    void this.bridge.cancelOnboardingAttempt();
    this.set({ provider, model: '' });
  }
  /** Editing after an unsuccessful test starts a fresh explicit save. */
  edit(key: string): void {
    this.set({ key, ready: false, saved: false, error: '', code: '' });
  }
  /** Optional model override is never guessed when the server supplies a catalog default. */
  setModel(model: string): void {
    this.set({ model, saved: false, ready: false });
  }
  /** Navigation clears credentials and cancels the corresponding main-owned work. */
  reset(): void {
    this.abort.abort();
    this.abort = new AbortController();
    this.set({ ...INITIAL, catalog: this.state.catalog });
  }

  /** Connect and test authorizes one small model request, without sending a browser task. */
  async connect(): Promise<void> {
    if (this.state.busy || !this.validKey()) return;
    const signal = this.abort.signal;
    this.set({ busy: true, error: '', code: '', ready: false });
    return this.run(signal)
      .catch((error) => this.failedCurrent(error, signal))
      .finally(() => this.ended(signal));
  }
  /** Missing keys are identified before opening authentication or sending a request. */
  private validKey(): boolean {
    if (!this.state.provider) return false;
    if (this.state.key.trim() || this.state.saved) return true;
    this.set({ error: 'Paste your provider API key.', code: 'model_key_refused' });
    return false;
  }
  /** Navigation invalidates errors as well as successful replies. */
  private failedCurrent(error: unknown, signal: AbortSignal): void {
    if (!signal.aborted) this.failed(error);
  }
  /** Completion of an older provider cannot reset a newer form's busy state. */
  private ended(signal: AbortSignal): void {
    if (!signal.aborted) this.set({ busy: false, progress: '' });
  }
  /** Resuming login is automatic and never asks for a workspace API key. */
  private async run(signal: AbortSignal): Promise<void> {
    await this.bridge.beginTechnicalModel();
    signal.throwIfAborted();
    await connectWorkspace(this.bridge, signal, (progress) => this.set({ progress }));
    if (!this.state.saved) await this.save(signal);
    signal.throwIfAborted();
    await this.verify(signal);
  }
  /** A server-confirmed provider response, not a save or sign-in, establishes readiness. */
  private async verify(signal: AbortSignal): Promise<void> {
    this.set({ progress: 'Testing provider…' });
    const answer = await this.bridge.testModelKey(this.state.provider);
    signal.throwIfAborted();
    if (answer.error) throw Object.assign(Error(answer.error), { code: answer.code });
    if (!answer.ok) throw Error('The server did not confirm a successful test.');
    this.set({ ready: true, key: '' });
  }
  /** Defaults come from the current workspace catalog, never hardcoded browser model IDs. */
  private async save(signal: AbortSignal): Promise<void> {
    const status = await this.bridge.modelStatus(true);
    signal.throwIfAborted();
    if (status.error) throw Error(String(status.error));
    const catalog = Array.isArray(status.catalog) ? (status.catalog as CatalogEntry[]) : [];
    const entry = catalog.find((item) => item.id === this.state.provider);
    if (!entry?.model) throw Error('This server needs an update to support this provider.');
    this.set({ catalog, model: this.state.model || entry.model, progress: 'Saving model key…' });
    await this.write(signal);
  }
  /** Workspace-bound main validation prevents a late catalog read from targeting another project. */
  private async write(signal: AbortSignal): Promise<void> {
    const { provider, key, model } = this.state;
    const answer = await this.bridge.saveModelKey({ provider, key: key.trim(), model, baseUrl: '' });
    signal.throwIfAborted();
    if (answer?.error) throw Error(String(answer.error));
    if (!answer?.ok) throw Error('The server did not confirm that the key was saved.');
    this.set({ saved: true });
  }
  /** Saving and testing have different recovery semantics and are announced separately. */
  private failed(error: unknown): void {
    const failure = error as DiagnosticFailure | null;
    const code = failure?.code || '';
    const detail = failureMessage(failure);
    this.set({ error: `${this.state.saved ? 'Key saved. Test did not pass. ' : ''}${detail}`, code });
  }

  /** A successful tested model can enter the existing Ask experience immediately. */
  finish(): void {
    if (this.state.ready) void this.bridge.finishOnboarding();
  }
}

/** Safe errors are small records, not vendor response bodies. */
interface DiagnosticFailure {
  /** Actionable server message. */ message?: string;
  /** Machine-readable recovery category. */ code?: string;
}
/** Older servers can save a key but cannot falsely declare a diagnostic success. */
function failureMessage(failure: DiagnosticFailure | null): string {
  return failure?.code === 'model_test_unavailable'
    ? 'The server needs an update before it can test this key.'
    : failure?.message || 'Could not connect. Try again.';
}
