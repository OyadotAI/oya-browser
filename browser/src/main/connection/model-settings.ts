/** Model writes and diagnostics keep a bounded owner and workspace across all asynchronous steps. */
import type { WebContents } from 'electron';
import type { AppServices } from '../app/services.ts';
import { ServerApi } from './server-api.ts';
import { SetupAttempt } from '../chatgpt/index.ts';
import { modelUpdate, statusOf, type ModelChoice, type ServerConfig } from './model-config.ts';
import type { Payload } from '../../shared/ipc.ts';
import { MODEL_SETTINGS_TIMEOUT_MS, MODEL_DIAGNOSTIC_TIMEOUT_MS } from './constants.ts';

/** Existing configuration and socket define the workspace receiving model credentials. */
type Deps = Pick<AppServices, 'config' | 'socket'>;
/** A draft is bound before authentication and may adopt only its same-owner login. */
interface ModelFlow {
  /** Expected workspace and draft cancellation. */ attempt: SetupAttempt;
  /** Originating shell owns the draft lifetime. */ owner: WebContents;
  /** Exact callback is removed after navigation or completion. */ closed: () => void;
}
/** Only a window's own pending model operation may be cancelled by its navigation. */
export class ModelSettings {
  /** The configured project and its authenticated API. */ private readonly deps: Deps;
  /** HTTP requests reuse the existing authorization and destination snapshot. */ private readonly api: ServerApi;
  /** At most one outstanding mutation/test per owning shell. */ private readonly pending = new Map<
    number,
    SetupAttempt
  >();
  /** Draft scopes survive their save and test requests until navigation. */
  private readonly flows = new Map<number, ModelFlow>();
  /** Compose this service once; no credential is returned by any method. */
  constructor(deps: Deps) {
    this.deps = deps;
    this.api = new ServerApi(deps);
  }
  /** Capture the destination at Connect, before external authentication. */
  beginFlow(owner: WebContents): void {
    this.cancel(owner.id);
    if (owner.isDestroyed()) throw Error('This setup window closed.');
    const closed = () => this.cancel(owner.id);
    const attempt = new SetupAttempt(() => this.deps.config.values);
    this.flows.set(owner.id, { attempt, owner, closed });
    owner.once('destroyed', closed);
  }
  /** Only this shell's native sign-in may adopt its new workspace credential. */
  adoptLogin(owner: number, apiKey: string): void {
    this.flows.get(owner)?.attempt.adoptKey(apiKey);
  }
  /** Technical catalog reads share save cancellation, deadlines and captured workspace checks. */
  status(owner: WebContents): Promise<Payload> {
    return this.during(owner, async (attempt) => {
      const config = await this.read(attempt);
      attempt.check();
      return statusOf(config, true);
    });
  }
  /** Ask and onboarding save through the same validation rules and cancellable write. */
  write(owner: WebContents, choice: ModelChoice): Promise<void> {
    const requireCatalog = this.flows.has(owner.id);
    return this.during(owner, (attempt) => this.save(choice, attempt, requireCatalog));
  }
  /** Validate against a bounded catalog read before writing the same captured project. */
  private async save(choice: ModelChoice, attempt: SetupAttempt, requireCatalog: boolean): Promise<void> {
    const config = await this.read(attempt);
    attempt.check();
    const body = modelUpdate(config, choice, requireCatalog);
    if (body.error) throw Error(String(body.error));
    await this.api.postCancelable('config', body, this.signal(attempt, MODEL_SETTINGS_TIMEOUT_MS));
    attempt.check();
  }
  /** Catalog reads cannot hold a cancelled setup or outlive its request budget. */
  private read(attempt: SetupAttempt): Promise<ServerConfig> {
    return this.api.getCancelable('config', this.signal(attempt, MODEL_SETTINGS_TIMEOUT_MS)) as Promise<ServerConfig>;
  }
  /** A successful save is distinct from a real own-key test. */
  test(owner: WebContents, provider: string): Promise<unknown> {
    return this.during(owner, (attempt) => this.verify(provider, attempt));
  }
  /** Diagnostic results belong only to the selected workspace and live owner. */
  private async verify(provider: string, attempt: SetupAttempt): Promise<unknown> {
    const answer = await this.api.postCancelable(
      'config/model-test',
      { provider },
      this.signal(attempt, MODEL_DIAGNOSTIC_TIMEOUT_MS),
    );
    attempt.check();
    return answer;
  }
  /** One pending operation owns cancellation until its network work has settled. */
  private async during<T>(owner: WebContents, work: (attempt: SetupAttempt) => Promise<T>): Promise<T> {
    const attempt = this.begin(owner);
    const closed = () => this.cancel(owner.id);
    owner.once('destroyed', closed);
    try {
      return await work(attempt);
    } finally {
      this.release(owner, attempt, closed);
    }
  }
  /** A closed or unauthenticated shell cannot start a write. */
  private begin(owner: WebContents): SetupAttempt {
    if (!this.api.canCall() || owner.isDestroyed()) throw Error('Reconnect to your workspace and try again.');
    if (this.pending.has(owner.id)) throw Error('A model connection is still finishing. Try again in a moment.');
    this.flows.get(owner.id)?.attempt.check();
    const attempt = new SetupAttempt(() => this.deps.config.values);
    this.pending.set(owner.id, attempt);
    attempt.check();
    return attempt;
  }
  /** Completion removes only this operation, never a replacement owned by another window. */
  private release(owner: WebContents, attempt: SetupAttempt, closed: () => void): void {
    owner.removeListener('destroyed', closed);
    if (this.pending.get(owner.id) === attempt) this.pending.delete(owner.id);
    attempt.abort.abort();
  }
  /** Deadline and cancellation both reach the underlying fetch. */
  private signal(attempt: SetupAttempt, timeout: number): AbortSignal {
    return AbortSignal.any([attempt.abort.signal, AbortSignal.timeout(timeout)]);
  }
  /** Flow exit cancels its owner; sign-out/shutdown cancel all owners. */
  cancel(owner?: number): void {
    for (const [id, attempt] of this.pending) if (owner === undefined || owner === id) attempt.abort.abort();
    for (const [id, flow] of this.flows) if (owner === undefined || owner === id) this.closeFlow(id, flow);
  }
  /** Cleanup removes only this draft and its exact native closure listener. */
  private closeFlow(id: number, flow: ModelFlow): void {
    this.flows.delete(id);
    flow.attempt.abort.abort();
    flow.owner.removeListener('destroyed', flow.closed);
  }
}
