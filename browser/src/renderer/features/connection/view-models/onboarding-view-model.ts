/** First-run choices and confirmed progress, independent of saved-account and manual-form state. */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { ConnectionStatus } from '../../../../shared/ipc.ts';
import { messageOf } from '../model/models.ts';
import { SETUP_GUIDE } from '../model/setup-constants.ts';
import { TechnicalSetupViewModel, type TechnicalBridge } from './technical-setup-view-model.ts';

/** The three starting points requested at first launch. */
export type SetupPath = 'technical' | 'nontechnical' | 'expert';
/** Each step offers one decision, with optional follow-up after connection. */
export type OnboardingPage =
  | 'choices'
  | 'technical'
  | 'assistant'
  | 'local'
  | 'claude'
  | 'expert'
  | 'waiting'
  | 'connected'
  | 'guide'
  | 'optional';
/** Progress belongs to this screen, never to a late provider callback. */
interface OnboardingState {
  /** Chosen starting point, retained across recoverable failures. */
  path: SetupPath | null;
  /** The current decision or guide step. */
  page: OnboardingPage;
  /** A provider or lifecycle call is outstanding. */
  busy: boolean;
  /** Safe actionable error. */
  error: string;
  /** The main process confirmed an authenticated socket. */
  connected: boolean;
  /** The provider configuration call also completed successfully. */
  chatgptReady: boolean;
  /** The current walkthrough position. */
  step: number;
}
/** The shell-only operations this flow needs. */
export type OnboardingBridge = TechnicalBridge &
  Pick<
    OyaBrowser,
    | 'beginOnboarding'
    | 'cancelOnboardingAttempt'
    | 'onboardingStatus'
    | 'connectChatgpt'
    | 'onWsStatus'
    | 'finishOnboarding'
    | 'openSetupHelp'
    | 'openConsole'
  >;

/** One setup flow fences stale results while leaving the connection hold with main. */
export class OnboardingViewModel extends ViewModel<OnboardingState> {
  /** Technical setup has its own login and provider draft lifetime. */
  readonly technical: TechnicalSetupViewModel;
  /** Shell bridge with the existing authorization boundary. */
  private readonly bridge: OnboardingBridge;
  /** Returns to the shared first-run view and clears obsolete form errors. */
  private readonly ready: () => void;
  /** A generation invalidates provider results after changing paths. */
  private generation = 0;
  /** Only a deliberate connection attempt can advance the screen automatically. */
  private pending = false;

  /** Observes real socket readiness, rather than inferring it from a key. */
  constructor(bridge: OnboardingBridge, ready: () => void) {
    super({ path: null, page: 'choices', busy: false, error: '', connected: false, chatgptReady: false, step: 0 });
    this.bridge = bridge;
    this.technical = new TechnicalSetupViewModel(bridge);
    this.ready = ready;
    this.watch();
  }
  /** All subscriptions share the onboarding owner's lifetime. */
  private watch(): void {
    this.own(() => this.technical.dispose());
    this.own(this.bridge.onWsStatus((status) => this.status(status)));
    this.own(() => {
      this.generation++;
      void this.bridge.cancelOnboardingAttempt();
    });
  }

  /** Starts a chosen path and invalidates an older provider attempt. */
  choose(path: SetupPath | null): void {
    this.technical.reset();
    this.generation++;
    this.pending = false;
    void this.bridge.cancelOnboardingAttempt();
    const pages = { technical: 'technical', nontechnical: 'assistant', expert: 'expert' } as const;
    this.set({ path, page: path ? pages[path] : 'choices', busy: false, error: '', chatgptReady: false });
    this.ready();
  }

  /** Opens a detail screen without changing the selected starting point. */
  show(page: OnboardingPage): void {
    this.set({ page, error: '' });
  }

  /** A UI continuation must still belong to this path and a pending connection attempt. */
  guard(): () => boolean {
    const generation = this.generation;
    return () => generation === this.generation && this.pending;
  }

  /** Holds first-run UI before a manual or paired connection can authenticate. */
  async begin(): Promise<boolean> {
    const generation = this.generation;
    await this.bridge.beginOnboarding();
    if (generation !== this.generation) return false;
    this.pending = true;
    this.set({ error: '' });
    return true;
  }

  /** The supported ChatGPT approval flow never requests pasted session tokens. */
  async connectChatgpt(): Promise<void> {
    if (this.state.busy) return;
    const generation = this.generation;
    this.set({ busy: true, error: '' });
    try {
      await this.connectProvider(generation);
    } catch (error) {
      if (generation === this.generation) this.failed(error);
    }
  }

  /** Late provider replies cannot complete a newer choice or claim an unconnected browser is ready. */
  private async connectProvider(generation: number): Promise<void> {
    await this.begin();
    if (generation !== this.generation) return;
    this.set({ page: 'waiting' });
    const answer = await this.bridge.connectChatgpt({ importLogins: false });
    if (generation !== this.generation) return;
    if (answer?.error) throw Error(answer.error);
    await this.providerReady(generation);
  }

  /** The provider and authenticated socket must both confirm readiness. */
  private async providerReady(generation: number): Promise<void> {
    this.set({ busy: false, chatgptReady: true });
    const status = await this.bridge.onboardingStatus();
    if (generation === this.generation) this.status({ connected: !!status?.connected });
  }

  /** A failed provider cannot be completed by an unrelated connection event. */
  private failed(error: unknown): void {
    this.pending = false;
    this.set({ error: messageOf(error), busy: false });
  }

  /** Socket events retain guide position on disconnect and advance only an active attempt. */
  private status(status: ConnectionStatus): void {
    this.set({ connected: !!status.connected });
    if (status.failure) this.set({ error: status.failure, busy: false });
    if (!status.connected || !this.pending || (this.state.busy && !this.state.chatgptReady)) return;
    this.pending = false;
    const page = this.state.page === 'guide' || this.state.page === 'optional' ? this.state.page : 'connected';
    this.set({ page, error: '', busy: false });
    this.ready();
  }

  /** Cancels a pending attempt while keeping first-run setup available. */
  cancel(): void {
    this.choose(this.state.path);
  }

  /** Moves within a bounded walkthrough; the caller explicitly finishes or skips it. */
  move(delta: number): void {
    this.set({ step: Math.max(0, Math.min(SETUP_GUIDE.length - 1, this.state.step + delta)) });
  }

  /** Opens a fixed provider help page through main, never a renderer-supplied URL. */
  help(topic: 'claude-install' | 'claude-auth'): void {
    void this.bridge
      .openSetupHelp(topic)
      .catch(() => this.set({ error: 'Could not open the instructions. Try again.' }));
  }

  /** Opens the paired workspace without claiming a guide step was completed. */
  openConsole(): void {
    void this.bridge.openConsole().catch(() => this.set({ error: 'Could not open the control plane. Try again.' }));
  }

  /** Explicit browsing releases the main-process hold and fences pending provider work. */
  finish(): void {
    this.technical.reset();
    this.generation++;
    this.pending = false;
    void this.bridge.finishOnboarding();
  }
}
