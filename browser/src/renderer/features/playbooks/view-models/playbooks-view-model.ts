/** The saved playbook library, edits, and replay status for this desktop. */
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import type { PanelViewModel } from '../../../app/panel/panel-view-model.ts';
import type { Payload } from '../../../../shared/ipc.ts';
import type { Playbook, PlaybookRun } from '../model/types.ts';
import { RUN_POLL_MS, ENDED } from '../model/constants.ts';

/** Library state, including recoverable errors and the selected input form. */
export interface PlaybooksState {
  /** Saved playbooks. */
  items: Playbook[];
  /** Search text. */
  query: string;
  /** Selected playbook name. */
  selected: string;
  /** Replay variable values. */
  values: Record<string, string>;
  /** Whether broken steps may use the project's model. */
  autoHeal: boolean;
  /** An API mutation is in progress. */
  busy: boolean;
  /** Initial list is loading. */
  loading: boolean;
  /** Error or retry guidance. */
  error: string;
  /** Success feedback. */
  note: string;
  /** Most recent replay. */
  run: PlaybookRun | null;
}
/** Empty state shared only as an immutable initial snapshot. */
const INITIAL_STATE: PlaybooksState = {
  items: [],
  query: '',
  selected: '',
  values: {},
  autoHeal: true,
  busy: false,
  loading: false,
  error: '',
  note: '',
  run: null,
};
/** Feedback for successful library operations. */
const SAVED: Record<string, string> = { export: 'Playbook exported.', delete: 'Playbook deleted.' };

/** The library uses only the trusted bridge and the workspace panel. */
export class PlaybooksViewModel extends ViewModel<PlaybooksState> {
  /** Main-process library operations. */
  private readonly bridge: Pick<OyaBrowser, 'playbooks' | 'onWsStatus' | 'onModeChanged'>;
  /** Workspace navigation. */
  private readonly panel: PanelViewModel;
  /** Polling timer, owned by this view model. */
  private timer: ReturnType<typeof setTimeout> | undefined;
  /** Stop asynchronous callbacks after disposal. */
  private disposed = false;
  /** Share an in-flight refresh with View playbook navigation. */
  private refreshTask: Promise<void> | undefined;
  /** Invalidates project data returned after sign-out. */
  private generation = 0;
  /** Builds an empty library and refreshes whenever it is opened or reconnected. */
  constructor(bridge: Pick<OyaBrowser, 'playbooks' | 'onWsStatus' | 'onModeChanged'>, panel: PanelViewModel) {
    super(INITIAL_STATE);
    this.bridge = bridge;
    this.panel = panel;
    this.own(panel.subscribe(() => this.shown()));
    this.own(bridge.onWsStatus(() => this.shown()));
    this.watchMode();
    this.own(() => this.stop());
  }
  /** Follow sign-out separately from transient connection changes. */
  private watchMode(): void {
    this.own(
      this.bridge.onModeChanged((mode) => {
        if (mode === 'setup') this.reset();
      }),
    );
  }
  /** Refresh only while the library is visible. */
  private shown(): void {
    if (this.panel.state.pane === 'playbooks') void this.refresh();
  }
  /** Release polling when the shell is disposed. */
  private stop(): void {
    this.disposed = true;
    clearTimeout(this.timer);
  }
  /** Filtered saved playbooks. */
  get matches(): Playbook[] {
    return this.state.items.filter((item) => item.name.toLowerCase().includes(this.state.query.toLowerCase()));
  }
  /** Selected details, if the playbook still exists. */
  get selected(): Playbook | undefined {
    return this.state.items.find((item) => item.name === this.state.selected);
  }
  /** Whether the current run is still active. */
  get running(): boolean {
    return !!this.state.run && !ENDED.includes(this.state.run.status);
  }
  /** Show the library, optionally selecting a newly saved playbook. */
  async open(name = ''): Promise<void> {
    await this.panel.open('playbooks');
    await this.refresh();
    if (name) this.select(name);
  }
  /** Search without changing the saved collection. */
  search(query: string): void {
    this.set({ query });
  }
  /** Choose a playbook and load its recorded defaults. */
  select(selected: string): void {
    const item = this.state.items.find((entry) => entry.name === selected);
    this.set({ selected, values: { ...item?.defaults }, error: '', note: '' });
  }
  /** Edit one replay input. */
  value(name: string, value: string): void {
    this.set({ values: { ...this.state.values, [name]: value } });
  }
  /** Choose whether a replay can use model-assisted repair. */
  heal(autoHeal: boolean): void {
    this.set({ autoHeal });
  }
  /** Refresh without throwing away a usable list during a transient failure. */
  refresh(): Promise<void> {
    if (this.refreshTask) return this.refreshTask;
    const task = this.readLibrary(this.generation);
    this.refreshTask = task;
    void task.finally(() => {
      if (this.refreshTask === task) this.refreshTask = undefined;
    });
    return task;
  }
  /** Load one snapshot, ignoring responses belonging to a signed-out project. */
  private async readLibrary(generation: number): Promise<void> {
    this.set({ loading: true });
    await this.call({ action: 'list' }).then(
      (result) => this.loaded(generation, result),
      (error: unknown) => this.loadFailed(generation, error),
    );
  }
  /** Apply a list only if the requesting account still owns this view. */
  private loaded(generation: number, result: Payload): void {
    if (this.current(generation))
      this.set({ items: (result.playbooks || []) as Playbook[], error: '', loading: false });
  }
  /** Retain usable data but stop the loading indicator after a failed refresh. */
  private loadFailed(generation: number, error: unknown): void {
    if (!this.current(generation)) return;
    this.fail(error);
    this.set({ loading: false });
  }
  /** Whether an asynchronous result still belongs to this visible account. */
  private current(generation: number): boolean {
    return !this.disposed && generation === this.generation;
  }
  /** Signing out removes project data and ends renderer polling. */
  private reset(): void {
    this.generation++;
    this.refreshTask = undefined;
    clearTimeout(this.timer);
    this.set(INITIAL_STATE);
  }
  /** Rename, delete or transfer a playbook, then refresh the collection. */
  async manage(action: string, to = ''): Promise<void> {
    if (this.state.busy || this.running) return;
    await this.perform(async () => {
      const result = await this.call({ action, name: this.state.selected, to });
      await this.refresh();
      this.managed(action, to, result);
    });
  }
  /** Apply selection and confirmation after a library edit succeeds. */
  private managed(action: string, to: string, result: Payload): void {
    if (action === 'rename') this.select(to);
    if (action === 'delete') this.select('');
    const note = Object.hasOwn(SAVED, action) ? SAVED[action] : 'Saved.';
    this.set({ note: result.canceled ? '' : note });
  }

