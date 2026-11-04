/** Technical setup owns equal choices and direct workspace login independently of other onboarding paths. */
import { ViewModel } from '../../../core/view-model.ts';
import { connectWorkspace } from '../model/technical-session.ts';
import { TechnicalModelViewModel, type TechnicalModelBridge } from './technical-model-view-model.ts';
import { AgentsViewModel, type AgentsBridge } from './agents-view-model.ts';

/** The technical flow changes its own screen without advancing other paths. */
export type TechnicalView = 'choices' | 'agents' | 'model';
/** One low-friction choice panel owns sign-in progress and recovery. */
interface State {
  /** The current technical detail page. */ view: TechnicalView;
  /** Login is awaiting an authenticated socket. */ busy: boolean;
  /** Waiting for authentication or reconnecting the browser. */ progress: string;
  /** Safe error stays next to the initiating action. */ error: string;
}
/** The application bridge supplies credentials only to existing main-owned services. */
export type TechnicalBridge = TechnicalModelBridge & AgentsBridge;
/** Only direct workspace sign-in exits automatically; model sign-in resumes its form instead. */
export class TechnicalSetupViewModel extends ViewModel<State> {
  /** Model drafts have a separate lifetime and success proof. */ readonly model: TechnicalModelViewModel;
  /** Technical status includes missing installations, unlike the footer. */ readonly agents: AgentsViewModel;
  /** Main owns authentication and credential writes. */ private readonly bridge: TechnicalBridge;
  /** Direct sign-in cancellation cannot finish a later chosen path. */ private abort = new AbortController();
  /** Compose the bounded technical flow under the existing onboarding owner. */
  constructor(bridge: TechnicalBridge) {
    super({ view: 'choices', busy: false, progress: '', error: '' });
    this.bridge = bridge;
    this.model = new TechnicalModelViewModel(bridge);
    this.agents = new AgentsViewModel(bridge, true);
    this.own(() => this.destroy());
  }
  /** Child view models release their subscriptions with their parent. */
  private destroy(): void {
    this.reset();
    this.model.dispose();
    this.agents.dispose();
  }
  /** Back and path changes invalidate draft keys and late authentication replies. */
  reset(): void {
    this.abort.abort();
    this.abort = new AbortController();
    this.model.reset();
    this.set({ view: 'choices', busy: false, progress: '', error: '' });
  }
  /** Switching routes cancels only this flow's main-owned attempts. */
  show(view: TechnicalView): void {
    this.reset();
    void this.bridge.cancelOnboardingAttempt();
    this.set({ view });
  }
  /** Direct login finishes after authenticated socket readiness, without a Continue screen. */
  async signIn(): Promise<void> {
    if (this.state.busy) return;
    const signal = this.beginSignIn();
    return this.authenticate(signal);
  }
  /** Starting login does not privilege it over the other technical choices. */
  private beginSignIn(): AbortSignal {
    this.set({ busy: true, error: '', progress: 'Connecting browser…' });
    return this.abort.signal;
  }
  /** Native login and socket readiness share one cancellation lifetime. */
  private authenticate(signal: AbortSignal): Promise<void> {
    return connectWorkspace(this.bridge, signal, (progress) => this.set({ progress }))
      .then(
        () => this.connected(signal),
        (error) => this.failed(error, signal),
      )
      .finally(() => this.ended(signal));
  }
  /** Only the current successful sign-in enters browsing. */
  private connected(signal: AbortSignal): void {
    if (!signal.aborted) void this.bridge.finishOnboarding();
  }
  /** Inline recovery keeps the equal choices visible. */
  private failed(error: unknown, signal: AbortSignal): void {
    if (!signal.aborted)
      this.set({ error: error instanceof Error ? error.message : 'Sign-in didn’t finish. Try again.' });
  }
  /** A cancelled older attempt cannot clear current progress. */
  private ended(signal: AbortSignal): void {
    if (!signal.aborted) this.set({ busy: false, progress: '' });
  }
  /** Cancel retains the three equal starting choices. */
  cancel(): void {
    this.reset();
    void this.bridge.cancelOnboardingAttempt();
  }
}
