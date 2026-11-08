/**
 * The footer's coding agents: Claude Code, Codex and the others this computer
 * has, whether each lists Oya's MCP port, and connecting or disconnecting one.
 */
import type { LocalAgent } from '../../../../shared/ipc.ts';
import { ViewModel } from '../../../core/view-model.ts';
import type { OyaBrowser } from '../../../core/bridge.ts';
import { messageOf } from '../model/models.ts';

/** What the footer shows. */
export interface AgentsState {
  /** Discovery is outstanding; an empty array is not yet an empty-state result. */
  loading: boolean;
  /** Discovery failed, rather than finding no supported installation. */
  error: string;
  /** The installed agents. */
  agents: LocalAgent[];
  /** The agent being changed ('' for none). */
  busy: string;
  /** What the last change did, or why it failed ('' for nothing to say). */
  note: string;
  /** The agent whose practical test prompt is shown. */ selected: string;
  /** Native launch outstanding, independent of configuration writes. */ opening: string;
  /** Launch refusal leaves the prompt available for a manual run. */ launchError: string;
  /** The prompt was explicitly copied by the person. */ copied: boolean;
}

/** The parts of the bridge the agents use. */
export type AgentsBridge = Pick<
  OyaBrowser,
  'localAgents' | 'connectAgent' | 'disconnectAgent' | 'openAgent' | 'copyAgentTest'
>;

/** Agents whose config Oya can tell to run its tools without asking; the others ask the first time. */
const ALLOWED_ON_CONNECT = ['claude-code', 'codex'];

/** A list answer, or none when it is not a list. */
const listOf = (answer: unknown): LocalAgent[] => (Array.isArray(answer) ? (answer as LocalAgent[]) : []);

/** Initial discovery and launch state contains no stored credentials. */
const INITIAL: AgentsState = {
  agents: [],
  busy: '',
  note: '',
  loading: true,
  error: '',
  selected: '',
  opening: '',
  launchError: '',
  copied: false,
};

/** The footer's coding agents. */
export class AgentsViewModel extends ViewModel<AgentsState> {
  /** The main process. */
  private readonly bridge: AgentsBridge;
  /** Technical setup can show missing installations without changing the footer's compact list. */
  private readonly all: boolean;

  /** Reads which agents this computer has. */
  constructor(bridge: AgentsBridge, all = false) {
    super(INITIAL);
    this.all = all;
    this.bridge = bridge;
    void this.refresh();
  }

  /** Reads the agents again, after one was installed or changed elsewhere. */
  async refresh(): Promise<void> {
    this.set({ loading: true, error: '' });
    try {
      this.set({ agents: listOf(await (this.all ? this.bridge.localAgents(true) : this.bridge.localAgents())) });
    } catch {
      this.set({ error: 'Could not check installed agents. Try again.' });
    } finally {
      this.set({ loading: false });
    }
  }

  /** Connects agent `id` when it is not, disconnects it when it is. */
  async toggle(id: string): Promise<void> {
    const agent = this.state.agents.find((a) => a.id === id);
    if (!agent || this.state.busy) return;
    this.set({ busy: id, note: '' });
    await this.change(agent);
    if (this.all) await this.refresh();
    this.set({ busy: '' });
  }

  /** Adds Oya to `agent` or takes it out, and says how that went. */
  private change(agent: LocalAgent): Promise<void> {
    const { id, connected } = agent;
    return (connected ? this.bridge.disconnectAgent(id) : this.bridge.connectAgent(id)).then(
      // The answer lists installed agents only: a view of all of them keeps its rows until its refresh.
      (agents) => this.set(this.all ? { note: this.done(agent) } : { agents: listOf(agents), note: this.done(agent) }),
      (error: unknown) => this.set({ note: messageOf(error, `Could not change ${agent.name}.`) }),
    );
  }

  /** What a change did, by what the agent was before it. */
  private done(agent: LocalAgent): string {
    if (agent.connected) return `Oya removed from ${agent.name}.`;
    if (this.all) return `This browser is configured for ${agent.name}. Restart the agent, then try the prompt.`;
    const asks = ALLOWED_ON_CONNECT.includes(agent.id) ? '' : ' Choose Always allow the first time it asks.';
    return `Connected. Restart ${agent.name} and it can use this browser.${asks}`;
  }
  /** Selecting a test never runs a model request or fabricates a browser result. */
  select(id: string): void {
    this.set({ selected: id, launchError: '', copied: false });
  }
  /** Opening an installed agent does not send the task or count as a passed test. */
  async open(id: string): Promise<void> {
    if (this.state.opening) return;
    this.select(id);
    this.set({ opening: id });
    const result = await this.bridge
      .openAgent(id)
      .catch(() => ({ error: 'Open the agent manually, then paste the test prompt.' }));
    this.set({ opening: '', launchError: result?.error || '' });
  }
  /** Copy only the main-owned public sample; no typed key enters the clipboard. */
  async copyTest(): Promise<void> {
    try {
      await this.bridge.copyAgentTest(this.state.selected);
      this.set({ copied: true });
    } catch {
      this.set({ launchError: 'Could not copy. Select the prompt and copy it manually.' });
    }
  }
}