  /** Replay on the current browser, then follow its background run. */
  async run(): Promise<void> {
    if (this.state.busy || this.running || !this.selected) return;
    const { selected: name, values, autoHeal } = this.state;
    await this.perform(async () => {
      const run = await this.call({ action: 'run', name, values, autoHeal });
      this.set({ run: run as unknown as PlaybookRun });
      this.follow();
    });
  }
  /** Shared mutation lifetime prevents double submission and always clears busy state. */
  private async perform(work: () => Promise<void>): Promise<void> {
    this.set({ busy: true, error: '', note: '' });
    await work()
      .catch((error: unknown) => this.fail(error))
      .finally(() => this.set({ busy: false }));
  }
  /** Send the person's answer to a run awaiting assistance. */
  async respond(response: string): Promise<void> {
    try {
      await this.call({ action: 'respond', id: this.state.run?.id, response });
      this.follow();
    } catch (error) {
      this.fail(error);
    }
  }
  /** Fetch the latest run status; failed polls remain retryable after reconnect. */
  private async poll(): Promise<void> {
    const generation = this.generation;
    try {
      const run = await this.call({ action: 'status', id: this.state.run?.id });
      if (this.current(generation)) this.set({ run: run as unknown as PlaybookRun, error: '' });
    } catch (error) {
      if (this.current(generation)) this.fail(error);
    }
    if (!this.disposed) this.follow();
  }
  /** Schedule at most one poll while work is active. */
  private follow(): void {
    clearTimeout(this.timer);
    if (this.running && !this.disposed) this.timer = setTimeout(() => void this.poll(), RUN_POLL_MS);
  }
  /** Convert IPC refusal payloads into ordinary view-model errors. */
  private async call(command: Payload): Promise<Payload> {
    const result = await this.bridge.playbooks(command);
    if (result.error) throw new Error(String(result.error));
    return result;
  }
  /** Present an actionable error in the pane. */
  private fail(error: unknown): void {
    this.set({ error: error instanceof Error ? error.message : 'Could not load playbooks. Try again.' });
  }
}
